# Moidify Rust Rewrite — Architecture & Plan

**Status:** planning  
**Target:** full replacement of the Python backend; frontend unchanged  
**Non-goal:** porting line-by-line. This is a re-architecture; Python is the behavioral spec.

---

## 1. Why rewrite now (scope reality check)

| Asset | Size | Fate |
|---|---|---|
| Python backend | ~6,300 LOC, 145 HTTP endpoints, 14 tables | Replaced by Rust |
| Frontend (vanilla JS) | ~11,800 LOC | **Untouched** — same API contract |
| Shell (install/CLI) | ~1,050 LOC | Mostly kept; small updates for Rust binary |
| Tests | ~40 pytest cases | Ported to Rust as the contract suite |

Biggest Python files → biggest rewrite risk: `routes/admin.py` (1,489), `subsonic.py` (974), `routes/playlists.py` (706), `scanner.py` (476).

---

## 2. Technology choices

| Concern | Choice | Why (vs Python) |
|---|---|---|
| HTTP framework | **axum 0.8** + tower | Extractor-based auth, tower middleware, tokio-native |
| Runtime | **tokio** (multi-thread) | Streaming, watchers, subprocesses |
| Middleware | **tower-http** | Static files w/ Range, CORS, compression, security headers as `Layer`s |
| DB | **SQLx (SqlitePool)**, runtime queries | Async pool (no connect-per-request); same SQLite file |
| Schema | Hand-written `init` mirroring `database.py` | **Drop-in compatible with existing `music.db`** |
| Metadata | **lofty** | ID3/Vorbis/MP4/FLAC tags + pictures (mutagen replacement) |
| FS watch | **notify** + debouncer | Watchdog replacement |
| HTTP client | **reqwest** (rustls) | LRCLIB, Deezer, Last.fm — no OpenSSL pain |
| XML (Subsonic) | **quick-xml** | Fast, streaming-capable |
| ZIP | **zip** (deflate) | Streamed downloads |
| Crypto | **sha2, hmac, pbkdf2, rand, fernet** crates | Bit-compatible with existing hashes/Fernet key |
| Subprocess | **tokio::process** | ffmpeg + **yt-dlp as system binary** (drops `extra-pkgs` hack) |
| Config | **serde** + env > `/etc/moidify/config.json` > defaults | Same precedence as Python |
| Errors | **thiserror** + `IntoResponse` | One error type → correct status codes everywhere |
| CLI | **Keep bash** for v1 | `moidify` scripts talk HTTP; no rewrite needed day one |

### Auth as type-state (the big correctness win)

Python required a manual `_require_admin(token)` call in every handler — easy to forget. In axum:

```rust
async fn delete_user(
    admin: AdminUser,          // FromRequestParts → 401/403 automatically
    Path(id): Path<i64>,
    State(app): State<AppState>,
) -> Result<Json<()>, AppError> { ... }
```

Every protected route **cannot compile without** the extractor. That eliminates a whole bug class we just spent a session fixing in Python.

---

## 3. Architecture (re-designed, not translated)

### 3.1 Layout

