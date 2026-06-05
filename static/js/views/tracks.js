function filterTrackList(query) {
  var list = document.getElementById('current-track-list');
  if (!list || !list._vlist) return;
  var vlist = list._vlist;
  var allTracks = state._allTracks || [];
  var q = query.toLowerCase().trim();
  if (!q) {
    vlist.update(allTracks);
    state.currentQueue = allTracks;
    state.currentTracks = allTracks;
    return;
  }
  if (list._mergeAll) {
    showToast('Please wait, tracks are still loading...', 'info');
    return;
  }
  var filtered = allTracks.filter(function(t) {
    return (t.title || '').toLowerCase().indexOf(q) !== -1 ||
           (t.artist || '').toLowerCase().indexOf(q) !== -1 ||
           (t.album || '').toLowerCase().indexOf(q) !== -1;
  });
  vlist.update(filtered);
  state.currentQueue = filtered;
  state.currentTracks = filtered;
}

// Simple virtual list shim (no actual virtualization — renders all items)
window.VirtualList = {
  create: function(container, items, renderFn) {
    function render(data) {
      qsa('.track-row', container).forEach(function(el) { el.remove(); });
      data.forEach(function(item, i) { container.appendChild(renderFn(item, i, data)); });
    }
    render(items);
    return { update: render };
  }
};

function createTrackRow(track, index, queue) {
  var row = document.createElement('div');
  row.className = 'track-row';
  row.draggable = true;
  row.dataset.trackId = track.id;
  row.dataset.index = index;
  row.dataset.trackDuration = track.duration || 0;
  if (state.queue===queue && state.currentIndex===index) row.classList.add('playing');
  if (state.selectedTrackIds.indexOf(track.id) !== -1) row.classList.add('selected');
  var dur = formatTime(track.duration);
  var isFav = state._favedFlag||false;
  var rating = track.rating || 0;
  var stars = '';
  for (var s = 1; s <= 5; s++) {
    stars += '<span class="star-rating-star'+(s<=rating?' filled':'')+'" data-rating="'+s+'">'+(s<=rating?'\u2605':'\u2606')+'</span>';
  }
  var numLabel = (track.disc_number && track.disc_number > 1) ? (track.disc_number+'-'+(track.track_number||(index+1))) : ((track.track_number||0) > 0 ? track.track_number : (index+1));
  row.innerHTML = '<span class="track-num">'+numLabel+'</span>'+
    (state.showTrackCovers ? '<img class="track-cover" src="/api/cover/'+track.id+'" alt="" loading="lazy" onerror="this.style.display=\'none\'">' : '')+
    '<span class="track-title">'+esc(track.title)+'</span>'+
    '<span class="track-artist">'+esc(track.artist||'Unknown')+'</span>'+
    '<span class="track-album">'+esc(track.album||'')+'</span>'+
    '<span class="track-rating">'+stars+'</span>'+
    '<span class="track-dur">'+dur+'</span>'+
    '';
  row.addEventListener('click',function(e){
    if(e.ctrlKey||e.metaKey||e.shiftKey){
      handleTrackSelect(e, track.id, index);
    } else {
      playFromQueue(queue,index);
    }
  });
  row.addEventListener('contextmenu',function(e){e.preventDefault();showContextMenu(e,track,queue,index);});
  row.addEventListener('dragstart', function(e) { e.dataTransfer.setData('text/plain', String(track.id)); e.dataTransfer.effectAllowed = 'copy'; row.classList.add('dragging'); });
  row.addEventListener('dragend', function() { row.classList.remove('dragging'); });
  setupLongPress(row, function(e) { showContextMenu({ clientX: e.changedTouches[0].clientX, clientY: e.changedTouches[0].clientY }, track, queue, index); });
  qsa('.star-rating-star', row).forEach(function(el) {
    el.addEventListener('click', function(e) {
      e.stopPropagation();
      var newRating = parseInt(this.dataset.rating);
      var currentRating = track.rating || 0;
      var finalRating = newRating === currentRating ? 0 : newRating;
      track.rating = finalRating;
      api('/api/tracks/'+track.id+'/rating', { method:'PUT', body:{ rating: finalRating } }).then(function() {
        qsa('.star-rating-star', row).forEach(function(st) {
          var r = parseInt(st.dataset.rating);
          st.textContent = r <= finalRating ? '\u2605' : '\u2606';
          st.classList.toggle('filled', r <= finalRating);
        });
      }).catch(function() { track.rating = currentRating; });
    });
  });
  return row;
}

