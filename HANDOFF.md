# Moidify — handoff for next session

## Project
Self-hosted music server. Rust port (axum + SQLx) on branch `rust-port` of https://github.com/Moid-M/moidify. Frontend is vanilla JS in `static/`. Subsonic `/rest/*` API included.

## Environment
- Rust toolchain (cargo/rustc). Build: `cargo build --release`.
- Tests: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test`.
- Run: set `MOIDIFY_DB_PATH`, `MOIDIFY_MUSIC_DIR`, `MOIDIFY_COVERS_DIR`, `MOIDIFY_PORT`.
- VM (testing): 192.168.122.124, root/admin123, service `moidify`.

## Current state
- All tests green; release build succeeds.
- Latest commit pushed to `rust-port`.
- Features: Subsonic 23 endpoints, word-level lyrics (LRCLIB Lyricsfile), Cinema View, album grouping fix, progress fixes.
- Review docs: `CODE_REVIEW.md`, `RECOMMENDATIONS.md`.

## Known remaining work
- Compile blockers none.
- Frontend polish: SpicyLyrics-style word animation (letter shake/glow), progress bar fill before dot, overlapping duet lines.
- Lyrics normalization (merge silences).
- Crossfade skip progress reset.
- See `CODE_REVIEW.md` for full list.

## Notes for next agent
- Use Read/Edit/Write tools, not bash cat.
- Do not push without explicit request.
- Run full test gate before claiming done.
- Prefer incremental commits.