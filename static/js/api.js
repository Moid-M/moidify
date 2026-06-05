function getAuthToken() {
  return state.token || localStorage.getItem('moidify_token');
}

async function api(path, options) {
  options = options || {};
  var headers = {};
  if (state.token) headers['token'] = state.token;
  if (options.body && !(options.body instanceof FormData)) {
    headers['Content-Type'] = 'application/json';
    options.body = JSON.stringify(options.body);
  }
  var res = await fetch(path, Object.assign({}, options, { headers: headers }));
  if (!res.ok) {
    var msg = res.statusText;
    try { var err = await res.json(); msg = err.detail || msg; } catch (e) {}
    throw new Error(msg);
  }
  return res;
}

function apiJson(path, options) {
  return api(path, options).then(function(r) { return r.json(); });
}

async function checkAuth() {
  if (!state.token) return;
  try {
    state.user = await apiJson('/api/auth/me');
    renderAuth();
    loadPlaylists();
  } catch (e) {
    state.user = null; renderAuth();
  }
}

async function login(username, password) {
  var data = await apiJson('/api/auth/login', { method:'POST', body:{ username, password } });
  state.token = data.token; localStorage.setItem('moidify_token', data.token);
  state.user = data.user; closeModal(); renderAuth(); loadPlaylists();
  // Load server state after login, then navigate
  var hadServerState = await loadStateFromServer();
  if (!hadServerState) {
    // Sync local state to server
    syncStateToServer();
  }
  if (state.currentView) navigate(state.currentView, state.currentData);
}

async function register(username, password, email) {
  await apiJson('/api/auth/register', { method:'POST', body:{ username, password, email } });
  await login(username, password);
}

function logout() {
  api('/api/auth/logout', { method: 'POST' }).catch(function() {});
  api('/api/player/state', { method: 'DELETE' }).catch(function() {});
  state.token = null; localStorage.removeItem('moidify_token'); state.user = null;
  renderAuth(); navigate('albums'); qs('#playlist-list').innerHTML = '';
  document.getElementById('pinned-section').style.display = 'none';
}

function renderAuth() {
  var loggedOut = qs('#top-auth #auth-logged-out');
  var loggedIn = qs('#top-auth #auth-logged-in');
  if (!loggedOut) return;
  if (state.user) {
    loggedOut.style.display = 'none'; loggedIn.style.display = 'flex';
    document.getElementById('user-display').textContent = state.user.username;
    document.getElementById('fav-nav').style.display = '';
  } else {
    loggedOut.style.display = ''; loggedIn.style.display = 'none';
    document.getElementById('fav-nav').style.display = 'none';
  }
}

async function checkFavoriteStatus(trackId) {
  if (!state.user) { state.favedTracks[trackId] = false; return; }
  try {
    var data = await apiJson('/api/favorites/check/'+trackId);
    state.favedTracks[trackId] = !!data.favorite;
  } catch(e) {}
}

async function toggleFavorite(trackId) {
  if (!state.user) { showLoginModal(); return; }
  try {
    var isFav = state.favedTracks[trackId];
    if (isFav) { await api('/api/favorites/'+trackId,{method:'DELETE'}); state.favedTracks[trackId] = false; showToast('Removed from Liked Songs', 'success'); }
    else { await api('/api/favorites/'+trackId,{method:'POST'}); state.favedTracks[trackId] = true; showToast('Added to Liked Songs', 'success'); }
  } catch(e) { showToast('Failed to toggle favorite', 'error'); }
}

