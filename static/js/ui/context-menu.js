function showAlbumContextMenu(event, album) {
  hideContextMenu();
  var menu = document.getElementById('context-menu');
  var x = event.clientX, y = event.clientY;
  var html = '';
  html += '<div class="context-menu-item" data-action="album-play"><span class="cmi-icon">'+iconPlay()+'</span> Play All</div>';
  html += '<div class="context-menu-item" data-action="album-shuffle"><span class="cmi-icon">'+iconShuffle()+'</span> Shuffle Album</div>';
  html += '<div class="context-menu-item" data-action="album-add-queue"><span class="cmi-icon">'+iconQueue()+'</span> Add to Queue</div>';
  html += '<div class="context-menu-divider"></div>';
  if (album.artist) html += '<div class="context-menu-item" data-action="album-go-artist"><span class="cmi-icon">'+iconArtist()+'</span> Go to Artist</div>';
  html += '<div class="context-menu-item" data-action="album-download"><span class="cmi-icon">'+iconDownload()+'</span> Download Album</div>';
  menu.innerHTML = html;
  menu.style.display = 'block';
  var mw = Math.min(260, menu.offsetWidth||220);
  var mh = menu.offsetHeight||300;
  var spaceBelow = window.innerHeight - y - 10;
  var spaceAbove = y - 10;
  if (mh > spaceBelow && spaceAbove > spaceBelow) {
    y = Math.max(10, y - mh);
  }
  if (x+mw>window.innerWidth) x=window.innerWidth-mw-10;
  if (y+mh>window.innerHeight) y=window.innerHeight-mh-10;
  if (x<10)x=10; if(y<10)y=10;
  menu.style.left=x+'px'; menu.style.top=y+'px';

  qsa('.context-menu-item', menu).forEach(function(el) {
    el.addEventListener('click', function() {
      var action = this.dataset.action;
      if (action === 'album-play') {
        apiJson('/api/albums/tracks?album='+encodeURIComponent(album.album)+(album.artist?'&artist='+encodeURIComponent(album.artist):'')).then(function(tracks) {
          playFromQueue(tracks, 0);
        }).catch(function() { showToast('Failed to load album', 'error'); });
      } else if (action === 'album-shuffle') {
        apiJson('/api/albums/tracks?album='+encodeURIComponent(album.album)+(album.artist?'&artist='+encodeURIComponent(album.artist):'')).then(function(tracks) {
          var shuffled = tracks.slice();
          for (var i = shuffled.length - 1; i > 0; i--) {
            var j = Math.floor(Math.random() * (i + 1));
            var tmp = shuffled[i]; shuffled[i] = shuffled[j]; shuffled[j] = tmp;
          }
          playFromQueue(shuffled, 0);
        }).catch(function() { showToast('Failed to load album', 'error'); });
      } else if (action === 'album-add-queue') {
        apiJson('/api/albums/tracks?album='+encodeURIComponent(album.album)+(album.artist?'&artist='+encodeURIComponent(album.artist):'')).then(function(tracks) {
          tracks.forEach(function(t) { addTrackToQueueEnd(t); });
        }).catch(function() { showToast('Failed to load album', 'error'); });
      } else if (action === 'album-go-artist') {
        navigate('artist-tracks', album.artist);
      } else if (action === 'album-download') {
        hideContextMenu();
        showDownloadMenu(e.clientX, e.clientY, 'album', {album: album.album, artist: album.artist});
      }
      hideContextMenu();
    });
  });
  document.addEventListener('click', hideContextMenuOnce);
}