```
moidify/
├── Cargo.toml
├── src/
│   ├── main.rs              # config load, db init, spawn scanner/watcher, serve
│   ├── config.rs            # typed Config, env/json precedence
│   ├── error.rs             # AppError → (status, JSON {detail})  [frontend-compatible]
│   ├── state.rs             # AppState { pool, config, scan, jobs, rate_limiter }
│   ├── db/
│   │   ├── mod.rs           # pool setup, PRAGMAs (WAL, NORMAL, FK)
│   │   ├── schema.rs        # CREATE TABLE IF NOT EXISTS + column-add migrations
│   │   └── models.rs        # Track, User, Playlist, … (serde Serialize)
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── password.rs      # PBKDF2-SHA256 600k (compat), policy, timing-safe eq
│   │   ├── session.rs       # token issue/lookup (SHA-256 stored), 30-day expiry
│   │   ├── extractors.rs    # User, AdminUser, OptionalUser
│   │   └── ratelimit.rs     # sliding window + max-keys eviction
│   ├── services/
│   │   ├── scanner.rs       # actor: full scan, clean scan, status via watch channel
│   │   ├── watcher.rs       # notify → debounce → scan messages
│   │   ├── metadata.rs      # lofty extract, file_hash, cover save, .lrc fallback
│   │   ├── lyrics.rs        # LRCLIB client + cache
│   │   ├── artist_image.rs  # Deezer + disk cache + SVG fallback
│   │   ├── lastfm.rs        # api_sig = md5(sorted k/v + secret)
│   │   ├── transcode.rs     # ffmpeg spawn, semaphore(10), chunked pipe
│   │   ├── zip_download.rs  # spooled temp file → streaming zip
│   │   ├── imports.rs       # yt-dlp jobs (mpsc + progress in DashMap)
│   │   └── resources.rs     # /proc CPU/mem/disk (no psutil)
│   ├── routes/
│   │   ├── mod.rs           # Router::new().merge(...)
│   │   ├── pages.rs         # /, /setup, /admin, /s/{t}, /a/{t}, /track/{id}
│   │   ├── auth.rs          # register/login/me/password/logout/setup
│   │   ├── tracks.rs        # listings, lyrics, gain, similar, home, duplicates
│   │   ├── streaming.rs     # stream, cover, downloads
│   │   ├── playlists.rs     # playlists, folders, favorites, shares, play, import/export
│   │   ├── player.rs        # player state CRUD
│   │   ├── lastfm.rs
│   │   ├── admin/
│   │   │   ├── mod.rs       # admin router assembly
│   │   │   ├── users.rs     # user CRUD, sessions, unlock, registration toggle
│   │   │   ├── library.rs   # upload, metadata edit, delete, lyrics ops, covers
│   │   │   ├── system.rs    # stats, dashboard, db, resources, server-info, scanner, rescan
│   │   │   └── imports.rs   # import-url, import-album, search-album, jobs
│   │   └── subsonic/
│   │       ├── mod.rs       # /rest/* router, .view stripping, f=xml|json|jsonp
│   │       ├── auth.rs      # t/s token, p= plaintext, enc: hex, rate limit
│   │       ├── ids.rs       # ar_/al_ md5 id scheme
│   │       └── handlers.rs  # ping, browse, stream, search, playlists, star, …
│   └── util/
│       ├── security.rs      # headers layer
│       ├── html.rs          # /track/{id} share page (escape!)
│       └── paths.rs         # within_music_dir, safe names
├── tests/                   # contract tests (port of test_api.py)
├── static/                  # unchanged frontend
├── rust/… (nothing — code at repo root Cargo.toml during dev)
└── … existing Python files until cutover
```

**Admin is split by domain from day one** — this is the structural fix Python never got.

### 3.2 AppState (replaces Python globals)

```rust
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
    pub scan: ScanHandle,      // watch::Receiver<ScanStatus> + cmd mpsc
    pub jobs: Arc<JobRegistry>,// DashMap<job_id, JobProgress>
    pub rate: Arc<RateLimiter>,
    pub fernet: Arc<Fernet>,   // loaded once from .moidify_secret_key
    pub transcode_sem: Arc<Semaphore>, // 10
}
```

- **Scanner** = spawned task; commands via `mpsc` (`Full`, `Clean`, `Process(path)`); status = `tokio::sync::watch` (readers get snapshots without locks).
- **Download jobs** = per-job task + `DashMap`; progress polled same as Python (`GET /api/admin/import-status/{id}`).
- No `threading` + global dict + `LOCK` soup.

### 3.3 Request pipeline (tower layers, outermost first)

1. `TimeoutLayer` (optional, generous — streams are long)
2. `CorsLayer` (mirror `*` behavior — documented limitation)
3. Security headers layer (same header values as `server.py`)
4. Compression (gzip/br) — **new win**, Python never had it
5. `ServeDir` for `/static` + fallback HTML routes
6. Routers (auth extractors live on handlers)

Subsonic `.view` stripping: do it in the `/rest` router with a middleware/`map_request` instead of mutating `request.scope`.

### 3.4 Streaming design

