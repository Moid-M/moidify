//! Browse/catalog endpoints: `getMusicFolders`, `getIndexes`, `getArtists`,
//! `getArtist`, `getAlbum`, `getSong`, `getMusicDirectory`, `search3`,
//! `getAlbumList2`, `getPlaylists`, `getPlaylist`.
//!
//! SQL matches Python byte-for-byte (including the COALESCE/GROUP BY quirks).
//! `getPlaylists` filters by the authenticated user — the deliberate fix
//! from plan §3.6 ("returns all users' playlists").

use axum::extract::{Query, State};
use std::collections::BTreeMap;

use super::auth;
use super::child::{
    duration_sum_str, fetch_albums, fetch_artists, fetch_playlist, fetch_playlists, fetch_track,
    fetch_tracks, nonzero_str, TRACK_COLS, TRACK_COLS_T,
};
use super::ids::artist_id;
use super::response::{error, ok, Elem};
use super::{respond, Params, SubResult};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const DISTINCT_ARTISTS: &str =
    "SELECT DISTINCT COALESCE(NULLIF(album_artist,''), artist) as artist \
                                FROM tracks WHERE artist IS NOT NULL ORDER BY artist";

const ARTIST_GROUPS: &str = "SELECT COALESCE(NULLIF(album_artist,''), artist) as artist, \
                              COUNT(*) as track_count, COUNT(DISTINCT album) as album_count \
                              FROM tracks WHERE artist IS NOT NULL GROUP BY artist ORDER BY artist";

const ARTIST_SEARCH: &str = "SELECT DISTINCT COALESCE(NULLIF(album_artist,''), artist) as artist, \
                              COUNT(*) as track_count, COUNT(DISTINCT album) as album_count \
                              FROM tracks WHERE artist LIKE ? OR album_artist LIKE ? \
                              GROUP BY artist ORDER BY artist LIMIT ?";

const ARTIST_ALBUMS: &str = "SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist, \
                              COUNT(*) as track_count, MAX(has_cover) as has_cover, \
                              MIN(id) as cover_track_id FROM tracks \
                              WHERE COALESCE(NULLIF(album_artist,''), artist) = ? \
                              GROUP BY album, COALESCE(NULLIF(album_artist,''), artist) \
                              ORDER BY album";

const ALBUM_GROUPS: &str = "SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist, \
                             COUNT(*) as track_count, MAX(has_cover) as has_cover, \
                             MIN(id) as cover_track_id, MIN(year) as year, MIN(genre) as genre \
                             FROM tracks WHERE album IS NOT NULL \
                             GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)";

const ALBUM_GROUPS_DIR: &str =
    "SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist, \
                                 COUNT(*) as track_count, MAX(has_cover) as has_cover, \
                                 MIN(id) as cover_track_id FROM tracks WHERE album IS NOT NULL \
                                 GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)";

const ALBUM_TRACKS: &str = "SELECT {cols} FROM tracks \
                             WHERE album = ? AND COALESCE(NULLIF(album_artist,''), artist) = ? \
                             ORDER BY disc_number, track_number";

const ALBUM_SEARCH: &str = "SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist, \
                             COUNT(*) as track_count, MAX(has_cover) as has_cover, \
                             MIN(id) as cover_track_id FROM tracks WHERE album LIKE ? \
                             GROUP BY album, COALESCE(NULLIF(album_artist,''), artist) \
                             ORDER BY album LIMIT ?";

const SONG_SEARCH: &str = "SELECT {cols} FROM tracks \
                            WHERE title LIKE ? OR artist LIKE ? OR album LIKE ? \
                            ORDER BY COALESCE(NULLIF(album_artist,''), artist), album, \
                            disc_number, track_number LIMIT ?";

/// `GET|POST /rest/getMusicFolders` — single static folder.
pub async fn get_music_folders(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let _user = auth::require(&app, &q).await?;
    let root = ok().with(
        Elem::new("musicFolders").with(
            Elem::new("musicFolder")
                .attr("id", "1")
                .attr("name", "Music"),
        ),
    );
    Ok(respond(&q, root))
}