async function loadPlaylists() {
  var list = document.getElementById('playlist-list');
  if (!state.user) { list.innerHTML=''; return; }
  try {
    state.playlists = await apiJson('/api/playlists');
    var folders = await apiJson('/api/playlist-folders');
    list.innerHTML = '';
    renderPinnedPlaylists();

    // Group playlists by folder
    var uncategorized = [];
    var byFolder = {};
    state.playlists.forEach(function(pl) {
      if (pl.folder_id) {
        if (!byFolder[pl.folder_id]) byFolder[pl.folder_id] = [];
        byFolder[pl.folder_id].push(pl);
      } else {
        uncategorized.push(pl);
      }
    });

    // Render folders
    folders.forEach(function(folder) {
      var folderEl = document.createElement('div');
      folderEl.className = 'playlist-folder';
      var isExpanded = localStorage.getItem('moidify_folder_open_' + folder.id) !== 'false';
      var folderPls = byFolder[folder.id] || [];
      folderEl.innerHTML =
        '<div class="playlist-folder-header">'+
          '<span class="playlist-folder-toggle">'+(isExpanded ? '\u25BC' : '\u25B6')+'</span>'+
          '<span class="playlist-folder-name">'+esc(folder.name)+'</span>'+
          '<span class="playlist-folder-count">'+folderPls.length+'</span>'+
          '<button class="del-pl" data-folder="'+folder.id+'" title="Delete folder">\u2715</button>'+
        '</div>'+
        '<div class="playlist-folder-items"'+(isExpanded?'':' style="display:none"')+'>'+
        '</div>';
      var header = folderEl.querySelector('.playlist-folder-header');
      header.addEventListener('click', function(e) {
        if (e.target.tagName === 'BUTTON') return;
        var items = folderEl.querySelector('.playlist-folder-items');
        var toggle = folderEl.querySelector('.playlist-folder-toggle');
        var expanded = items.style.display !== 'none';
        items.style.display = expanded ? 'none' : '';
        toggle.textContent = expanded ? '\u25B6' : '\u25BC';
        localStorage.setItem('moidify_folder_open_' + folder.id, !expanded);
      });
      var delBtn = folderEl.querySelector('.del-pl');
      if (delBtn) {
        delBtn.addEventListener('click', function(e) {
          e.stopPropagation();
          if (confirm('Delete folder "'+folder.name+'"? Playlists will be moved out.')) {
            deletePlaylistFolder(folder.id);
          }
        });
      }
      list.appendChild(folderEl);

      var itemsContainer = folderEl.querySelector('.playlist-folder-items');
      folderPls.forEach(function(pl) {
        itemsContainer.appendChild(createPlaylistItem(pl));
      });
    });

    // Render uncategorized playlists
    uncategorized.forEach(function(pl) {
      list.appendChild(createPlaylistItem(pl));
    });
  } catch(e) { showToast('Failed to load playlists', 'error'); }
}

function createPlaylistItem(pl) {
  var isPinned = state.pinnedPlaylists.indexOf(pl.id) !== -1;
  var item = document.createElement('div');
  item.className = 'playlist-item'+(state.currentView==='playlist'&&state.currentData===pl.id?' active':'');
  item.innerHTML = '<button class="pin-btn'+(isPinned?' pinned':'')+'" data-pl="'+pl.id+'" title="Pin">'+(isPinned?iconPinFilled():iconPin())+'</button><span>'+esc(pl.name)+'</span><button class="del-pl" data-pl="'+pl.id+'" title="Delete">\u2715</button>';
  item.addEventListener('click',function(e){if(e.target.tagName==='BUTTON')return;navigate('playlist',pl.id);});
  qs('.pin-btn',item).addEventListener('click',function(e){e.stopPropagation();togglePinPlaylist(pl.id);});
  qs('.del-pl',item).addEventListener('click',function(e){e.stopPropagation();deletePlaylist(pl.id);});
  item.addEventListener('contextmenu', function(e) {
    e.preventDefault();
    showPlaylistFolderMenu(e, pl.id);
  });
  return item;
}