function showArtistContextMenu(event, artist) {
  hideContextMenu();
  var menu = document.getElementById('context-menu');
  var x = event.clientX, y = event.clientY;
  var html = '';
  html += '<div class="context-menu-item" data-action="artist-play"><span class="cmi-icon">'+iconPlay()+'</span> Play All</div>';
  html += '<div class="context-menu-item" data-action="artist-shuffle"><span class="cmi-icon">'+iconShuffle()+'</span> Shuffle Artist</div>';
  html += '<div class="context-menu-item" data-action="artist-add-queue"><span class="cmi-icon">'+iconQueue()+'</span> Add to Queue</div>';

  menu.innerHTML = html;
  menu.style.display = 'block';
  var mw = Math.min(260, menu.offsetWidth||220);
  var mh = menu.offsetHeight||200;
  var spaceBelow = window.innerHeight - y - 10;
  var spaceAbove = y - 10;
  if (mh > spaceBelow && spaceAbove > spaceBelow) {
    y = Math.max(10, y - mh);
  }
  if (x+mw>window.innerWidth) x=window.innerWidth-mw-10;
  if (y+mh>window.innerHeight) y=window.innerHeight-mh-10;
  if (x<10)x=10; if(y<10)y=10;
  menu.style.left=x+'px'; menu.style.top=y+'px';

  qsa('.context-menu-item', menu).forEach(function(el) {
    el.addEventListener('click', function() {
      var action = this.dataset.action;
      if (action === 'artist-play') {
        apiJson('/api/artists/tracks?artist='+encodeURIComponent(artist.artist)).then(function(tracks) {
          playFromQueue(tracks, 0);
        }).catch(function() { showToast('Failed to load artist tracks', 'error'); });
      } else if (action === 'artist-shuffle') {
        apiJson('/api/artists/tracks?artist='+encodeURIComponent(artist.artist)).then(function(tracks) {
          var shuffled = tracks.slice();
          for (var i = shuffled.length - 1; i > 0; i--) {
            var j = Math.floor(Math.random() * (i + 1));
            var tmp = shuffled[i]; shuffled[i] = shuffled[j]; shuffled[j] = tmp;
          }
          playFromQueue(shuffled, 0);
        }).catch(function() { showToast('Failed to load artist tracks', 'error'); });
      } else if (action === 'artist-add-queue') {
        apiJson('/api/artists/tracks?artist='+encodeURIComponent(artist.artist)).then(function(tracks) {
          tracks.forEach(function(t) { addTrackToQueueEnd(t); });
        }).catch(function() { showToast('Failed to load artist tracks', 'error'); });
      }
      hideContextMenu();
    });
  });
  document.addEventListener('click', hideContextMenuOnce);
}

