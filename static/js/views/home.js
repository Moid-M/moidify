var _homeData = null;

function homeNavAlbum(e) {
  var card = e.target.closest('.home-card');
  if (!card) return;
  var idx = parseInt(card.dataset.idx);
  if (!_homeData || !_homeData.recommended_albums || !_homeData.recommended_albums[idx]) return;
  var a = _homeData.recommended_albums[idx];
  navigate('album', {album: a.album, artist: a.album_artist || a.artist});
}

function homeNavRecent(e) {
  var card = e.target.closest('.home-card');
  if (!card) return;
  var idx = parseInt(card.dataset.idx);
  if (!_homeData || !_homeData.recently_played || !_homeData.recently_played[idx]) return;
  var t = _homeData.recently_played[idx];
  navigate('album', {album: t.album, artist: t.artist});
}

function homePlayTrack(i) {
  if (!_homeData || !_homeData.recommended_tracks) return;
  playFromQueue(_homeData.recommended_tracks, i);
}

async function renderHome(navId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">\uD83C\uDFE0 Home</div></div><div id="home-feed">'+skeletonGrid()+'</div>';

  apiJson('/api/home').then(function(data) {
    if (state._navId !== navId) return;
    _homeData = data;
    var feed = document.getElementById('home-feed');
    feed.innerHTML = '';

    if (data.recently_played && data.recently_played.length) {
      var section = document.createElement('div');
      section.className = 'home-section';
      section.innerHTML = '<div class="home-section-title">Recently played</div><div class="home-grid" id="home-recent-grid"></div>';
      feed.appendChild(section);
      var grid = document.getElementById('home-recent-grid');
      data.recently_played.slice(0, 6).forEach(function(t, i) {
        var card = document.createElement('div');
        card.className = 'home-card';
        card.dataset.idx = i;
        card.innerHTML = '<img class="home-card-img" src="/api/cover/'+t.id+'" loading="lazy" onerror="this.src=\'/static/logo.png\'"><div class="home-card-title">'+esc(t.title||'')+'</div><div class="home-card-sub">'+esc(t.artist||'')+'</div>';
        card.addEventListener('click', homeNavRecent);
        grid.appendChild(card);
      });
    }

    if (data.recommended_albums && data.recommended_albums.length) {
      var section = document.createElement('div');
      section.className = 'home-section';
      section.innerHTML = '<div class="home-section-title">Recommended albums</div><div class="home-grid" id="home-album-grid"></div>';
      feed.appendChild(section);
      var grid = document.getElementById('home-album-grid');
      data.recommended_albums.forEach(function(a, i) {
        var artistName = a.album_artist || a.artist || 'Unknown';
        var card = document.createElement('div');
        card.className = 'home-card';
        card.dataset.idx = i;
        card.innerHTML = '<img class="home-card-img home-card-img-album" src="/api/cover/'+a.cover_track_id+'" loading="lazy" onerror="this.src=\'/static/logo.png\'"><div class="home-card-title">'+esc(a.album||'')+'</div><div class="home-card-sub">'+esc(artistName)+'</div>';
        card.addEventListener('click', homeNavAlbum);
        grid.appendChild(card);
      });
    }

    if (data.recommended_tracks && data.recommended_tracks.length) {
      var section = document.createElement('div');
      section.className = 'home-section';
      section.innerHTML = '<div class="home-section-title">Recommended tracks</div><div class="home-track-list" id="home-track-list"></div>';
      feed.appendChild(section);
      var list = document.getElementById('home-track-list');
      data.recommended_tracks.slice(0, 10).forEach(function(t, i) {
        var dur = formatTime(t.duration);
        var row = document.createElement('div');
        row.className = 'home-track-row';
        row.innerHTML = '<span class="home-track-idx">'+(i+1)+'</span><img class="home-track-cover" src="/api/cover/'+t.id+'" loading="lazy" onerror="this.src=\'/static/logo.png\'"><span class="home-track-title">'+esc(t.title||'')+'</span><span class="home-track-artist">'+esc(t.artist||'')+'</span><span class="home-track-dur">'+dur+'</span>';
        row.addEventListener('click', function() { homePlayTrack(i); });
        list.appendChild(row);
      });
    }

    if (data.recommended_artists && data.recommended_artists.length) {
      var section = document.createElement('div');
      section.className = 'home-section';
      section.innerHTML = '<div class="home-section-title">Recommended artists</div><div class="home-grid" id="home-artist-grid"></div>';
      feed.appendChild(section);
      var grid = document.getElementById('home-artist-grid');
      data.recommended_artists.forEach(function(a) {
        var card = document.createElement('div');
        card.className = 'home-card';
        card.innerHTML = '<img class="home-card-img home-card-artist" src="/api/cover/'+a.cover_track_id+'" loading="lazy" onerror="this.src=\'/static/logo.png\'"><div class="home-card-title">'+esc(a.artist||'')+'</div><div class="home-card-sub">'+a.track_count+' tracks</div>';
        card.addEventListener('click', function() { navigate('artist-tracks', a.artist); });
        grid.appendChild(card);
      });
    }

    if (data.playlists && data.playlists.length) {
      var section = document.createElement('div');
      section.className = 'home-section';
      section.innerHTML = '<div class="home-section-title">Your playlists</div><div class="home-grid" id="home-playlist-grid"></div>';
      feed.appendChild(section);
      var grid = document.getElementById('home-playlist-grid');
      data.playlists.forEach(function(p) {
        var card = document.createElement('div');
        card.className = 'home-card';
        card.innerHTML = '<div class="home-card-img home-card-playlist">\uD83C\uDFB6</div><div class="home-card-title">'+esc(p.name||'')+'</div><div class="home-card-sub">'+p.track_count+' tracks</div>';
        card.addEventListener('click', function() { navigate('playlist', p.id); });
        grid.appendChild(card);
      });
    }

    if (!feed.children.length) {
      feed.innerHTML = '<div style="text-align:center;padding:60px 20px;color:var(--text-muted);"><div style="font-size:48px;margin-bottom:16px;">\uD83C\uDFB5</div><div style="font-size:18px;font-weight:600;margin-bottom:8px;">Welcome to Moidify</div><div style="font-size:14px;">Upload some music to get started</div></div>';
    }
  }).catch(function() {
    document.getElementById('home-feed').innerHTML = '<div style="text-align:center;padding:60px 20px;color:var(--text-muted);">Failed to load home feed</div>';
  });
}
