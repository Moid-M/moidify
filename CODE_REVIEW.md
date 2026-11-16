# Moidify code review (static pass, no compile)

## Backend (Rust)

- `src/routes/tracks.rs`: `list_albums` now groups by album name only; `list_album_tracks` ignores the artist filter. Watch for collisions when two unrelated artists share an album name.
- `src/routes/subsonic/browse.rs`: album grouping similarly changed to `GROUP BY album` with `MIN(artist)`. `ARTIST_ALBUMS` uses `HAVING MIN(...) = ?`, so an album is attributed to its lexicographically smallest artist; featuring artists will not own the album. `ALBUM_TRACKS` no longer filters by artist.
- `src/services/lyrics.rs`: new LRCLIB word-sync path + embedded-tag fallback. `serde_yaml` and `lofty` are used; not yet compiled. Throttle is a global 300 ms mutex; `scan_all_lyrics` will take ~4 min for 800 tracks.
- `src/routes/tracks.rs::get_lyrics`: now reads `file_path` and `duration`; ensure the row decode matches the query (it does).
- `src/routes/admin/library.rs`: embedded fallback added; duration not passed (None) to LRCLIB, which may reduce match accuracy. Consider passing duration.
- `src/routes/subsonic/media.rs`: POST/PUT coerced to GET for ServeFile; Range requests still pass through.
- `src/routes/subsonic/auth.rs`: `require` returns a large `Response` in `Err`; allowed via crate-level lint.
- `src/routes/streaming.rs`: `within_music_dir`, `cover_mime`, `file_response` are `pub(crate)` for subsonic reuse.

## Frontend

- `static/js/player.js::updatePlayerUI`: highlights `.track-row` by `state.currentIndex`, which is a queue index, not a DOM index. This can highlight the wrong row when the queue order differs from the rendered list. Fix by matching `data-track-id`.
- Progress bar: updates only when `audio.duration` is truthy; `loadedmetadata` may not fire for cached/streamed responses. Consider also listening to `durationchange` and `canplay`.
- `fetchSimilarAndAppend` appends similar tracks and plays index `prevLen`; if the append raced with another update, index math can drift.
- Cinema View: cover URL uses `track.id || track.track_id`; ensure queue tracks always carry `id`.
- Lyrics word-level: `parseLyricWords` regex assumes `<mm:ss.xx>` tags; LRC without word tags falls back to line-level (correct).
- `updateNowPlayingLyricsSync` and `updateSyncedLyrics` both manipulate word classes; keep them in sync to avoid stale highlights.

## General

- Many `unwrap`/`expect` remain (mostly tests and infallible paths); review before release.
- Compile-time static path (`env!("CARGO_MANIFEST_DIR")`) should become runtime config for portability.
- No DB migration versioning yet.
EOF
echo done
---

## Backend agent findings (full pass)

### Compile blockers
- `src/services/lyrics.rs:11-12` — `tokio::sync::Mutex::new` in a static is not const; use `std::sync::Mutex` or `LazyLock`.
- `src/services/lyrics.rs:153` — `TaggedFileExt` trait not in scope for `primary_tag`/`first_tag`.

### Correctness / logic
- `src/routes/tracks.rs:335-349`, `:423-432` — album grouping by name only; unrelated artists collide.
- `src/routes/subsonic/browse.rs:36-56` — same grouping; album attributed to lexicographically smallest artist.
- `src/routes/subsonic/browse.rs:58-61` — `ALBUM_TRACKS` filters only by album.
- `src/routes/subsonic/browse.rs:362-396` — `get_playlist` no ownership check (IDOR).
- `src/scanner/mod.rs:108-118` — durations truncated to whole seconds (parity with Python).
- `src/auth/password.rs:460-476` — failed-attempt exponentiation can overflow; cap attempts.

### Security
- `src/lib.rs:30` — `CorsLayer::permissive()`.
- `src/routes/pages.rs:184-195` — host/proto injection via `Host`/`X-Forwarded-Proto`.
- `src/routes/pages.rs:110`, `src/routes/streaming.rs:115-120` — TOCTOU symlink swap.
- `src/routes/admin/library.rs:113-125` — small-file rejection; early return on oversized.
- `src/routes/admin/library.rs:296-327` — dynamic SQL + silent coercion to 0.