function showPlaylistFolderMenu(event, playlistId) {
  hideContextMenu();
  var menu = document.getElementById('context-menu');
  var playlistName = '';
  var pl = state.playlists.find(function(p) { return p.id === playlistId; });
  if (pl) playlistName = pl.name;
  var html = '';
  html += '<div class="context-menu-item" data-action="playlist-play"><span class="cmi-icon">'+iconPlay()+'</span> Play</div>';
  html += '<div class="context-menu-item" data-action="playlist-play-next"><span class="cmi-icon">'+iconForward()+'</span> Play Next</div>';
  html += '<div class="context-menu-item" data-action="playlist-add-queue"><span class="cmi-icon">'+iconQueue()+'</span> Add to Queue</div>';
  html += '<div class="context-menu-divider"></div>';
  html += '<div class="context-menu-item" data-action="playlist-share"><span class="cmi-icon">'+iconShare()+'</span> Share</div>';
  html += '<div class="context-menu-item" data-action="playlist-export-m3u"><span class="cmi-icon">'+iconDownload()+'</span> Export M3U</div>';
  html += '<div class="context-menu-item" data-action="playlist-export-json"><span class="cmi-icon">'+iconDownload()+'</span> Export JSON</div>';
  html += '<div class="context-menu-divider"></div>';
  html += '<div class="context-menu-item" style="cursor:default;color:var(--text-muted);font-size:12px;text-transform:uppercase;letter-spacing:1px;">Move to folder</div>';
  html += '<div class="context-menu-item" data-action="move-folder" data-folder="">None</div>';
  menu.innerHTML = html;
  // Load folders and append
  apiJson('/api/playlist-folders').then(function(folders) {
    folders.forEach(function(f) {
      var el = document.createElement('div');
      el.className = 'context-menu-item';
      el.dataset.action = 'move-folder';
      el.dataset.folder = f.id;
      el.innerHTML = '<span class="cmi-icon"></span>'+esc(f.name);
      menu.appendChild(el);
    });
    // Re-attach listeners on new items
    qsa('.context-menu-item[data-action="move-folder"]', menu).forEach(function(el) {
      el.addEventListener('click', function() {
        var folderId = this.dataset.folder;
        api('/api/playlists/'+playlistId+'/folder', { method:'PUT', body:{ folder_id: folderId ? parseInt(folderId) : null } }).then(function() {
          loadPlaylists();
        }).catch(function() { showToast('Failed to move playlist', 'error'); });
        hideContextMenu();
      });
    });
  }).catch(function() {});
  menu.style.display = 'block';
  var x = event.clientX, y = event.clientY;
  var mw = Math.min(240, menu.offsetWidth || 240);
  if (x + mw > window.innerWidth) x = window.innerWidth - mw - 10;
  if (y + 300 > window.innerHeight) y = window.innerHeight - 300 - 10;
  if (x < 10) x = 10; if (y < 10) y = 10;
  menu.style.left = x + 'px'; menu.style.top = y + 'px';
  // Attach action handlers
  qsa('.context-menu-item', menu).forEach(function(el) {
    el.addEventListener('click', function(e) {
      var action = this.dataset.action;
      if (action === 'playlist-play') {
        apiJson('/api/playlists/' + playlistId + '/tracks').then(function(tracks) {
          if (tracks.length) playFromQueue(tracks, 0);
        }).catch(function() { showToast('Failed to load playlist', 'error'); });
        hideContextMenu();
      } else if (action === 'playlist-play-next') {
        apiJson('/api/playlists/' + playlistId + '/tracks').then(function(tracks) {
          tracks.slice().reverse().forEach(function(t) { addTrackToQueueNext(t); });
          showToast('Added ' + tracks.length + ' tracks to play next', 'info');
        }).catch(function() { showToast('Failed to load playlist', 'error'); });
        hideContextMenu();
      } else if (action === 'playlist-add-queue') {
        apiJson('/api/playlists/' + playlistId + '/tracks').then(function(tracks) {
          tracks.forEach(function(t) { addTrackToQueueEnd(t); });
          showToast('Added ' + tracks.length + ' tracks to queue', 'info');
        }).catch(function() { showToast('Failed to load playlist', 'error'); });
        hideContextMenu();
      } else if (action === 'playlist-share') {
        apiJson('/api/playlists/' + playlistId + '/share', { method: 'POST' }).then(function(d) {
          var link = window.location.origin + '/s/' + d.token;
          if (navigator.clipboard && navigator.clipboard.writeText) {
            navigator.clipboard.writeText(link).then(function() {
              showToast('Share link copied!', 'success');
            }).catch(function() {
              prompt('Share link:', link);
            });
          } else {
            prompt('Share link:', link);
          }
        }).catch(function() { showToast('Failed to share playlist', 'error'); });
        hideContextMenu();
      } else if (action === 'playlist-export-m3u') {
        exportPlaylist(playlistId, 'm3u');
        hideContextMenu();
      } else if (action === 'playlist-export-json') {
        exportPlaylist(playlistId, 'json');
        hideContextMenu();
      }
    });
  });
  document.addEventListener('click', hideContextMenuOnce);
}