| Mode | Implementation |
|---|---|
| `quality=original` | `tower_http::services::ServeFile` — **native Range/206**, `Accept-Ranges` (Starlette parity) |
| `high/medium/low/voice` | `tokio::process::Command` ffmpeg → stdout → `ReaderStream`, 64 KiB chunks, `X-Transcoded` header, `Semaphore(10)` |
| Path safety | `canonicalize` + `starts_with(MUSIC_DIR)` → else 403 |
| ZIP | temp file ≤8 MiB in memory then spill (same as Python), `Background` drop closes; **also accept `?token=` query** (fixes the `<a href>` 401 bug) |

### 3.5 Compatibility locks (must not break)

These are **API/DB contracts** — tests enforce them:

1. **Auth header is literally `token`** (not `Authorization`) — `localStorage['moidify_token']`
2. **Password hash**: `pbkdf2_hmac(sha256, pw, salt, 600_000)` hex + `token_hex(16)` salt
3. **Subsonic**: `t = md5(password + salt)` vs Fernet-decrypted copy; `p=` plaintext fallback + backfill; `enc:` hex; error code 40; rate limit 30/900
4. **Sessions**: raw token `token_urlsafe(48)`; store `sha256(token)` in both `token` and `token_hash`; 30-day expiry
5. **Same SQLite schema** — existing `music.db` works unchanged (column-add migrations included)
6. **Same JSON shapes** (`{detail: ...}` on errors, list vs paginated tracks, etc.)
7. **Anonymous by design**: `/api/stream/*`, listings, `/api/play/{id}` stay open
8. **Security fixes from the Python pass** all ported: no admin auto-promote, registration gate pre-setup, lyrics PUT auth, share-status auth, timing-safe compares, rate-limit eviction, encrypted Subsonic copy everywhere

### 3.6 Deliberate fixes while rewriting (not blind ports)

| Issue in Python | Rust plan |
|---|---|
| ZIP endpoints require `token` header but UI uses `<a href>` | Accept `?token=` **or** `token` header on download routes only |
| `GET /api/admin/scanner` polled by player UI without token → always 401 | Frontend: only poll when admin (1-line JS fix at cutover) |
| `getPlaylists` returns all users' playlists | Filter by authenticated Subsonic user |
| No shutdown teardown | Graceful shutdown: drop watcher, drain scan, close pool |
| `extra-pkgs` yt-dlp sys.path hack | Require `yt-dlp` on `PATH` (install.sh updated) |
| Admin 1,489-line module | Split `admin/{users,library,system,imports}` |

---

## 4. Phased delivery

Each phase ends with **compiling binary + passing tests**. Frontend stays runnable against the Rust server from Phase 2 onward (dogfood early).

| Phase | Deliverable | Tests |
|---|---|---|
| **0 — Scaffold** | `cargo init`, deps, `config`, `db/schema` + migrations, `AppState`, error type, `GET /health`, CI workflow | schema idempotent on fresh+existing DB |
| **1 — Auth core** | setup/init, login, me, logout, logout-all, change password, register gate, rate limit, lockout, Fernet key, extractors | port: setup flow, login, no auto-promote, registration gating, password not stored plaintext |
| **2 — Read API** | tracks/albums/artists/genres/home/duplicates/similar/gain, static pages, `/track/{id}` share HTML, cover placeholder | empty-library listings, page content-types |
| **3 — Streaming** | `/api/stream` (original + 4 transcode tiers), `/api/cover`, path safety | no-auth stream, quality headers, path traversal 403 |
| **4 — User data** | playlists CRUD/reorder/share/export/import, folders, favorites, player state, play counter, album shares | ownership 403s, share-status 401, export/import roundtrip |
| **5 — Scanner** | metadata (lofty), file_hash, covers, .lrc, full/clean scan, notify watcher, rescan scheduler | scan empty dir, process fixture files, hash format parity |
| **6 — Admin** | split modules: users, upload, metadata edit, lyrics ops, stats/dashboard, db tools, resources, server-info, registration toggle, unlock/sessions | all admin 401/403, registration toggle fix |
| **7 — Subsonic** | full `/rest/*` (23 paths), xml+json+jsonp, id scheme, auth matrix, scrobble/star/search/browse | full auth matrix from Python suite |
| **8 — Integrations** | ZIP downloads (+`?token=`), Last.fm, yt-dlp import jobs, LRCLIB bulk scan, Deezer images | download auth, lastfm sig, job lifecycle |
| **9 — Cutover** | Dockerfile (rust-slim/distroless), install.sh/update.sh/systemd, `version.txt`, README, remove Python, port remaining tests, CI = `cargo test` | full suite green; manual smoke: setup→upload→stream→subsonic client |