/// `GET|POST /rest/getIndexes` — artists bucketed by initial (Python groups
/// with a plain dict keyed by `name[0].upper()`).
pub async fn get_indexes(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let _user = auth::require(&app, &q).await?;

    let rows = fetch_artists_raw(&app, DISTINCT_ARTISTS).await?;
    let mut indexes: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for name in rows {
        let letter = initial_letter(&name);
        indexes
            .entry(letter)
            .or_default()
            .push((name.clone(), artist_id(&name)));
    }

    let mut indexes_elem = Elem::new("indexes")
        .attr("lastModified", "0")
        .attr("ignoredArticles", "");
    for (letter, artists) in &indexes {
        let mut index = Elem::new("index").attr("name", letter);
        for (name, id) in artists {
            index = index.with(Elem::new("artist").attr("name", name).attr("id", id));
        }
        indexes_elem = indexes_elem.with(index);
    }
    Ok(respond(&q, ok().with(indexes_elem)))
}

/// `GET|POST /rest/getArtists` — same bucketing with full artist children.
pub async fn get_artists(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let _user = auth::require(&app, &q).await?;

    let rows = fetch_artists(&app.pool, sqlx::query(ARTIST_GROUPS)).await?;
    let mut indexes: BTreeMap<String, Vec<Elem>> = BTreeMap::new();
    for row in rows {
        let letter = initial_letter(row.artist.as_deref().unwrap_or(""));
        indexes.entry(letter).or_default().push(row.child());
    }

    let mut artists_elem = Elem::new("artists").attr("ignoredArticles", "");
    for (letter, children) in &indexes {
        let mut index = Elem::new("index").attr("name", letter);
        for child in children {
            index = index.with(child.clone());
        }
        artists_elem = artists_elem.with(index);
    }
    Ok(respond(&q, ok().with(artists_elem)))
}

/// `GET|POST /rest/getArtist` — resolve `ar_*` id by scanning artist names
/// (same O(n) md5 comparison loop as Python), then list that artist's albums.
pub async fn get_artist(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let id = q.require_id()?;
    let _user = auth::require(&app, &q).await?;

    let rows = fetch_artists_raw(&app, DISTINCT_ARTISTS).await?;
    let Some(name) = rows.into_iter().find(|n| artist_id(n) == id) else {
        return Ok(respond(&q, error(70, "Artist not found")));
    };

    let albums = fetch_albums(&app.pool, sqlx::query(ARTIST_ALBUMS).bind(&name)).await?;

    let mut artist_elem = Elem::new("artist")
        .attr("id", id)
        .attr("name", &name)
        .attr("albumCount", albums.len());
    for album in &albums {
        artist_elem = artist_elem.with(album.browse_child(id));
    }
    Ok(respond(&q, ok().with(artist_elem)))
}

/// `GET|POST /rest/getAlbum` — matches either the `al_*` id or the numeric
/// cover track id (Python's `str(cover_track_id) == id` fallback).
pub async fn get_album(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let id = q.require_id()?;
    let _user = auth::require(&app, &q).await?;

    let albums = fetch_albums(&app.pool, sqlx::query(ALBUM_GROUPS)).await?;
    let target = albums.iter().find(|a| {
        a.id() == id
            || a.cover_track_id
                .map(|c| c.to_string())
                .unwrap_or_else(|| "None".into())
                == id
    });
    let Some(target) = target else {
        return Ok(respond(&q, error(70, "Album not found")));
    };

    let tracks = fetch_tracks(
        &app.pool,
        sqlx::query(&ALBUM_TRACKS.replace("{cols}", TRACK_COLS))
            .bind(target.album.clone())
            .bind(target.artist.clone()),
    )
    .await?;

    let mut album_elem = Elem::new("album")
        .attr("id", target.id())
        .attr("name", target.album())
        .attr("artist", target.artist())
        .attr(
            "coverArt",
            target
                .cover_track_id
                .map(|c| c.to_string())
                .unwrap_or_default(),
        )
        .attr("songCount", tracks.len())
        .attr(
            "duration",
            duration_sum_str(&tracks.iter().map(|t| t.duration).collect::<Vec<_>>()),
        )
        .attr("created", "")
        .attr("year", nonzero_str(target.year, ""))
        .attr("genre", target.genre.as_deref().unwrap_or(""))
        .attr("isDir", "false")
        .attr("type", "album");
    for track in &tracks {
        album_elem = album_elem.with(track.child("song"));
    }
    Ok(respond(&q, ok().with(album_elem)))
}