### SQL / data
- `src/db/schema.rs:97` — table name interpolation in PRAGMA.
- `src/db/connect.rs:12-23` — no busy_timeout.
- `src/routes/tracks.rs:128-148` — N+1 fetch in search.

### Auth
- `src/auth/mod.rs:30-44` — insecure existing Fernet key file not corrected.
- `src/auth/extractors.rs:58-75` — duplicated AdminUser auth logic.
- `src/auth/session.rs:38-40` — global DELETE on every token lookup.
- `src/db/models.rs:30-36` — `to_user()` drops email; unused dead code.

### Architecture / dead code / unsafe
- `src/lib.rs:20`, `src/routes/pages.rs:25`, `src/routes/streaming.rs:278`, `src/config.rs:33/188` — compile-time paths.
- `src/routes/mod.rs:10`, `src/routes/lastfm.rs` — lastfm module never merged.
- `src/auth/mod.rs:79-82` — `shared_fernet` unused.
- `src/routes/tracks.rs:263-270` — `get_gain` TODO returns null.
- `src/services/resources.rs:30,220`, `src/routes/admin/system.rs:146` — lock unwrap / NUL panics.
- `src/routes/streaming.rs:136`, `src/routes/subsonic/media.rs:108`, `src/routes/tracks.rs:386` — expect on infallible paths.
- `src/scanner/mod.rs:651` — watcher lock expect.

## Frontend agent findings (full pass)

### Player desync / playback
- `static/js/player.js:19-20` — play before metadata; progress can stay 0.
- `static/js/player.js:28-53` — one-shot loadedmetadata can fail to fire.
- `static/js/player.js:469-482` — gapless preload race.
- `static/js/player.js:512-560` — crossfade swaps src without metadata readiness.
- `static/js/player.js:103-113` — fetchSimilarAndAppend race.
- `static/js/app.js:490-526`, `:563-604` — session restore metadata timing.

### Progress bar
- `static/js/app.js:280-304` — smooth-seek mode skips seek-bar value updates.
- `static/js/app.js:451-462` — smooth mode omits seek value.
- `static/js/app.js:325-338` — late duration leaves bar at 0.
- `static/js/player.js:396-402` — cinema progress returns early on no duration.

### Lyrics sync
- `static/js/lyrics.js:209-210` — early return on `!audio.currentTime` blocks sync at start.
- `static/js/lyrics.js:108-118` — unsynced lines with time -1 can become active.
- `static/js/player.js:429-454` — duplicate sync logic can leave stale highlights.
- `static/js/lyrics.js:42-54` — parseLyricWords only handles word tags.

### Queue handling
- `static/js/queue.js:2` — addTrackToQueueNext resets queue when currentIndex < 0.
- `static/js/queue.js:28-34` — removeFromQueue current index calls nextTrack but doesn't remove current.
- `static/js/queue.js:64-73` — autoShowQueue auto-opens queue panel.
- `static/js/queue.js:123-141` — shuffle queue view depends on matching lengths.
- `static/js/player.js:128-149` — shuffle nextTrack index range after autoplay append.

### Dead code / incomplete features
- `static/js/icons.js` — several unused icons.
- `static/js/views/tracks.js:27-37` — VirtualList is a shim.
- `static/js/views/tracks.js:244-250` — `_mergeAll` never invoked.
- `static/js/ui/sleep-timer.js:34-36` — empty timer branches.
- `static/js/ui/context-menu.js:192-206` — global long-press timer conflict.

### UI / HTML
- `static/index.html:233`, `:260-262`, `:324` — empty/invalid aria-labels.
- `static/shared.html:100`, `static/shared-album.html:111` — timeupdate listener leaks.
- `static/js/ui/settings.js:413` — optional chaining browser compat.

### Playback / crossfade
- Skipping mid-track during crossfade can leave the progress bar stuck; the new track's bar is not reset to 0 before starting. Reset `seek`/`--seek-pct`/time labels on track change.