**Ordering rationale:** auth first (everything depends on extractors); read API before writes (frontend needs listings to look alive); scanner before admin (admin UI shows scan status); Subsonic after core DB models stabilize; integrations last (least coupled).

---

## 5. Testing strategy

- **Contract suite** in `tests/` using `axum::Router` + `tower::ServiceExt::oneshot` (no real port bind) or `reqwest` against a spawned server on a random port.
- Port every `test_api.py` case 1:1, especially the **security regressions** from the Python hardening pass.
- Fixture: temp dir + temp SQLite + `init_schema()` — same as pytest fixture.
- CI: GitHub Actions `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` (replace Python matrix).
- Optional later: generate OpenAPI from axum and diff against captured Python responses for shape parity.

---

## 6. Deployment changes

| Artifact | Change |
|---|---|
| `Dockerfile` | `FROM rust:1.8x AS build` → `debian:bookworm-slim` (+ ffmpeg, yt-dlp, ca-certificates); multi-stage; non-root `moidify` |
| `docker-compose.yml` | same volumes/env/healthcheck (`/api/home` or `/health`) |
| `install.sh` | drop python/pip/venv steps; install Rust release binary to `/opt/moidify/bin/moidify-server`; keep ffmpeg + **yt-dlp via pipx/apt or official binary** |
| `update.sh` | build or fetch release binary instead of pip |
| `moidify.service` | `ExecStart=/opt/moidify/bin/moidify-server` |
| `start.sh` (dev) | `cargo run --release` |
| `.github/workflows` | rust toolchain, cache `~/.cargo` + `target/` |
| `requirements.txt` / Python files | deleted at Phase 9 |

---

## 7. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Subsonic client quirks (xml namespaces, `.view`, jsonp) | Port Python handlers against captured fixtures; test with Sonixd/Sublime manually in Phase 7 |
| lofty tag differences vs mutagen | Golden-file tests: same MP3/FLAC/OGG/MP4 fixtures → same extracted fields |
| Fernet key incompatibility | Test vector: encrypt with Python Fernet, decrypt with `fernet` crate (and vice versa) in Phase 1 |
| Long dual-running period | Phases 2+ are runnable; switch `docker-compose`/systemd only at Phase 9 |
| Scope creep in admin | Keep admin endpoints 1:1; no new admin features during rewrite |
| yt-dlp missing on host | Clear startup warning + admin `server-info` shows path/version |

---

## 8. Explicit non-goals (this rewrite)

- No frontend rewrite / no framework
- No new features (only the listed bugfixes)
- No bash CLI rewrite (Phase 10+ if ever)
- No Postgres/MySQL — stays SQLite
- No multi-worker horizontal scale (single process, like Python; jobs/scan state are in-process by design)
- No cookie sessions — keep `token` header

---

## 9. Success criteria

1. Existing `music.db` + `covers/` + `music/` work with zero migration steps from the user's perspective  
2. All ~40 contract tests pass; security regressions included  
3. Unmodified frontend works against Rust backend  
4. Subsonic clients (Sonixd etc.) authenticate and stream  
5. RSS: RSS < Python (spot-check `ps` RSS after idle + during stream)  
6. `cargo clippy -D warnings` clean  
7. Docker image smaller than Python image (target: <80 MB vs current slim+pip)  

---

## 10. Immediate next step

**Phase 0:** scaffold `Cargo.toml`, module tree, `config.rs`, `db/schema.rs` (byte-compatible tables + column migrations), `error.rs`, `state.rs`, `GET /health`, CI workflow — then Phase 1 auth with the full security test port.