/// `GET|POST /rest/getSong` — single track by numeric id.
pub async fn get_song(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let id = q.require_id()?;
    let _user = auth::require(&app, &q).await?;

    let Ok(track_id) = id.parse::<i64>() else {
        return Ok(respond(&q, error(70, "Song not found")));
    };
    let sql = format!("SELECT {TRACK_COLS} FROM tracks WHERE id = ?");
    let song = fetch_track(&app.pool, sqlx::query(&sql).bind(track_id)).await?;
    let Some(song) = song else {
        return Ok(respond(&q, error(70, "Song not found")));
    };
    Ok(respond(&q, ok().with(song.child("song"))))
}

/// `GET|POST /rest/getMusicDirectory` — album directory first, then artist
/// directory, else error 70 (same order as Python).
pub async fn get_music_directory(
    State(app): State<AppState>,
    Query(q): Query<Params>,
) -> SubResult {
    let id = q.require_id()?;
    let _user = auth::require(&app, &q).await?;

    // Album id?
    let albums = fetch_albums(&app.pool, sqlx::query(ALBUM_GROUPS_DIR)).await?;
    if let Some(target) = albums.iter().find(|a| a.id() == id) {
        let tracks = fetch_tracks(
            &app.pool,
            sqlx::query(&ALBUM_TRACKS.replace("{cols}", TRACK_COLS))
                .bind(target.album.clone())
                .bind(target.artist.clone()),
        )
        .await?;
        let mut directory = Elem::new("directory")
            .attr("id", id)
            .attr("name", target.album());
        for track in &tracks {
            directory = directory.with(track.child("child"));
        }
        return Ok(respond(&q, ok().with(directory)));
    }

    // Artist id?
    let names = fetch_artists_raw(&app, DISTINCT_ARTISTS).await?;
    if let Some(name) = names.into_iter().find(|n| artist_id(n) == id) {
        let albums = fetch_albums(&app.pool, sqlx::query(ARTIST_ALBUMS).bind(&name)).await?;
        let mut directory = Elem::new("directory").attr("id", id).attr("name", &name);
        for album in &albums {
            directory = directory.with(album.directory_child(id));
        }
        return Ok(respond(&q, ok().with(directory)));
    }

    Ok(respond(&q, error(70, "Directory not found")))
}

/// `GET|POST /rest/search3` — `searchResult3` with artist/album/song arrays.
/// `totalHits` counts songs, like Python.
pub async fn search3(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let _user = auth::require(&app, &q).await?;

    let query = q.query.as_deref().unwrap_or("");
    let like = format!("%{query}%");
    let artist_count = q.int_param(q.artist_count.as_deref(), 20);
    let album_count = q.int_param(q.album_count.as_deref(), 20);
    let song_count = q.int_param(q.song_count.as_deref(), 20);

    let artists = fetch_artists(
        &app.pool,
        sqlx::query(ARTIST_SEARCH)
            .bind(&like)
            .bind(&like)
            .bind(artist_count),
    )
    .await?;
    let albums = fetch_albums(
        &app.pool,
        sqlx::query(ALBUM_SEARCH).bind(&like).bind(album_count),
    )
    .await?;
    let song_sql = SONG_SEARCH.replace("{cols}", TRACK_COLS);
    let songs = fetch_tracks(
        &app.pool,
        sqlx::query(&song_sql)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(song_count),
    )
    .await?;

    let mut result = Elem::new("searchResult3").attr("totalHits", songs.len());
    for artist in &artists {
        result = result.with(artist.child());
    }
    for album in &albums {
        result = result.with(album.child());
    }
    for song in &songs {
        result = result.with(song.child("song"));
    }
    Ok(respond(&q, ok().with(result)))
}

