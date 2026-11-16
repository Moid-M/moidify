# Moidify — recommendations & future work (kept for later)

These are NOT committed tasks; they are ideas to revisit when time permits.

## Reliability / backend
- Background job queue + WebSocket progress for scanning, transcoding, and lyrics fetching (scan-all currently blocks the request handler).
- Pluggable lyrics providers (LRCLIB, embedded tags, manual) with confidence scoring and caching; respect LRCLIB throttling (300 ms, Retry-After).
- Richer metadata via MusicBrainz/Discogs + album/artist images (Deezer/Last.fm already referenced).
- Multi-user permissions, sharing, and collaborative playlists.
- ffmpeg tier caching and on-the-fly transcode profiles; avoid re-transcoding.
- Metrics/observability (Prometheus/tracing) and health checks.
- Automated integration tests against real Subsonic clients (Sonixd, Sublime, Symfonium, DSub).

## Frontend / UX
- Cinema View polish: word-level SpicyLyrics-style animation, blurred album-color background, cover placement, title/artist.
- Fix desync: one song playing but another shown; progress bar stuck at 0.
- Rework UI/UX with fades and animations; mobile/PWA + offline cache.
- Lyrics editor improvements and source attribution.

## Architecture
- Separate API (axum) and scanner into worker processes communicating via queue.
- Replace compile-time static path (`env!("CARGO_MANIFEST_DIR")`) with runtime config path for portability.
- Centralize error handling and consistent JSON shapes; reduce `unwrap`/`expect`.
- Add DB migrations versioning and backup/restore commands.

## Subsonic
- Keep id/SQL parity with Python; verify against real clients.
- Investigate empty-list omission vs. clients expecting arrays.
## Lyrics normalization
- Add a lyrics manager that normalizes LRC/Lyricsfile pauses: merge consecutive empty/silence lines and implicit gaps between lines into a single silence row, so the player shows one stable pause instead of flickering or duplicate silences.
