function copyTrackLink(track) {
  var url = window.location.origin + '/track/' + track.id;
  if (navigator.clipboard) {
    navigator.clipboard.writeText(url).then(function() {
      showToast('Track link copied', 'success');
    }).catch(function() {
      fallbackCopy(url);
    });
  } else {
    fallbackCopy(url);
  }
}
function fallbackCopy(text) {
  var ta = document.createElement('textarea');
  ta.value = text; ta.style.position='fixed'; ta.style.opacity='0';
  document.body.appendChild(ta); ta.select();
  try { document.execCommand('copy'); showToast('Track link copied', 'success'); }
  catch(e) { showToast('Failed to copy link', 'error'); }
  document.body.removeChild(ta);
}

function showModal(html) {
  var overlay = document.getElementById('modal-overlay');
  document.getElementById('modal-content').innerHTML = html;
  overlay.style.display = 'flex';
  overlay.classList.remove('overlay-out');
  overlay.classList.add('overlay-in');
  var modal = document.getElementById('modal');
  if (modal) { modal.classList.remove('modal-out'); modal.classList.add('modal-in'); }
  overlay.onclick = function(e) { if (e.target === overlay) closeModal(); };
}

var modalCloseTimer = null;
function closeModal() {
  var overlay = document.getElementById('modal-overlay');
  var modal = document.getElementById('modal');
  if (!overlay) return;
  overlay.classList.remove('overlay-in');
  overlay.classList.add('overlay-out');
  if (modal) { modal.classList.remove('modal-in'); modal.classList.add('modal-out'); }
  setTimeout(function() {
    overlay.style.display = 'none';
  }, 200);
}

function showLoginModal() {
  showModal('<h2>Log In</h2><input type="text" id="login-username" placeholder="Username" autofocus><input type="password" id="login-password" placeholder="Password"><div class="modal-actions"><button onclick="doLogin()" class="btn-primary">Log In</button><button onclick="closeModal()" class="btn-secondary">Cancel</button></div>');
  setTimeout(function(){var el=document.getElementById('login-username');if(el)el.focus();},100);
}

function showRegisterModal() {
  showModal('<h2>Register</h2><input type="text" id="reg-username" placeholder="Username" autofocus><input type="email" id="reg-email" placeholder="Email (optional)"><input type="password" id="reg-password" placeholder="Password"><div class="modal-actions"><button onclick="doRegister()" class="btn-primary">Register</button><button onclick="closeModal()" class="btn-secondary">Cancel</button></div>');
  setTimeout(function(){var el=document.getElementById('reg-username');if(el)el.focus();},100);
}

function showTrackInfo(track) {
  var html = '<div class="track-info-modal"><h2 data-i18n="trackInfo.title">Track Info</h2>'+
    '<div class="track-info-cover"><img src="/api/cover/'+track.id+'" alt="" onerror="this.style.display=\'none\'"></div>'+
    '<table class="track-info-table">'+
    '<tr><td data-i18n="common.track">Track</td><td>'+esc(track.title)+'</td></tr>'+
    (track.artist ? '<tr><td data-i18n="common.artist">Artist</td><td>'+esc(track.artist)+'</td></tr>' : '')+
    (track.album ? '<tr><td data-i18n="common.album">Album</td><td>'+esc(track.album)+'</td></tr>' : '')+
    (track.year ? '<tr><td data-i18n="common.year">Year</td><td>'+esc(track.year)+'</td></tr>' : '')+
    (track.track_number ? '<tr><td data-i18n="common.number">#</td><td>'+track.track_number+'</td></tr>' : '')+
    (track.genre ? '<tr><td>Genre</td><td>'+esc(track.genre)+'</td></tr>' : '')+
    (track.duration ? '<tr><td data-i18n="trackInfo.duration">Duration</td><td>'+formatTime(track.duration)+'</td></tr>' : '')+
    (track.file_path ? '<tr><td data-i18n="trackInfo.filePath">File Path</td><td style="font-size:11px;word-break:break-all;">'+esc(track.file_path)+'</td></tr>' : '')+
    (track.play_count != null ? '<tr><td data-i18n="trackInfo.playCount">Play Count</td><td>'+track.play_count+'</td></tr>' : '')+
    (track.created_at ? '<tr><td data-i18n="trackInfo.added">Added</td><td>'+esc(track.created_at)+'</td></tr>' : '')+
    '</table></div>'+
    '<div class="modal-actions"><button onclick="closeModal()" class="btn-secondary" data-i18n="common.close">Close</button></div>';
  showModal(html);
  translateDOM(document.getElementById('modal'));
}