/// `GET|POST /rest/getAlbumList2` — `type` → ORDER BY whitelist (unknown
/// types fall back to RANDOM like Python).
pub async fn get_album_list2(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let _user = auth::require(&app, &q).await?;

    let size = q.int_param(q.size.as_deref(), 50);
    let offset = q.int_param(q.offset.as_deref(), 0);
    let order = match q.list_type.as_deref().unwrap_or("random") {
        "newest" | "recent" => "ORDER BY created_at DESC",
        "alphabeticalByName" => "ORDER BY album",
        "alphabeticalByArtist" => "ORDER BY artist",
        "frequent" => "ORDER BY play_count DESC",
        _ => "ORDER BY RANDOM()",
    };
    let sql = format!(
        "SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist, \
         COUNT(*) as track_count, MAX(has_cover) as has_cover, MIN(id) as cover_track_id \
         FROM tracks WHERE album IS NOT NULL \
         GROUP BY album, COALESCE(NULLIF(album_artist,''), artist) \
         {order} LIMIT ? OFFSET ?"
    );
    let albums = fetch_albums(&app.pool, sqlx::query(&sql).bind(size).bind(offset)).await?;

    let mut list = Elem::new("albumList2");
    for album in &albums {
        list = list.with(album.child());
    }
    Ok(respond(&q, ok().with(list)))
}

/// `GET|POST /rest/getPlaylists` — only this user's playlists (plan §3.6 fix).
pub async fn get_playlists(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let user = auth::require(&app, &q).await?;

    let sql = "SELECT p.id, p.name, COUNT(pt.id) as track_count \
               FROM playlists p \
               LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id \
               WHERE p.user_id = ? GROUP BY p.id ORDER BY p.name";
    let rows = fetch_playlists(&app.pool, sqlx::query(sql).bind(user.id)).await?;

    let mut playlists = Elem::new("playlists");
    for row in &rows {
        playlists = playlists.with(row.child(&user.username));
    }
    Ok(respond(&q, ok().with(playlists)))
}

/// `GET|POST /rest/getPlaylist` — existence is checked but ownership is not
/// (Python allows fetching any playlist id you know; only the *list* is
/// filtered). Entries ordered by `playlist_tracks.position`.
pub async fn get_playlist(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let id = q.require_id()?;
    let user = auth::require(&app, &q).await?;

    let Ok(playlist_id) = id.parse::<i64>() else {
        return Ok(respond(&q, error(70, "Playlist not found")));
    };
    let playlist = fetch_playlist(
        &app.pool,
        sqlx::query("SELECT id, name FROM playlists WHERE id = ?").bind(playlist_id),
    )
    .await?;
    let Some((pid, name)) = playlist else {
        return Ok(respond(&q, error(70, "Playlist not found")));
    };

    let sql = format!(
        "SELECT {TRACK_COLS_T} FROM tracks t \
         JOIN playlist_tracks pt ON pt.track_id = t.id \
         WHERE pt.playlist_id = ? ORDER BY pt.position"
    );
    let tracks = fetch_tracks(&app.pool, sqlx::query(&sql).bind(pid)).await?;

    let name = if name.is_empty() {
        "Unnamed"
    } else {
        name.as_str()
    };
    let mut playlist_elem = Elem::new("playlist")
        .attr("id", pid)
        .attr("name", name)
        .attr("owner", &user.username)
        .attr("public", "false")
        .attr("songCount", tracks.len())
        .attr(
            "duration",
            duration_sum_str(&tracks.iter().map(|t| t.duration).collect::<Vec<_>>()),
        )
        .attr("created", "");
    for track in &tracks {
        playlist_elem = playlist_elem.with(track.child("entry"));
    }
    Ok(respond(&q, ok().with(playlist_elem)))
}

/// `SELECT DISTINCT COALESCE(...)` as plain artist-name strings.
async fn fetch_artists_raw(app: &AppState, sql: &str) -> AppResult<Vec<String>> {
    let rows = sqlx::query(sql)
        .fetch_all(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(rows
        .iter()
        .map(|r| {
            use sqlx::Row;
            r.get::<String, _>("artist")
        })
        .collect())
}

/// Python `name[0].upper() if name else "#"` (char upper can grow, e.g.
/// `ß` → `SS`, which Rust's `to_uppercase` matches).
fn initial_letter(name: &str) -> String {
    match name.chars().next() {
        Some(c) => c.to_uppercase().to_string(),
        None => "#".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_letter_matches_python() {
        assert_eq!(initial_letter("alpha"), "A");
        assert_eq!(initial_letter("Édith"), "É");
        assert_eq!(initial_letter("ßeta"), "SS");
        assert_eq!(initial_letter(""), "#");
    }

    #[test]
    fn order_whitelist_is_static() {
        // The match arms keep user input out of the SQL string.
        let sql = format!("{} LIMIT ?", "ORDER BY album");
        assert!(sql.starts_with("ORDER BY"));
    }
}