function trackHeaderHTML() {
  return '<div class="track-list-header"><div class="track-header">'+
    '<span class="sortable" data-sort="tracknum"><input type="checkbox" id="select-all-checkbox" title="Select all" style="cursor:pointer;accent-color:var(--accent);"><span class="sort-indicator"></span></span>'+
    '<span class="sortable" data-sort="title">Title<span class="sort-indicator"></span></span>'+
    '<span class="sortable" data-sort="artist">Artist<span class="sort-indicator"></span></span>'+
    '<span class="sortable" data-sort="album">Album<span class="sort-indicator"></span></span>'+
    '<span class="sortable" data-sort="rating">Rating<span class="sort-indicator"></span></span>'+
    '<span class="sortable" data-sort="duration">Duration<span class="sort-indicator"></span></span>'+
    '<span></span></div></div>';
}

function sortTracks(tracks, field, dir) {
  var sorted = tracks.slice();
  sorted.sort(function(a, b) {
    var va, vb;
    switch (field) {
      case 'tracknum': va = (a.disc_number||1) * 1000 + (a.track_number||0); vb = (b.disc_number||1) * 1000 + (b.track_number||0); break;
      case 'title': va = (a.title||'').toLowerCase(); vb = (b.title||'').toLowerCase(); break;
      case 'artist': va = (a.artist||'').toLowerCase(); vb = (b.artist||'').toLowerCase(); break;
      case 'album': va = (a.album||'').toLowerCase(); vb = (b.album||'').toLowerCase(); break;
      case 'rating': va = a.rating || 0; vb = b.rating || 0; break;
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
      // Persist per-view
      localStorage.setItem('moidify_sort_' + state.currentView, JSON.stringify({by:field, dir:dir}));
      var allTracks = tracks;
      if (trackList._mergeAll) {
        showToast('Please wait, tracks are still loading...', 'info');
        return;
      }
      var sorted = sortTracks(allTracks, field, dir);
      if (trackList._vlist) {
        trackList._vlist.update(sorted);
      } else {
        qsa('.track-row', trackList).forEach(function(r) { r.remove(); });
        sorted.forEach(function(t, i) { trackList.appendChild(createTrackRow(t, i, sorted)); });
      }
      qsa('.sort-indicator', trackList).forEach(function(s) { s.className = 'sort-indicator'; });
      var indicator = qs('.sortable[data-sort="'+field+'"] .sort-indicator', trackList);
      if (indicator) indicator.className = 'sort-indicator active ' + dir;
      state.currentTracks = sorted;
      state.currentQueue = sorted;
    });
  });
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

