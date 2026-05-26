function navigate(view, data) {
  var prevView = state.currentView;
  var prevData = state.currentData;
  if (view === 'search' && prevView !== 'search') {
    state._prevView = prevView;
    state._prevData = prevData;
  }
  state.currentView = view; state.currentData = data;
  qsa('.nav-item').forEach(function(el) { el.classList.toggle('active', el.dataset.view === view); });
  qsa('.playlist-item').forEach(function(el) { el.classList.remove('active'); });
  switch (view) {
    case 'albums': renderAlbums(); break;
    case 'artists': renderArtists(); break;
    case 'tracks': renderTracks(data || null); break;
    case 'favorites': renderFavorites(); break;
    case 'album': renderAlbumDetail(data); break;
    case 'playlist': renderPlaylistDetail(data); break;
    case 'artist-tracks': renderArtistTracks(data); break;
    case 'search': renderSearchResults(data); break;
    default: renderAlbums();
  }
}

async function renderAlbums() {
  var content = document.getElementById('content');
  var isGrid = state.viewMode === 'grid';
  var viewIcon = isGrid ? iconListView() : iconGridView();
  var viewLabel = isGrid ? 'List' : 'Grid';
  content.innerHTML = '<div class="content-header"><div class="view-title">Albums</div><button id="view-toggle" class="icon-btn" title="'+viewLabel+' view">'+viewIcon+'</button></div><div class="album-'+(isGrid?'grid':'list')+'"></div>';
  var container = qs('.album-'+(isGrid?'grid':'list'));
  try {
    var albums = await apiJson('/api/albums');
    if (albums.length === 0) { content.innerHTML = '<div class="content-header"><div class="view-title">Albums</div></div><p style="color:#727272;padding:20px 0;">Drop music into the <strong>music/</strong> folder.</p>'; return; }
    if (isGrid) {
      albums.forEach(function(album) {
        if (!album.album) return;
        var card = document.createElement('div');
        card.className = 'album-card';
        card.innerHTML = '<img src="/api/cover/'+album.cover_track_id+'" alt="" loading="lazy"><div class="album-name">'+esc(album.album)+'</div><div class="album-artist">'+esc(album.artist||'Unknown Artist')+'</div>';
        card.addEventListener('click', function() { navigate('album', {album:album.album, artist:album.artist}); });
        container.appendChild(card);
      });
    } else {
      var list = document.createElement('div'); list.className = 'track-list';
      list.innerHTML = '<div class="track-header"><span></span><span>Album</span><span>Artist</span><span>Tracks</span><span></span></div>';
      albums.forEach(function(album) {
        if (!album.album) return;
        var row = document.createElement('div');
        row.className = 'track-row';
        var coverUrl = '/api/cover/' + album.cover_track_id;
        row.innerHTML =
          '<span class="track-num"><img src="'+coverUrl+'" alt="" class="album-list-cover" loading="lazy"></span>'+
          '<span class="track-title">'+esc(album.album)+'</span>'+
          '<span class="track-artist">'+esc(album.artist||'Unknown Artist')+'</span>'+
          '<span class="track-album">'+(album.track_count||'')+' tracks</span>'+
          '<span class="track-actions"></span>';
        row.addEventListener('click',function(){navigate('album',{album:album.album,artist:album.artist});});
        container.appendChild(row);
      });
    }
    var toggle = document.getElementById('view-toggle');
    if (toggle) {
      toggle.addEventListener('click', function() {
        state.viewMode = (state.viewMode === 'grid' ? 'list' : 'grid');
        localStorage.setItem('moidify_view', state.viewMode);
        renderAlbums();
      });
    }
  } catch(e) { content.innerHTML = '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

async function renderAlbumDetail(data) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="view-title">Loading...</div>';
  try {
    var url = '/api/albums/tracks?album='+encodeURIComponent(data.album)+(data.artist?'&artist='+encodeURIComponent(data.artist):'');
    var albumTracks = await apiJson(url);
    var first = albumTracks[0]||{};
    var yearStr = first.year ? '<div class="album-detail-year">'+first.year+'</div>' : '';
    content.innerHTML = '<div class="album-detail-header"><img src="/api/cover/'+(first.id||0)+'" alt=""><div class="album-detail-meta"><div class="album-detail-title">'+esc(data.album)+'</div><div class="album-detail-artist">'+esc(data.artist||'Unknown Artist')+'</div>'+yearStr+'</div></div><div class="track-list">'+trackHeaderHTML()+'</div>';
    var list = qs('.track-list');
    albumTracks.forEach(function(t,i) { list.appendChild(createTrackRow(t,i,albumTracks)); });
    state.currentTracks = albumTracks; state.currentQueue = albumTracks; state._favedFlag = false;
    setupTrackSorting(list, albumTracks);
  } catch(e) { content.innerHTML = '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

async function renderArtists() {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">Artists</div></div><div class="artist-list"></div>';
  var list = qs('.artist-list');
  try {
    var artists = await apiJson('/api/artists');
    if (artists.length===0) { content.innerHTML = '<div class="content-header"><div class="view-title">Artists</div></div><p style="color:#727272;">No artists found.</p>'; return; }
    artists.forEach(function(a) {
      if (!a.artist) return;
      var item = document.createElement('div');
      item.className = 'artist-item';
      item.innerHTML = '<span class="artist-name">'+esc(a.artist)+'</span><span class="artist-meta">'+a.track_count+' tracks, '+a.album_count+' albums</span>';
      item.addEventListener('click',function(){navigate('artist-tracks',a.artist);});
      list.appendChild(item);
    });
  } catch(e) { content.innerHTML = '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

async function renderArtistTracks(artist) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">'+esc(artist)+'</div></div>';
  try {
    var filtered = await apiJson('/api/artists/tracks?artist='+encodeURIComponent(artist));
    if (filtered.length===0) { content.innerHTML += '<p style="color:#727272;">No tracks found.</p>'; return; }
    var list = document.createElement('div'); list.className = 'track-list';
    list.innerHTML = trackHeaderHTML();
    filtered.forEach(function(t,i){list.appendChild(createTrackRow(t,i,filtered));});
    content.appendChild(list);
    state.currentTracks = filtered; state.currentQueue = filtered; state._favedFlag = false;
    setupTrackSorting(list, filtered);
  } catch(e) { content.innerHTML += '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

async function renderTracks(searchQuery) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">'+(searchQuery?'Search Results':'All Tracks')+'</div></div>';
  try {
    var url = searchQuery ? '/api/tracks?search='+encodeURIComponent(searchQuery) : '/api/tracks';
    var tracks = await apiJson(url);
    if (tracks.length===0) { content.innerHTML += '<p style="color:#727272;">'+(searchQuery?'No results for "'+esc(searchQuery)+'".':'No tracks yet.')+'</p>'; return; }
    var list = document.createElement('div'); list.className='track-list';
    list.innerHTML = trackHeaderHTML();
    tracks.forEach(function(t,i){list.appendChild(createTrackRow(t,i,tracks));});
    content.appendChild(list);
    state.currentTracks = tracks; state.currentQueue = tracks; state._favedFlag = false;
    setupTrackSorting(list, tracks);
  } catch(e) { content.innerHTML += '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

async function renderFavorites() {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">Liked Songs</div></div>';
  if (!state.user) { content.innerHTML += '<p style="color:#727272;padding:20px 0;">Log in to see your liked songs.</p>'; return; }
  try {
    var tracks = await apiJson('/api/favorites');
    if (tracks.length===0) { content.innerHTML += '<div class="fav-empty">No liked songs yet. Click the heart on any track.</div>'; return; }
    var list = document.createElement('div'); list.className='track-list';
    list.innerHTML = trackHeaderHTML();
    tracks.forEach(function(t,i){list.appendChild(createTrackRow(t,i,tracks,true));});
    content.appendChild(list);
    state.currentTracks = tracks; state.currentQueue = tracks; state._favedFlag = true;
    setupTrackSorting(list, tracks);
  } catch(e) { content.innerHTML += '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

async function renderPlaylistDetail(playlistId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="view-title">Loading...</div>';
  try {
    var tracks = await apiJson('/api/playlists/'+playlistId+'/tracks');
    var playlistName = state.playlists.find(function(p){return p.id===playlistId;});
    content.innerHTML = '<div class="content-header"><div class="view-title">'+esc(playlistName?playlistName.name:'Playlist')+'</div></div>';
    if (tracks.length===0) { content.innerHTML += '<div class="fav-empty">This playlist is empty.</div>'; return; }
    var list = document.createElement('div'); list.className='track-list';
    list.innerHTML = trackHeaderHTML();
    tracks.forEach(function(t,i){list.appendChild(createTrackRow(t,i,tracks));});
    content.appendChild(list);
    state.currentTracks = tracks; state.currentQueue = tracks; state._favedFlag = false;
    setupTrackSorting(list, tracks);
    setupPlaylistDragDrop(list, tracks, playlistId);
  } catch(e) { content.innerHTML = '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

function createTrackRow(track, index, queue, isFaved) {
  var row = document.createElement('div');
  row.className = 'track-row';
  row.dataset.trackId = track.id;
  row.dataset.index = index;
  row.dataset.trackDuration = track.duration || 0;
  if (state.queue===queue && state.currentIndex===index) row.classList.add('playing');
  if (state.selectedTrackIds.indexOf(track.id) !== -1) row.classList.add('selected');
  var dur = formatTime(track.duration);
  var isFav = isFaved||false;
  row.innerHTML = '<span class="track-num">'+(index+1)+'</span>'+
    (state.showTrackCovers ? '<img class="track-cover" src="/api/cover/'+track.id+'" alt="" loading="lazy" onerror="this.style.display=\'none\'">' : '')+
    '<span class="track-title">'+esc(track.title)+'</span>'+
    '<span class="track-artist">'+esc(track.artist||'Unknown')+'</span>'+
    '<span class="track-album">'+esc(track.album||'')+'</span>'+
    '<span class="track-dur">'+dur+'</span>'+
    '<span class="track-actions">'+
      '<button class="fav-btn'+(isFav?' faved':'')+'" data-track="'+track.id+'" title="Like">'+(isFav?iconHeartFilled():iconHeart())+'</button>'+
      '<button class="more-btn" data-track="'+track.id+'" title="More">'+iconMore()+'</button></span>';
  row.addEventListener('click',function(e){
    if(e.target.tagName==='BUTTON'||e.target.closest('button'))return;
    if(e.ctrlKey||e.metaKey||e.shiftKey){
      handleTrackSelect(e, track.id, index);
    } else {
      playFromQueue(queue,index);
    }
  });
  row.addEventListener('contextmenu',function(e){e.preventDefault();showContextMenu(e,track,queue,index);});
  qs('.fav-btn',row).addEventListener('click',function(e){e.stopPropagation();toggleFavorite(track.id,this);});
  qs('.more-btn',row).addEventListener('click',function(e){e.stopPropagation();showContextMenu(e,track,queue,index);});
  return row;
}

function handleTrackSelect(e, trackId, index) {
  if (e.shiftKey && state.selectedTrackIds.length) {
    var rows = qsa('.track-row');
    var lastSelected = -1;
    for (var i = 0; i < rows.length; i++) {
      if (state.selectedTrackIds.indexOf(parseInt(rows[i].dataset.trackId)) !== -1) { lastSelected = i; break; }
    }
    if (lastSelected === -1) { toggleTrackSelection(trackId); return; }
    var start = Math.min(lastSelected, index);
    var end = Math.max(lastSelected, index);
    clearSelection();
    for (var j = start; j <= end; j++) {
      var id = parseInt(rows[j].dataset.trackId);
      toggleTrackSelection(id);
    }
  } else {
    if (!e.ctrlKey && !e.metaKey) clearSelection();
    toggleTrackSelection(trackId);
  }
  updateSelectionBar();
}

function toggleTrackSelection(trackId) {
  var idx = state.selectedTrackIds.indexOf(trackId);
  if (idx === -1) { state.selectedTrackIds.push(trackId); } else { state.selectedTrackIds.splice(idx, 1); }
  qsa('.track-row').forEach(function(r) {
    if (parseInt(r.dataset.trackId) === trackId) r.classList.toggle('selected');
  });
}

function clearSelection() {
  state.selectedTrackIds = [];
  qsa('.track-row.selected').forEach(function(r) { r.classList.remove('selected'); });
  updateSelectionBar();
}

function updateSelectionBar() {
  var bar = document.getElementById('selection-bar');
  if (!bar) return;
  if (state.selectedTrackIds.length) {
    bar.classList.add('visible');
    var countEl = qs('.sel-count', bar);
    if (countEl) countEl.textContent = state.selectedTrackIds.length + ' selected';
  } else {
    bar.classList.remove('visible');
  }
}

function getSelectedTracks() {
  var tracks = [];
  qsa('.track-row').forEach(function(r) {
    if (state.selectedTrackIds.indexOf(parseInt(r.dataset.trackId)) !== -1) {
      tracks.push(parseInt(r.dataset.trackId));
    }
  });
  return tracks;
}

function trackHeaderHTML() {
  return '<div class="track-header">'+
    '<span class="sortable" data-sort="tracknum">#<span class="sort-indicator"></span></span>'+
    '<span class="sortable" data-sort="title">Title<span class="sort-indicator"></span></span>'+
    '<span class="sortable" data-sort="artist">Artist<span class="sort-indicator"></span></span>'+
    '<span class="sortable" data-sort="album">Album<span class="sort-indicator"></span></span>'+
    '<span class="sortable" data-sort="duration">Duration<span class="sort-indicator"></span></span>'+
    '<span></span></div>';
}

function sortTracks(tracks, field, dir) {
  var sorted = tracks.slice();
  sorted.sort(function(a, b) {
    var va, vb;
    switch (field) {
      case 'tracknum': va = a.track_number || 0; vb = b.track_number || 0; break;
      case 'title': va = (a.title||'').toLowerCase(); vb = (b.title||'').toLowerCase(); break;
      case 'artist': va = (a.artist||'').toLowerCase(); vb = (b.artist||'').toLowerCase(); break;
      case 'album': va = (a.album||'').toLowerCase(); vb = (b.album||'').toLowerCase(); break;
      case 'duration': va = a.duration || 0; vb = b.duration || 0; break;
      default: return 0;
    }
    if (typeof va === 'string') return dir === 'asc' ? va.localeCompare(vb) : vb.localeCompare(va);
    return dir === 'asc' ? va - vb : vb - va;
  });
  return sorted;
}

function setupTrackSorting(trackList, tracks) {
  qsa('.sortable', trackList).forEach(function(el) {
    el.addEventListener('click', function() {
      var field = this.dataset.sort;
      if (!field) return;
      var dir = 'asc';
      if (state.sortBy === field) dir = state.sortDir === 'asc' ? 'desc' : 'asc';
      state.sortBy = field;
      state.sortDir = dir;
      var sorted = sortTracks(tracks, field, dir);
      qsa('.track-row', trackList).forEach(function(r) { r.remove(); });
      sorted.forEach(function(t, i) { trackList.appendChild(createTrackRow(t, i, sorted, state._favedFlag)); });
      qsa('.sort-indicator', trackList).forEach(function(s) { s.className = 'sort-indicator'; });
      var indicator = qs('.sortable[data-sort="'+field+'"] .sort-indicator', trackList);
      if (indicator) indicator.className = 'sort-indicator active ' + dir;
      state.currentTracks = sorted;
      state.currentQueue = sorted;
    });
  });
}

function setupPlaylistDragDrop(trackList, tracks, playlistId) {
  var listEl = trackList;
  qsa('.track-row', listEl).forEach(function(r) { r.draggable = true; });
  listEl.addEventListener('dragstart', function(e) {
    var row = e.target.closest('.track-row');
    if (!row) return;
    e.dataTransfer.setData('text/plain', row.dataset.trackId);
    row.classList.add('dragging');
  });
  listEl.addEventListener('dragend', function(e) {
    var row = e.target.closest('.track-row');
    if (!row) return;
    row.classList.remove('dragging');
    qsa('.track-row', listEl).forEach(function(r) { r.classList.remove('drag-over'); });
  });
  listEl.addEventListener('dragover', function(e) {
    e.preventDefault();
    var row = e.target.closest('.track-row');
    if (!row) return;
    qsa('.track-row', listEl).forEach(function(r) { r.classList.remove('drag-over'); });
    row.classList.add('drag-over');
  });
  listEl.addEventListener('dragleave', function(e) {
    var row = e.target.closest('.track-row');
    if (!row) return;
    row.classList.remove('drag-over');
  });
  listEl.addEventListener('drop', function(e) {
    e.preventDefault();
    qsa('.track-row', listEl).forEach(function(r) { r.classList.remove('drag-over'); });
    var fromId = parseInt(e.dataTransfer.getData('text/plain'));
    var toRow = e.target.closest('.track-row');
    if (!toRow || !fromId) return;
    var toId = parseInt(toRow.dataset.trackId);
    if (fromId === toId) return;
    var ids = tracks.map(function(t) { return t.id; });
    var fromIdx = ids.indexOf(fromId);
    var toIdx = ids.indexOf(toId);
    if (fromIdx === -1 || toIdx === -1) return;
    ids.splice(fromIdx, 1);
    ids.splice(toIdx, 0, fromId);
    api('/api/playlists/'+playlistId+'/tracks/reorder', { method:'PUT', body:{ order: ids } }).then(function() {
      renderPlaylistDetail(playlistId);
    }).catch(function(err) { console.error('Reorder failed', err); });
  });
}

async function renderSearchResults(query) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">Search: "'+esc(query)+'"</div></div><div class="sr-full-loading">Searching...</div>';

  try {
    var q = query.toLowerCase();
    var qNorm = q.normalize ? q.normalize('NFKD').replace(/[\u0300-\u036f]/g, '') : q;

    var [tracks, albums, artists] = await Promise.all([
      apiJson('/api/tracks?search='+encodeURIComponent(query)),
      apiJson('/api/albums'),
      apiJson('/api/artists'),
    ]);

    function matchField(val) {
      if (!val) return false;
      var v = val.toLowerCase();
      var vNorm = v.normalize ? v.normalize('NFKD').replace(/[\u0300-\u036f]/g, '') : v;
      return v.indexOf(q) !== -1 || vNorm.indexOf(qNorm) !== -1;
    }

    var matchedAlbums = albums.filter(function(a) { return matchField(a.album); }).slice(0, 12);
    var matchedArtists = artists.filter(function(a) { return matchField(a.artist); }).slice(0, 10);
    var matchedTracks = tracks.slice(0, 50);
    var total = matchedAlbums.length + matchedArtists.length + matchedTracks.length;

    var headerHtml = '<div class="content-header"><div class="view-title">Search: "'+esc(query)+'"</div><span style="font-size:13px;color:var(--text-muted);font-weight:400;margin-top:2px;">'+total+' result'+(total!==1?'s':'')+'</span></div>';

    if (!total) {
      content.innerHTML = headerHtml + '<div class="sr-full-empty"><div style="font-size:48px;opacity:0.15;margin-bottom:16px;">\uD83D\uDD0D</div><p style="font-size:18px;font-weight:600;">No results for "'+esc(query)+'"</p><p style="color:var(--text-muted);margin-top:4px;">Try a different search term</p></div>';
      return;
    }

    var html = '';

    if (matchedAlbums.length) {
      html += '<div class="sr-full-section"><h3 class="sr-full-title">Albums</h3><div class="album-grid">';
      matchedAlbums.forEach(function(a) {
        html += '<div class="album-card sr-album" data-album="'+esc(a.album)+'" data-artist="'+esc(a.artist||'')+'">'+
          '<img src="/api/cover/'+a.cover_track_id+'" alt="" loading="lazy">'+
          '<div class="album-name">'+esc(a.album)+'</div>'+
          '<div class="album-artist">'+esc(a.artist||'Unknown Artist')+'</div></div>';
      });
      html += '</div></div>';
    }

    if (matchedArtists.length) {
      html += '<div class="sr-full-section"><h3 class="sr-full-title">Artists</h3><div class="sr-full-artists">';
      matchedArtists.forEach(function(a) {
        html += '<div class="sr-full-artist" data-artist="'+esc(a.artist)+'">'+
          '<div class="sr-full-artist-icon">\uD83C\uDFA4</div>'+
          '<div class="sr-full-artist-info">'+
            '<div class="sr-full-artist-name">'+esc(a.artist)+'</div>'+
            '<div class="sr-full-artist-meta">'+a.track_count+' tracks'+(a.album_count ? ', '+a.album_count+' albums' : '')+'</div>'+
          '</div></div>';
      });
      html += '</div></div>';
    }

    if (matchedTracks.length) {
      html += '<div class="sr-full-section"><h3 class="sr-full-title">Tracks</h3><div class="track-list">'+trackHeaderHTML()+'</div></div>';
    }

    content.innerHTML = headerHtml + html;

    if (matchedTracks.length) {
      var trackList = content.querySelector('.track-list');
      matchedTracks.forEach(function(t, i) {
        trackList.appendChild(createTrackRow(t, i, matchedTracks));
      });
      state.currentTracks = matchedTracks; state.currentQueue = matchedTracks; state._favedFlag = false;
      setupTrackSorting(trackList, matchedTracks);
    }

    qsa('.sr-album', content).forEach(function(el) {
      el.addEventListener('click', function() {
        navigate('album', {album: this.dataset.album, artist: this.dataset.artist});
        document.getElementById('search').value = '';
      });
    });

    qsa('.sr-full-artist', content).forEach(function(el) {
      el.addEventListener('click', function() {
        navigate('artist-tracks', this.dataset.artist);
        document.getElementById('search').value = '';
      });
    });

  } catch(e) {
    content.innerHTML = '<div class="content-header"><div class="view-title">Search</div></div><div class="sr-full-empty"><p style="color:var(--danger);">Error: '+esc(e.message)+'</p></div>';
  }
}
