function togglePinAlbum(albumName) {
  var idx = state.pinnedAlbums.indexOf(albumName);
  if (idx === -1) { state.pinnedAlbums.push(albumName); } else { state.pinnedAlbums.splice(idx, 1); }
  localStorage.setItem('moidify_pinned_albums', JSON.stringify(state.pinnedAlbums));
  renderAlbums();
}

async function renderAlbums(navId) {
  var content = document.getElementById('content');
  var isGrid = state.viewMode === 'grid';
  var viewIcon = isGrid ? iconListView() : iconGridView();
  var viewLabel = isGrid ? 'List' : 'Grid';
  content.innerHTML = '<div class="content-header"><div class="view-title">Albums</div>'+
    '<div style="display:flex;gap:8px;align-items:center"><button id="group-toggle" class="icon-btn'+(state.albumGrouping?' active':'')+'" title="Group album versions">'+iconCollapse()+'</button><button id="view-toggle" class="icon-btn" title="'+viewLabel+' view">'+viewIcon+'</button></div></div><div class="album-'+(isGrid?'grid':'list')+'"></div>';
  var container = qs('.album-'+(isGrid?'grid':'list'));
  try {
    var url = state.albumGrouping ? '/api/albums?grouped=true' : '/api/albums';
    var data = await apiJson(url);
    if (state._navId !== navId) return;
    var albums = state.albumGrouping ? [] : data;
    var groups = state.albumGrouping ? data : [];
    if ((!state.albumGrouping && albums.length === 0) || (state.albumGrouping && groups.length === 0)) {
      content.innerHTML = '<div class="content-header"><div class="view-title">Albums</div></div><p style="color:#727272;padding:20px 0;">Drop music into the <strong>music/</strong> folder.</p>'; return;
    }
    if (!state.albumGrouping) {
      albums.sort(function(a, b) {
        var aPinned = state.pinnedAlbums.indexOf(a.album) !== -1 ? 0 : 1;
        var bPinned = state.pinnedAlbums.indexOf(b.album) !== -1 ? 0 : 1;
        return aPinned - bPinned;
      });
    }
    if (state.albumGrouping) {
      groups.forEach(function(group) {
        var expanded = state.expandedAlbumGroups && state.expandedAlbumGroups.indexOf(group.base_album) !== -1;
        if (!group.versions || group.versions.length <= 1) {
          // Single version — render as normal album
          var album = group.versions[0];
          if (!album) return;
          renderAlbumItem(container, album, isGrid);
          return;
        }
        // Multiple versions — render group header + expandable sub-items
        var wrapper = document.createElement('div');
        wrapper.className = 'album-group';
        var header = document.createElement('div');
        header.className = 'album-group-header';
        header.innerHTML = '<span class="album-group-arrow">'+(expanded?'▾':'▸')+'</span><span class="album-group-name">'+esc(group.base_album)+'</span><span class="album-group-artist">'+esc(group.artist||'Unknown Artist')+'</span><span class="album-group-count">'+group.versions.length+' versions</span>';
        header.addEventListener('click', function() {
          if (!state.expandedAlbumGroups) state.expandedAlbumGroups = [];
          var idx = state.expandedAlbumGroups.indexOf(group.base_album);
          if (idx === -1) state.expandedAlbumGroups.push(group.base_album);
          else state.expandedAlbumGroups.splice(idx, 1);
          localStorage.setItem('moidify_expanded_groups', JSON.stringify(state.expandedAlbumGroups));
          renderAlbums();
        });
        wrapper.appendChild(header);
        if (expanded) {
          var sub = document.createElement('div');
          sub.className = 'album-group-versions';
          group.versions.forEach(function(album) {
            renderAlbumItem(sub, album, isGrid);
          });
          wrapper.appendChild(sub);
        }
        container.appendChild(wrapper);
      });
    } else {
      if (isGrid) {
        albums.forEach(function(album) { renderAlbumItem(container, album, isGrid); });
      } else {
        var list = document.createElement('div'); list.className = 'track-list';
        list.innerHTML = '<div class="track-header"><span></span><span>Album</span><span>Artist</span><span>Tracks</span><span></span></div>';
        albums.forEach(function(album) { renderAlbumItem(list, album, isGrid); });
        container.appendChild(list);
      }
    }
    var toggle = document.getElementById('view-toggle');
    if (toggle) {
      toggle.addEventListener('click', function() {
        state.viewMode = (state.viewMode === 'grid' ? 'list' : 'grid');
        localStorage.setItem('moidify_view', state.viewMode);
        renderAlbums();
      });
    }
    var gtoggle = document.getElementById('group-toggle');
    if (gtoggle) {
      gtoggle.addEventListener('click', function() {
        state.albumGrouping = !state.albumGrouping;
        localStorage.setItem('moidify_album_grouping', state.albumGrouping ? '1' : '');
        renderAlbums();
      });
    }
  } catch(e) { content.innerHTML = '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

function renderAlbumItem(container, album, isGrid) {
  if (!album || !album.album) return;
  var isPinned = state.pinnedAlbums.indexOf(album.album) !== -1;
  if (isGrid) {
    var card = document.createElement('div');
    card.className = 'album-card';
    card.innerHTML = '<img src="/api/cover/'+album.cover_track_id+'" alt="" loading="lazy"><button class="album-pin-btn'+(isPinned?' pinned':'')+'" data-album="'+esc(album.album)+'">'+iconPin()+'</button><div class="album-name">'+esc(album.album)+'</div><div class="album-artist">'+esc(album.artist||'Unknown Artist')+'</div>';
    qs('.album-pin-btn', card).addEventListener('click', function(e) { e.stopPropagation(); togglePinAlbum(this.dataset.album); });
    card.addEventListener('click', function() { navigate('album', {album:album.album, artist:album.artist}); });
    card.addEventListener('contextmenu', function(e) { e.preventDefault(); showAlbumContextMenu(e, album); });
    container.appendChild(card);
  } else {
    var row = document.createElement('div');
    row.className = 'track-row';
    var coverUrl = '/api/cover/' + album.cover_track_id;
    row.innerHTML =
      '<span class="track-num"><img src="'+coverUrl+'" alt="" class="album-list-cover" loading="lazy"><button class="album-pin-btn'+(isPinned?' pinned':'')+'" data-album="'+esc(album.album)+'" style="background:none;border:none;cursor:pointer;padding:0;color:var(--text-muted);margin-left:4px;display:inline-flex;">'+iconPin()+'</button></span>'+
      '<span class="track-title">'+esc(album.album)+'</span>'+
      '<span class="track-artist">'+esc(album.artist||'Unknown Artist')+'</span>'+
      '<span class="track-album">'+(album.track_count||'')+' tracks</span>'+
      '<span class="track-actions"></span>';
    qs('.album-pin-btn', row).addEventListener('click', function(e) { e.stopPropagation(); togglePinAlbum(this.dataset.album); });
    row.addEventListener('click',function(){navigate('album',{album:album.album,artist:album.artist});});
    row.addEventListener('contextmenu',function(e){e.preventDefault();showAlbumContextMenu(e, album);});
    container.appendChild(row);
  }
}

async function renderAlbumDetail(data, navId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="view-title">Loading...</div>';
  try {
    var url = '/api/albums/tracks?album='+encodeURIComponent(data.album)+(data.artist?'&artist='+encodeURIComponent(data.artist):'');
    var albumTracks = await apiJson(url);
    if (state._navId !== navId) return;
    var first = albumTracks[0]||{};
    var yearStr = first.year ? '<div class="album-detail-year">'+first.year+'</div>' : '';
    content.innerHTML = '<div class="album-detail-header"><img src="/api/cover/'+(first.id||0)+'" alt=""><div class="album-detail-meta"><div class="album-detail-title">'+esc(data.album)+'</div><div class="album-detail-artist">'+esc(data.artist||'Unknown Artist')+'</div>'+yearStr+'</div><div style="display:flex;gap:8px;align-items:center"><button class="shuffle-view-btn" id="album-play-btn" title="Play All">'+iconPlay()+' <span>Play All</span></button><button class="shuffle-play-btn" id="album-shuffle-btn" title="Shuffle Album">'+iconShuffle()+' <span>Shuffle</span></button><button class="icon-btn" id="album-share-btn" title="Share album">'+iconShare()+'</button></div></div><div class="track-list">'+trackHeaderHTML()+'</div>';
    var list = qs('.track-list');
    albumTracks.forEach(function(t,i) { list.appendChild(createTrackRow(t,i,albumTracks)); });
    state.currentTracks = albumTracks; state.currentQueue = albumTracks; state._favedFlag = false;
    setupTrackSorting(list, albumTracks);
    setupAlbumPlay(albumTracks);
    setupAlbumShuffle(albumTracks);
    setupAlbumShare(data.album, data.artist);
  } catch(e) { content.innerHTML = '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

function setupAlbumPlay(tracks) {
  var btn = document.getElementById('album-play-btn');
  if (!btn) return;
  btn.addEventListener('click', function() {
    playFromQueue(tracks, 0);
  });
}

function setupAlbumShuffle(tracks) {
  var btn = document.getElementById('album-shuffle-btn');
  if (!btn) return;
  btn.addEventListener('click', function() {
    var shuffled = tracks.slice();
    for (var i = shuffled.length - 1; i > 0; i--) {
      var j = Math.floor(Math.random() * (i + 1));
      var tmp = shuffled[i]; shuffled[i] = shuffled[j]; shuffled[j] = tmp;
    }
    playFromQueue(shuffled, 0);
  });
}

function setupAlbumShare(album, artist) {
  var btn = document.getElementById('album-share-btn');
  if (!btn) return;
  btn.addEventListener('click', function() {
    apiJson('/api/albums/share', {method:'POST', body:{album:album, artist:artist||null}}).then(function(d) {
      var origin = window.location.origin;
      var link = origin + '/a/' + d.token;
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(link).then(function() {
          showToast('Album share link copied!', 'success');
        }).catch(function() {
          prompt('Share this link:', link);
        });
      } else {
        prompt('Share this link:', link);
      }
    }).catch(function() {
      showToast('Failed to share album', 'error');
    });
  });
}