async function renderTracks(searchQuery, navId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">'+(searchQuery?'Search Results':'All Tracks')+'</div></div>'+(searchQuery?'':'<div class="track-list-filter"><input type="text" id="track-filter-input" class="track-filter-input" placeholder="Filter tracks..." oninput="filterTrackList(this.value)"></div>')+'<div class="track-skeleton-wrap">'+skeletonTrackRows()+'</div>';
  try {
    var url = searchQuery ? '/api/tracks?search='+encodeURIComponent(searchQuery) : '/api/tracks?limit=200';
    var data = await apiJson(url);
    if (state._navId !== navId) return;
    var tracks = Array.isArray(data) ? data : data.tracks;
    var total = Array.isArray(data) ? tracks.length : data.total;
    if (tracks.length===0) { content.innerHTML += '<p style="color:#727272;">'+(searchQuery?'No results for "'+esc(searchQuery)+'".':'No tracks yet.')+'</p>'; return; }
    var skelWrap = qs('.track-skeleton-wrap', content);
    if (skelWrap) skelWrap.remove();
    var list = document.createElement('div'); list.className='track-list virtual';
    list.id = searchQuery ? '' : 'current-track-list';
    list.innerHTML = trackHeaderHTML();
    var vlist = VirtualList.create(list, tracks, function(t, i, arr) { return createTrackRow(t, i, arr); });
    content.appendChild(list);
    list._vlist = vlist;
    state._allTracks = tracks;
    state.currentTracks = tracks; state.currentQueue = tracks; state._favedFlag = false;
    setupTrackSorting(list, tracks);
    if (!searchQuery) {
      addShuffleButton(tracks, 'All Tracks');
      var filterInput = document.getElementById('track-filter-input');
      if (filterInput && filterInput.value.trim()) filterTrackList(filterInput.value);
      // Load remaining pages in background
      if (!Array.isArray(data) && total > tracks.length) {
        var loadMore = document.createElement('div');
        loadMore.className = 'load-more-bar';
        loadMore.textContent = 'Loading ' + (total - tracks.length) + ' more tracks...';
        content.appendChild(loadMore);
        (function() {
          var allTracks = tracks.slice();
          var page = 1;
          var PAGE_SIZE = 200;
          function fetchNext() {
            var offset = page * PAGE_SIZE;
            if (offset >= total) { loadMore.remove(); return; }
            apiJson('/api/tracks?limit=' + PAGE_SIZE + '&offset=' + offset).then(function(d) {
              if (state._navId !== navId) return;
              var chunk = Array.isArray(d) ? d : d.tracks;
              allTracks = allTracks.concat(chunk);
              page++;
              loadMore.textContent = 'Loading ' + (total - allTracks.length) + ' more tracks...';
              fetchNext();
            }).catch(function() { loadMore.textContent = 'Failed to load more tracks'; });
          }
          fetchNext();
          // Expose merge function for sort/filter to use
          list._mergeAll = function() {
            loadMore.remove();
            state._allTracks = allTracks;
            state.currentTracks = allTracks;
            state.currentQueue = allTracks;
            vlist.update(allTracks);
          };
        })();
      }
    }
  } catch(e) { content.innerHTML += '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

async function renderFavorites(navId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">Liked Songs</div></div><div class="track-list-filter"><input type="text" id="track-filter-input" class="track-filter-input" placeholder="Filter tracks..." oninput="filterTrackList(this.value)"></div><div class="track-skeleton-wrap">'+skeletonTrackRows()+'</div>';
  if (!state.user) { content.innerHTML += '<p style="color:#727272;padding:20px 0;">Log in to see your liked songs.</p>'; return; }
  try {
    var tracks = await apiJson('/api/favorites');
    if (state._navId !== navId) return;
    if (tracks.length===0) { content.innerHTML += '<div class="fav-empty">No liked songs yet. Click the heart on any track.</div>'; return; }
    var skelWrap = qs('.track-skeleton-wrap', content);
    if (skelWrap) skelWrap.remove();
    var list = document.createElement('div'); list.className='track-list virtual';
    list.id = 'current-track-list';
    list.innerHTML = trackHeaderHTML();
    var vlist = VirtualList.create(list, tracks, function(t, i, arr) { return createTrackRow(t, i, arr, true); });
    content.appendChild(list);
    list._vlist = vlist;
    state._allTracks = tracks;
    state.currentTracks = tracks; state.currentQueue = tracks; state._favedFlag = true;
    setupTrackSorting(list, tracks);
    addShuffleButton(tracks, 'Liked Songs');
    var filterInput = document.getElementById('track-filter-input');
    if (filterInput && filterInput.value.trim()) filterTrackList(filterInput.value);
  } catch(e) { content.innerHTML += '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}
