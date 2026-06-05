function togglePinArtist(artistName) {
  var idx = state.pinnedArtists.indexOf(artistName);
  if (idx === -1) { state.pinnedArtists.push(artistName); } else { state.pinnedArtists.splice(idx, 1); }
  localStorage.setItem('moidify_pinned_artists', JSON.stringify(state.pinnedArtists));
  renderArtists();
}

async function renderArtists(navId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">Artists</div></div>'+skeletonGrid();
  try {
    var artists = await apiJson('/api/artists');
    if (state._navId !== navId) return;
    if (artists.length===0) { content.innerHTML = '<div class="content-header"><div class="view-title">Artists</div></div><p style="color:#727272;">No artists found.</p>'; return; }
    qs('.skeleton-grid', content).remove();
    artists.sort(function(a, b) {
      var aPinned = state.pinnedArtists.indexOf(a.artist) !== -1 ? 0 : 1;
      var bPinned = state.pinnedArtists.indexOf(b.artist) !== -1 ? 0 : 1;
      return aPinned - bPinned;
    });
    var grid = document.createElement('div'); grid.className = 'artist-grid';
    artists.forEach(function(a) {
      if (!a.artist) return;
      var card = document.createElement('div');
      card.className = 'artist-card';
      card.innerHTML = '<img class="artist-card-img" src="/api/artist-image/'+encodeURIComponent(a.artist)+'" loading="lazy" onerror="this.src=\'/static/logo.png\'"><div class="artist-card-name">'+esc(a.artist)+'</div><div class="artist-card-meta">'+a.track_count+' tracks, '+a.album_count+' albums</div>';
      card.addEventListener('click',function(){navigate('artist-tracks',a.artist);});
      card.addEventListener('contextmenu',function(e){e.preventDefault();showArtistContextMenu(e, a);});
      grid.appendChild(card);
    });
    content.appendChild(grid);
  } catch(e) { content.innerHTML = '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

async function renderArtistTracks(artist, navId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="album-detail-header" style="margin-bottom:16px;"><img src="/api/artist-image/'+encodeURIComponent(artist)+'" alt="" style="width:80px;height:80px;border-radius:50%;object-fit:cover;"><div class="album-detail-meta"><div class="album-detail-title">'+esc(artist)+'</div></div></div><div class="track-list-filter"><input type="text" id="track-filter-input" class="track-filter-input" placeholder="Filter tracks..." oninput="filterTrackList(this.value)"></div>';
  try {
    var filtered = await apiJson('/api/artists/tracks?artist='+encodeURIComponent(artist));
    if (state._navId !== navId) return;
    if (filtered.length===0) { content.innerHTML += '<p style="color:#727272;">No tracks found.</p>'; return; }
    var list = document.createElement('div'); list.className = 'track-list';
    list.id = 'current-track-list';
    list.innerHTML = trackHeaderHTML();
    filtered.forEach(function(t,i){list.appendChild(createTrackRow(t,i,filtered));});
    content.appendChild(list);
    state.currentTracks = filtered; state.currentQueue = filtered; state._favedFlag = false;
    setupTrackSorting(list, filtered);
    addShuffleButton(filtered, 'Artist');
    var filterInput = document.getElementById('track-filter-input');
    if (filterInput && filterInput.value.trim()) filterTrackList(filterInput.value);
  } catch(e) { content.innerHTML += '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}
