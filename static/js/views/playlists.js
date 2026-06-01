async function renderPlaylistDetail(playlistId, navId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="view-title">Loading...</div>';
  try {
    var tracks = await apiJson('/api/playlists/'+playlistId+'/tracks');
    if (state._navId !== navId) return;
    var playlistName = state.playlists.find(function(p){return p.id===playlistId;});
    content.innerHTML = '<div class="content-header"><div class="view-title">'+esc(playlistName?playlistName.name:'Playlist')+'</div>'+
      '<div style="display:flex;gap:8px"><button id="share-pl-btn" class="icon-btn" title="Share playlist">'+iconShare()+'</button><button id="export-pl-btn" class="icon-btn" title="Export playlist">'+iconDownload()+'</button></div></div>'+
      '<div id="share-pl-status" style="display:none;padding:10px 14px;background:var(--bg-el);border-radius:var(--radius);margin-bottom:12px;font-size:13px;align-items:center;gap:10px"></div>'+
      '<div class="track-list-filter"><input type="text" id="track-filter-input" class="track-filter-input" placeholder="Filter tracks..." oninput="filterTrackList(this.value)"></div>';
    if (tracks.length===0) { content.innerHTML += '<div class="fav-empty">This playlist is empty.</div>'; return; }
    var list = document.createElement('div'); list.className = 'track-list';
    list.id = 'current-track-list';
    list.innerHTML = trackHeaderHTML();
    tracks.forEach(function(t,i){list.appendChild(createTrackRow(t,i,tracks));});
    content.appendChild(list);
    state.currentTracks = tracks; state.currentQueue = tracks; state._favedFlag = false;
    setupTrackSorting(list, tracks);
    setupPlaylistDragDrop(list, tracks, playlistId);
    setupPlaylistShare(playlistId);
    setupPlaylistExport(playlistId);
    addShuffleButton(tracks, 'Playlist');
    var filterInput = document.getElementById('track-filter-input');
    if (filterInput && filterInput.value.trim()) filterTrackList(filterInput.value);
  } catch(e) { content.innerHTML = '<p style="color:#e74c3c;">Error: '+e.message+'</p>'; }
}

function setupPlaylistShare(playlistId) {
  var btn = document.getElementById('share-pl-btn');
  var status = document.getElementById('share-pl-status');
  if (!btn) return;

  apiJson('/api/playlists/'+playlistId+'/share').then(function(d) {
    if (d.shared) { btn.classList.add('active'); }
  }).catch(function(){});

  btn.addEventListener('click', function() {
    apiJson('/api/playlists/'+playlistId+'/share', {method:'POST'}).then(function(d) {
      var link = window.location.origin + '/s/' + d.token;
      btn.classList.add('active');
      status.style.display = 'flex';
      status.innerHTML =
        '<span style="flex:1;color:var(--text2);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">'+link+'</span>'+
        '<button class="icon-btn" id="copy-share-link" style="flex-shrink:0" title="Copy link">'+iconCopy()+'</button>'+
        '<button class="icon-btn" id="unshare-pl-btn" style="flex-shrink:0;color:var(--danger)" title="Revoke">'+iconClose()+'</button>';
      document.getElementById('copy-share-link').addEventListener('click', function() {
        navigator.clipboard.writeText(link).then(function() {
          this.innerHTML = iconCheck();
          setTimeout(function() { this.innerHTML = iconCopy(); }.bind(this), 2000);
        }.bind(this)).catch(function() {});
      });
      document.getElementById('unshare-pl-btn').addEventListener('click', function() {
        api('/api/playlists/'+playlistId+'/share', {method:'DELETE'}).then(function() {
          status.style.display = 'none';
          btn.classList.remove('active');
        }).catch(function() {});
      });
    }).catch(function(e) { showToast('Failed to load playlist', 'error'); });
  });
}

function setupPlaylistExport(playlistId) {
  var btn = document.getElementById('export-pl-btn');
  if (!btn) return;
  btn.addEventListener('click', function() {
    var dropdown = document.createElement('div');
    dropdown.className = 'context-menu';
    dropdown.style.display = 'block';
    dropdown.style.position = 'absolute';
    var rect = btn.getBoundingClientRect();
    dropdown.innerHTML =
      '<div class="context-menu-item" data-format="m3u">Export as M3U</div>'+
      '<div class="context-menu-item" data-format="json">Export as JSON</div>';
    dropdown.style.left = Math.min(rect.left, window.innerWidth - 200) + 'px';
    dropdown.style.top = (rect.bottom + 4) + 'px';
    document.body.appendChild(dropdown);
    function cleanup() { dropdown.remove(); document.removeEventListener('click', cleanup); }
    setTimeout(function() { document.addEventListener('click', cleanup); }, 0);
    qsa('.context-menu-item', dropdown).forEach(function(el) {
      el.addEventListener('click', function() {
        var format = this.dataset.format;
        var link = '/api/playlists/'+playlistId+'/export?format='+format;
        var a = document.createElement('a');
        a.href = link;
        a.download = '';
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        cleanup();
      });
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
    }).catch(function(err) { showToast('Reorder failed', 'error'); });
  });
}
