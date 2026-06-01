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
  html += '<div class="context-menu-divider"></div>';
  html += '<div class="context-menu-item" style="cursor:default;color:var(--text-muted);font-size:12px;text-transform:uppercase;letter-spacing:1px;">Rate All Tracks</div>';
  for (var ari = 1; ari <= 5; ari++) {
    html += '<div class="context-menu-item" data-action="album-rate" data-rating="'+ari+'"><span class="cmi-icon" style="color:var(--accent);">★</span> '+ari+' Star'+(ari>1?'s':'')+'</div>';
  }

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
        downloadAlbum(album.album, album.artist);
      } else if (action === 'album-rate') {
        var rateVal = parseInt(this.dataset.rating);
        apiJson('/api/albums/tracks?album='+encodeURIComponent(album.album)+(album.artist?'&artist='+encodeURIComponent(album.artist):'')).then(function(tracks) {
          var done = 0;
          tracks.forEach(function(t) {
            api('/api/tracks/'+t.id+'/rating', { method:'PUT', body:{ rating: rateVal } }).then(function() {
              done++;
              if (done === tracks.length) {
                if (state.currentView === 'album' && state.currentData && state.currentData.album === album.album) {
                  navigate('album', state.currentData);
                }
              }
            }).catch(function(){ done++; });
          });
        });
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
  html += '<div class="context-menu-item" data-action="download"><span class="cmi-icon">'+iconDownload()+'</span> Download Song</div>';
  if (track.album) {
    html += '<div class="context-menu-item" data-action="download-album"><span class="cmi-icon">'+iconDownload()+'</span> Download Album</div>';
  }
  html += '<div class="context-menu-item" data-action="share-track"><span class="cmi-icon">'+iconShare()+'</span> Copy Track Link</div>';

  html += '<div class="context-menu-divider"></div>';
  var curRating = track.rating || 0;
  html += '<div class="context-menu-item" style="cursor:default;color:var(--text-muted);font-size:12px;text-transform:uppercase;letter-spacing:1px;">Rate</div>';
  html += '<div class="context-menu-rating" data-track="'+track.id+'">';
  for (var ri = 1; ri <= 5; ri++) {
    html += '<span class="cm-rating-star'+(ri<=curRating?' filled':'')+'" data-rating="'+ri+'">'+(ri<=curRating?'★':'☆')+'</span>';
  }
  html += '</div>';

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
      else if (action==='download') downloadTrack(track.id, track.title);
      else if (action==='download-album') downloadAlbum(track.album, track.artist);
      else if (action==='share-track') { copyTrackLink(track); }
      hideContextMenu();
    });
  });
  qsa('.cm-rating-star', menu).forEach(function(el) {
    el.addEventListener('click', function(e) {
      e.stopPropagation();
      var newRating = parseInt(this.dataset.rating);
      var currentRating = track.rating || 0;
      var finalRating = newRating === currentRating ? 0 : newRating;
      track.rating = finalRating;
      api('/api/tracks/'+track.id+'/rating', { method:'PUT', body:{ rating: finalRating } }).then(function() {
        qsa('.cm-rating-star', menu).forEach(function(st) {
          var r = parseInt(st.dataset.rating);
          st.textContent = r <= finalRating ? '★' : '☆';
          st.classList.toggle('filled', r <= finalRating);
        });
        var row = qs('[data-track-id="'+track.id+'"]');
        if (row) {
          qsa('.star-rating-star', row).forEach(function(st) {
            var r = parseInt(st.dataset.rating);
            st.textContent = r <= finalRating ? '★' : '☆';
            st.classList.toggle('filled', r <= finalRating);
          });
        }
        var favRow = qs('.fav-btn[data-track="'+track.id+'"]');
        if (favRow && favRow.closest('.track-row')) {
          var row2 = favRow.closest('.track-row');
          qsa('.star-rating-star', row2).forEach(function(st) {
            var r = parseInt(st.dataset.rating);
            st.textContent = r <= finalRating ? '★' : '☆';
            st.classList.toggle('filled', r <= finalRating);
          });
        }
      }).catch(function() { track.rating = currentRating; });
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