function exportPlaylist(playlistId, format) {
  var link = '/api/playlists/' + playlistId + '/export?format=' + format;
  var a = document.createElement('a');
  a.href = link;
  a.download = '';
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
}

async function deletePlaylistFolder(folderId) {
  try {
    await api('/api/playlist-folders/'+folderId, {method:'DELETE'});
    loadPlaylists();
  } catch(e) { alert('Error: '+e.message); }
}

async function deletePlaylist(id) {
  if (!confirm('Delete this playlist?')) return;
  try { await api('/api/playlists/'+id,{method:'DELETE'}); await loadPlaylists(); if (state.currentView==='playlist'&&state.currentData===id) navigate('albums'); }
  catch(e) { alert('Error: '+e.message); }
}

function showNewPlaylistForm(trackId, trackIds) {
  var ids = trackIds || (trackId != null ? [trackId] : []);
  var html = '<h2>New Playlist</h2><input type="text" id="new-pl-name" placeholder="Playlist name" autofocus>'+
    '<div class="modal-actions"><button onclick="createAndAddToPlaylist('+(ids.length ? JSON.stringify(ids) : trackId)+')" class="btn-primary">Create &amp; Add</button><button onclick="closeModal()" class="btn-secondary">Cancel</button></div>';
  showModal(html);
  setTimeout(function(){var inp=document.getElementById('new-pl-name');if(inp)inp.focus();},100);
}

window.createAndAddToPlaylist = createAndAddToPlaylist;
async function createAndAddToPlaylist(trackIdsOrId) {
  var name = document.getElementById('new-pl-name').value;
  if (!name.trim()) return;
  try {
    var pl = await apiJson('/api/playlists',{method:'POST',body:{name:name.trim()}});
    var ids = Array.isArray(trackIdsOrId) ? trackIdsOrId : (trackIdsOrId != null ? [trackIdsOrId] : []);
    for (var i = 0; i < ids.length; i++) {
      await api('/api/playlists/'+pl.id+'/tracks',{method:'POST',body:{track_id:ids[i]}});
    }
    closeModal(); await loadPlaylists();
    if (ids.length > 1) clearSelection();
  } catch(e) { alert('Error: '+e.message); }
}



async function addToPlaylist(playlistId, trackId) {
  try { await api('/api/playlists/'+playlistId+'/tracks',{method:'POST',body:{track_id:trackId}}); closeModal(); await loadPlaylists(); }
  catch(e) { alert('Error: '+e.message); }
}

function downloadTrack(trackId, title) {
  var a = document.createElement('a');
  a.href = '/api/stream/' + trackId;
  a.download = (title || 'track') + '.mp3';
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
}

function downloadTrackZip(trackId, title) {
  var a = document.createElement('a');
  a.href = '/api/download/track-zip/' + trackId;
  a.download = (title || 'track') + '.zip';
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
}