function showContextMenu(event, track, queue, index) {
  hideContextMenu();
  var menu = document.getElementById('context-menu');
  var x = event.clientX, y = event.clientY;
  var html = '';

  if (state.queue!==queue || state.currentIndex!==index) {
    html += '<div class="context-menu-item" data-action="play"><span class="cmi-icon">'+iconPlay()+'</span> Play</div>';
  }
  html += '<div class="context-menu-item" data-action="play-next"><span class="cmi-icon">'+iconForward()+'</span> Play Next</div>';
  html += '<div class="context-menu-item" data-action="add-queue"><span class="cmi-icon">'+iconQueue()+'</span> Add to Queue</div>';
  html += '<div class="context-menu-divider"></div>';

  var isFaved = state.favedTracks[track.id];
  html += '<div class="context-menu-item" data-action="toggle-fav"><span class="cmi-icon">'+(isFaved?iconHeartFilled():iconHeart())+'</span> '+(isFaved?'Remove from Liked Songs':'Like')+'</div>';

  if (track.album) html += '<div class="context-menu-item" data-action="go-album"><span class="cmi-icon">'+iconAlbum()+'</span> Go to Album</div>';
  html += '<div class="context-menu-item" data-action="go-artist"><span class="cmi-icon">'+iconArtist()+'</span> Go to Artist</div>';

  html += '<div class="context-menu-divider"></div>';
  html += '<div class="context-menu-item" data-action="download"><span class="cmi-icon">'+iconDownload()+'</span> Download</div>';
  if (track.album) {
    html += '<div class="context-menu-item" data-action="download-album"><span class="cmi-icon">'+iconDownload()+'</span> Download Album</div>';
  }
  html += '<div class="context-menu-item" data-action="share-track"><span class="cmi-icon">'+iconShare()+'</span> Copy Track Link</div>';

  if (queue===state.queue && state.currentIndex>=0 && index>state.currentIndex) {
    html += '<div class="context-menu-divider"></div>';
    html += '<div class="context-menu-item" data-action="remove-queue" data-qi="'+index+'"><span class="cmi-icon">'+iconClose()+'</span> Remove from Queue</div>';
  }

  if (state.user && state.playlists.length>0) {
    html += '<div class="context-menu-divider"></div>';
    html += '<div class="context-menu-item" style="cursor:default;color:var(--text-muted);font-size:12px;text-transform:uppercase;letter-spacing:1px;">Add to Playlist</div>';
    state.playlists.forEach(function(pl){html+='<div class="context-menu-item" data-action="add-pl" data-pl="'+pl.id+'"><span class="cmi-icon">'+iconPlus()+'</span> '+esc(pl.name)+'</div>';});
  }

  menu.innerHTML = html;
  menu.style.display = 'block';
  var mw = Math.min(280, menu.offsetWidth||220);
  var mh = menu.offsetHeight||400;
  var spaceBelow = window.innerHeight - y - 10;
  var spaceAbove = y - 10;
  if (mh > spaceBelow && spaceAbove > spaceBelow) {
    y = Math.max(10, y - mh);
  }
  if (x+mw>window.innerWidth) x=window.innerWidth-mw-10;
  if (y+mh>window.innerHeight) y=window.innerHeight-mh-10;
  if (x<10)x=10; if(y<10)y=10;
  menu.style.left=x+'px'; menu.style.top=y+'px';

  qsa('.context-menu-item', menu).forEach(function(el) {
    el.addEventListener('click', function() {
      var action = this.dataset.action;
      if (action==='play') playFromQueue(queue, index);
      else if (action==='play-next') { addTrackToQueueNext(track); showToast('Added to queue', 'success'); }
      else if (action==='add-queue') { addTrackToQueueEnd(track); showToast('Added to queue', 'success'); }
      else if (action==='toggle-fav') { toggleFavorite(track.id); }
      else if (action==='add-pl') { addToPlaylist(parseInt(this.dataset.pl), track.id); showToast('Added to playlist', 'success'); }
      else if (action==='remove-queue') { removeFromQueue(parseInt(this.dataset.qi)); showToast('Removed from queue', 'info'); }
      else if (action==='go-album') navigate('album',{album:track.album,artist:track.artist});
      else if (action==='go-artist') navigate('artist-tracks',track.artist);
      else if (action==='download') { hideContextMenu(); showDownloadMenu(x, y, 'track', {id: track.id, title: track.title}); return; }
      else if (action==='download-album') { hideContextMenu(); showDownloadMenu(x, y, 'album', {album: track.album, artist: track.artist}); return; }
      else if (action==='share-track') { copyTrackLink(track); }
      hideContextMenu();
    });
  });
  document.addEventListener('click', hideContextMenuOnce);
}

function hideContextMenu() {
  var menu = document.getElementById('context-menu');
  menu.style.display = 'none'; menu.innerHTML = '';
  document.removeEventListener('click', hideContextMenuOnce);
}

function hideContextMenuOnce(e) {
  if (!e.target.closest('#context-menu')) {
    hideContextMenu();
    document.removeEventListener('click', hideContextMenuOnce);
  }
}

var longPressTimer = null;
function setupLongPress(el, callback) {
  el.addEventListener('touchstart', function(e) {
    longPressTimer = setTimeout(function() {
      longPressTimer = null;
      callback(e);
    }, 500);
  });
  el.addEventListener('touchend', function() {
    if (longPressTimer) { clearTimeout(longPressTimer); longPressTimer = null; }
  });
  el.addEventListener('touchmove', function() {
    if (longPressTimer) { clearTimeout(longPressTimer); longPressTimer = null; }
  });
}
