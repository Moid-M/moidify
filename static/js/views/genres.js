async function renderGenres(navId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">Genres</div></div>';
  try {
    var genres = await apiJson('/api/genres');
    if (state._navId !== navId) return;
    if (genres.length === 0) {
      content.innerHTML += '<p style="color:var(--text-muted);padding:20px 0;">No genre tags found in your music.</p>';
      return;
    }
    var grid = document.createElement('div'); grid.className = 'genre-grid';
    genres.forEach(function(g) {
      var card = document.createElement('div');
      card.className = 'genre-card';
      card.innerHTML = '<div class="genre-card-name">'+esc(g.genre||'Unknown')+'</div>'+
        '<div class="genre-card-meta">'+g.track_count+' tracks'+(g.album_count ? ', '+g.album_count+' albums' : '')+'</div>';
      card.addEventListener('click', function() { navigate('genre-tracks', g.genre); });
      grid.appendChild(card);
    });
    content.appendChild(grid);
  } catch(e) { content.innerHTML += '<p style="color:var(--danger);">Error: '+esc(e.message)+'</p>'; }
}

async function renderGenreTracks(genre, navId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">'+esc(genre)+'</div></div><div class="track-list-filter"><input type="text" id="track-filter-input" class="track-filter-input" placeholder="Filter tracks..." oninput="filterTrackList(this.value)"></div>';
  try {
    var tracks = await apiJson('/api/genres/tracks?genre='+encodeURIComponent(genre));
    if (state._navId !== navId) return;
    if (tracks.length === 0) { content.innerHTML += '<p style="color:var(--text-muted);">No tracks found.</p>'; return; }
    var list = document.createElement('div'); list.className = 'track-list';
    list.id = 'current-track-list';
    list.innerHTML = trackHeaderHTML();
    tracks.forEach(function(t,i) { list.appendChild(createTrackRow(t,i,tracks)); });
    content.appendChild(list);
    state.currentTracks = tracks; state.currentQueue = tracks; state._favedFlag = false;
    setupTrackSorting(list, tracks);
    addShuffleButton(tracks, genre);
    var filterInput = document.getElementById('track-filter-input');
    if (filterInput && filterInput.value.trim()) filterTrackList(filterInput.value);
  } catch(e) { content.innerHTML += '<p style="color:var(--danger);">Error: '+esc(e.message)+'</p>'; }
}