function showDownloadMenu(x, y, type, data) {
  var existing = document.getElementById('download-menu');
  if (existing) existing.remove();
  var menu = document.createElement('div');
  menu.id = 'download-menu';
  menu.className = 'context-menu';
  menu.style.cssText = 'display:block;position:fixed;left:'+x+'px;top:'+y+'px;z-index:300;';
  if (type === 'track') {
    menu.innerHTML =
      '<div class="context-menu-item" data-action="download-raw"><span class="cmi-icon">'+iconDownload()+'</span> Download Raw</div>'+
      '<div class="context-menu-item" data-action="download-zip"><span class="cmi-icon">'+iconAlbum()+'</span> Download as ZIP</div>';
    menu.querySelector('[data-action="download-raw"]').addEventListener('click', function() { downloadTrack(data.id, data.title); menu.remove(); });
    menu.querySelector('[data-action="download-zip"]').addEventListener('click', function() { downloadTrackZip(data.id, data.title); menu.remove(); });
  } else if (type === 'album') {
    menu.innerHTML =
      '<div class="context-menu-item" data-action="download-raw"><span class="cmi-icon">'+iconDownload()+'</span> Download Raw (all files)</div>'+
      '<div class="context-menu-item" data-action="download-zip"><span class="cmi-icon">'+iconAlbum()+'</span> Download as ZIP</div>';
    menu.querySelector('[data-action="download-raw"]').addEventListener('click', function() { downloadAlbumRaw(data.album, data.artist); menu.remove(); });
    menu.querySelector('[data-action="download-zip"]').addEventListener('click', function() { downloadAlbum(data.album, data.artist); menu.remove(); });
  } else if (type === 'playlist') {
    menu.innerHTML =
      '<div class="context-menu-item" data-action="download-raw"><span class="cmi-icon">'+iconDownload()+'</span> Download Raw (all files)</div>'+
      '<div class="context-menu-item" data-action="download-zip"><span class="cmi-icon">'+iconAlbum()+'</span> Download as ZIP</div>';
    menu.querySelector('[data-action="download-raw"]').addEventListener('click', function() { downloadPlaylistRaw(data.id); menu.remove(); });
    menu.querySelector('[data-action="download-zip"]').addEventListener('click', function() { downloadPlaylistZip(data.id, data.name); menu.remove(); });
  }
  document.body.appendChild(menu);
  setTimeout(function() { document.addEventListener('click', function cleanup() { menu.remove(); document.removeEventListener('click', cleanup); }); }, 0);
}

function downloadAlbumRaw(albumName, artistName) {
  apiJson('/api/albums/tracks?album='+encodeURIComponent(albumName)+(artistName?'&artist='+encodeURIComponent(artistName):'')).then(function(tracks) {
    tracks.forEach(function(t) { downloadTrack(t.id, t.title); });
  }).catch(function() {});
}

function downloadPlaylistRaw(playlistId) {
  apiJson('/api/playlists/'+playlistId+'/tracks').then(function(tracks) {
    tracks.forEach(function(t) { downloadTrack(t.id, t.title); });
  }).catch(function() {});
}

function downloadPlaylistZip(playlistId, name) {
  var a = document.createElement('a');
  a.href = '/api/download/playlist/' + playlistId;
  a.download = (name || 'playlist') + '.zip';
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
}

function togglePinPlaylist(id) {
  var idx = state.pinnedPlaylists.indexOf(id);
  if (idx === -1) { state.pinnedPlaylists.push(id); } else { state.pinnedPlaylists.splice(idx, 1); }
  localStorage.setItem('moidify_pinned', JSON.stringify(state.pinnedPlaylists));
  loadPlaylists();
  renderPinnedPlaylists();
}

function renderPinnedPlaylists() {
  var container = document.getElementById('pinned-section');
  if (!container) return;
  if (!state.pinnedPlaylists.length || !state.playlists.length) { container.style.display = 'none'; container.innerHTML = ''; return; }
  var pinned = state.playlists.filter(function(pl) { return state.pinnedPlaylists.indexOf(pl.id) !== -1; });
  if (!pinned.length) { container.style.display = 'none'; container.innerHTML = ''; return; }
  container.style.display = 'block';
  container.innerHTML = pinned.map(function(pl) {
    return '<div class="pinned-item" data-id="'+pl.id+'"><div class="pinned-label">'+esc(pl.name)+'</div></div>';
  }).join('');
  qsa('.pinned-item', container).forEach(function(el) {
    el.addEventListener('click', function() { navigate('playlist', parseInt(this.dataset.id)); });
  });
}

function downloadAlbum(albumName, artistName) {
  var url = '/api/download/album?album=' + encodeURIComponent(albumName);
  if (artistName) url += '&artist=' + encodeURIComponent(artistName);
  var a = document.createElement('a');
  a.href = url;
  a.download = albumName + '.zip';
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
}
