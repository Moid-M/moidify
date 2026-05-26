function setupEvents() {
  document.getElementById('play-btn').addEventListener('click', togglePlay);
  document.getElementById('next-btn').addEventListener('click', nextTrack);
  document.getElementById('prev-btn').addEventListener('click', prevTrack);
  document.getElementById('shuffle-btn').addEventListener('click', toggleShuffle);
  document.getElementById('repeat-btn').addEventListener('click', cycleRepeat);
  document.getElementById('eq-btn').addEventListener('click', openEQPanel);
  var eqOverlay = document.getElementById('eq-overlay');
  document.getElementById('close-eq-btn').addEventListener('click', closeEQPanel);
  eqOverlay.addEventListener('click', function(e) { if (e.target === eqOverlay) closeEQPanel(); });
  document.getElementById('lyrics-btn').addEventListener('click', toggleLyrics);
  document.getElementById('viz-toggle-btn').addEventListener('click', toggleLyricsVisualizer);
  var lyricsOverlay = document.getElementById('lyrics-overlay');
  var closeLyrics = function() {
    lyricsOverlay.style.display='none'; qs('#lyrics-btn').classList.remove('active');
    clearLyricsSync(); stopLyricsVisualizer(); vizActive=false;
    var panel = document.getElementById('lyrics-panel');
    if (panel) panel.classList.remove('viz-open');
    var canvas = document.getElementById('lyrics-visualizer');
    if (canvas) canvas.style.display = 'none';
    var btn = document.getElementById('viz-toggle-btn');
    if (btn) btn.classList.remove('active');
  };
  document.getElementById('close-lyrics-btn').addEventListener('click', closeLyrics);
  lyricsOverlay.addEventListener('click', function(e) { if (e.target === lyricsOverlay) closeLyrics(); });
  document.getElementById('queue-toggle-btn').addEventListener('click', toggleQueuePanel);
  document.getElementById('close-queue-btn').addEventListener('click', toggleQueuePanel);
  document.getElementById('clear-queue-btn').addEventListener('click', clearQueue);
  document.getElementById('settings-btn').addEventListener('click', showSettings);
  document.getElementById('player-title').addEventListener('click', function(e) {
    e.stopPropagation();
    if (state.queue[state.currentIndex]) {
      showTrackInfo(state.queue[state.currentIndex]);
    }
  });
  document.getElementById('player-artist').addEventListener('click', function(e) {
    e.stopPropagation();
    var track = state.queue[state.currentIndex];
    if (track && track.artist) navigate('artist-tracks', track.artist);
  });
  document.getElementById('player-cover').addEventListener('click', function(e) {
    e.stopPropagation();
    var track = state.queue[state.currentIndex];
    if (track && track.album) navigate('album', {album: track.album, artist: track.artist});
    else toggleNowPlaying();
  });
  document.getElementById('player-cover-expand').addEventListener('click', function(e) {
    e.stopPropagation();
    var track = state.queue[state.currentIndex];
    if (track) openFullScreenArt(track.id);
  });
  qs('.player-left').addEventListener('dblclick', function() {
    toggleNowPlaying();
  });
  document.getElementById('close-np-btn').addEventListener('click', toggleNowPlaying);
  var npOverlay = document.getElementById('nowplaying-overlay');
  npOverlay.addEventListener('click', function(e) { if (e.target === npOverlay) toggleNowPlaying(); });
  document.getElementById('np-play').addEventListener('click', togglePlay);
  document.getElementById('np-next').addEventListener('click', nextTrack);
  document.getElementById('np-prev').addEventListener('click', prevTrack);
  document.getElementById('np-cover').addEventListener('click', function() {
    var track = state.queue[state.currentIndex];
    if (track) openFullScreenArt(track.id);
  });
  document.getElementById('fs-art-close').addEventListener('click', closeFullScreenArt);
  document.getElementById('fs-art').addEventListener('click', function(e) {
    if (e.target === this) closeFullScreenArt();
  });

  setupHoldRepeat('rewind-btn', function(){seekRelative(-10);});
  setupHoldRepeat('forward-btn', function(){seekRelative(10);});
  document.getElementById('sleep-timer-cancel').addEventListener('click', cancelSleepTimer);

  document.getElementById('volume').addEventListener('input', function() {
    audio.volume = parseFloat(this.value);
    updateVolumeFill();
    audio.muted = false;
  });
  // Mini player
  var miniPlayer = document.getElementById('mini-player');
  var miniCover = document.getElementById('mini-cover');
  var miniTitle = document.getElementById('mini-title');
  var miniArtist = document.getElementById('mini-artist');
  var miniPlay = document.getElementById('mini-play');
  var miniNext = document.getElementById('mini-next');
  var miniClose = document.getElementById('mini-close');
  var miniBtn = document.getElementById('mini-player-btn');

  function updateMiniPlayer() {
    var track = state.queue[state.currentIndex];
    if (!track) { miniPlayer.style.display = 'none'; return; }
    miniCover.src = '/api/cover/' + track.id;
    miniTitle.textContent = track.title || '';
    miniArtist.textContent = (track.artist || 'Unknown');
    miniPlay.innerHTML = audio.paused ? '<svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><polygon points="8,5 19,12 8,19"/></svg>' : '<svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>';
  }

  miniBtn.addEventListener('click', function() {
    if (!state.queue.length) return;
    var show = miniPlayer.style.display === 'none';
    miniPlayer.style.display = show ? 'block' : 'none';
    if (show) updateMiniPlayer();
  });
  miniPlay.addEventListener('click', function(e) { e.stopPropagation(); togglePlay(); setTimeout(updateMiniPlayer, 50); });
  miniNext.addEventListener('click', function(e) { e.stopPropagation(); nextTrack(); });
  miniClose.addEventListener('click', function() { miniPlayer.style.display = 'none'; });

  document.getElementById('volume-btn').addEventListener('click', function() {
    audio.muted = !audio.muted;
    this.classList.toggle('muted', audio.muted);
  });

  var speeds = [0.5, 0.75, 1, 1.25, 1.5, 2];
  var speedBtn = document.getElementById('speed-btn');
  function updateSpeedBtn() {
    speedBtn.textContent = state.playbackSpeed + 'x';
    localStorage.setItem('moidify_speed', state.playbackSpeed);
    audio.playbackRate = state.playbackSpeed;
  }
  updateSpeedBtn();
  speedBtn.addEventListener('click', function() {
    var idx = speeds.indexOf(state.playbackSpeed);
    state.playbackSpeed = speeds[(idx + 1) % speeds.length];
    updateSpeedBtn();
    showToast('Speed: ' + state.playbackSpeed + 'x', 'info');
  });

  document.getElementById('seek').addEventListener('input', function() {
    if (audio.duration) {
      audio.currentTime = (parseFloat(this.value)/100)*audio.duration;
      document.documentElement.style.setProperty('--seek-pct', this.value + '%');
    }
  });
  document.getElementById('np-seek').addEventListener('input', function() {
    if (audio.duration) {
      audio.currentTime = (parseFloat(this.value)/100)*audio.duration;
      document.documentElement.style.setProperty('--seek-pct', this.value + '%');
    }
  });

  document.getElementById('queue-seek').addEventListener('input', function() {
    if (audio.duration) {
      audio.currentTime = (parseFloat(this.value)/100)*audio.duration;
      document.documentElement.style.setProperty('--seek-pct', this.value + '%');
    }
  });

  document.getElementById('login-btn').addEventListener('click', showLoginModal);
  document.getElementById('register-btn').addEventListener('click', showRegisterModal);
  document.getElementById('logout-btn').addEventListener('click', logout);
  document.getElementById('new-playlist-btn').addEventListener('click', function() {
    if (!state.user) { showLoginModal(); return; }
    showNewPlaylistForm(null);
  });
  document.getElementById('new-playlist-folder-btn').addEventListener('click', function() {
    if (!state.user) { showLoginModal(); return; }
    var name = prompt('Folder name:');
    if (name && name.trim()) {
      api('/api/playlist-folders', {method:'POST', body:{name: name.trim()}}).then(function() {
        loadPlaylists();
      }).catch(function(e) { alert('Error: '+e.message); });
    }
  });
  document.getElementById('import-playlist-btn').addEventListener('click', function() {
    if (!state.user) { showLoginModal(); return; }
    var input = document.createElement('input');
    input.type = 'file';
    input.accept = '.m3u,.m3u8,.json,.txt';
    input.addEventListener('change', function() {
      var file = input.files[0];
      if (!file) return;
      var formData = new FormData();
      formData.append('file', file);
      fetch('/api/playlists/import', { method:'POST', headers: state.token ? { 'token': state.token } : {}, body: formData })
        .then(function(r) { return r.json(); })
        .then(function(d) {
          if (d.ok) {
            loadPlaylists();
            navigate('playlist', d.playlist_id);
            showModal('<div style="text-align:center;padding:20px;"><h2>Import complete</h2><p style="color:var(--text-secondary);margin-top:8px;">Matched '+d.matched+' of '+d.total+' tracks</p><div class="modal-actions" style="margin-top:16px;"><button onclick="closeModal()" class="btn-secondary">OK</button></div></div>');
          } else {
            alert('Import failed: ' + (d.detail || 'Unknown error'));
          }
        }).catch(function(e) { alert('Error: '+e.message); });
    });
    input.click();
  });

  document.getElementById('selection-bar').addEventListener('click', function(e) {
    var btn = e.target.closest('[data-sel]');
    if (!btn) return;
    var action = btn.dataset.sel;
    var trackIds = getSelectedTracks();
    if (!trackIds.length) return;
    if (action === 'clear') { clearSelection(); }
    else if (action === 'play') {
      var rows = qsa('.track-row');
      var selectedTracks = [];
      rows.forEach(function(r) {
        if (state.selectedTrackIds.indexOf(parseInt(r.dataset.trackId)) !== -1) {
          var idx = parseInt(r.dataset.index);
          selectedTracks.push(idx);
        }
      });
      selectedTracks.sort(function(a,b){return a-b;});
      var newQueue = [];
      selectedTracks.forEach(function(i) {
        var r = qs('[data-index="'+i+'"]');
        newQueue.push({ id: parseInt(r.dataset.trackId), title: qs('.track-title', r).textContent, artist: qs('.track-artist', r).textContent });
      });
      clearSelection();
      if (newQueue.length) playFromQueue(newQueue, 0);
    } else if (action === 'queue') {
      addMultipleToQueue(trackIds);
      clearSelection();
    } else if (action === 'playlist') {
      if (!state.user) { showLoginModal(); return; }
      showNewPlaylistForm(null, trackIds);
    } else if (action === 'download') {
      trackIds.forEach(function(id) {
        var row = qs('[data-track-id="'+id+'"]');
        var title = row ? qs('.track-title', row).textContent : 'track';
        downloadTrack(id, title);
      });
      clearSelection();
    }
  });

  qsa('.nav-item').forEach(function(el) {
    el.addEventListener('click', function() { navigate(this.dataset.view); });
  });

  document.addEventListener('click', function(e) {
    if (state.selectedTrackIds.length && !e.target.closest('.track-row') && !e.target.closest('#selection-bar')) {
      clearSelection();
    }
  });

  audio.addEventListener('timeupdate', function() {
    if (audio.duration) {
      if (!state.smoothSeek) {
        var pct = (audio.currentTime / audio.duration) * 100;
        document.getElementById('seek').value = pct;
        document.getElementById('queue-seek').value = pct;
        document.getElementById('current-time').textContent = formatTime(audio.currentTime);
        document.getElementById('queue-current-time').textContent = formatTime(audio.currentTime);
        updateSeekFill();
      }
      renderSleepTimer();
      updateSyncedLyrics();
      var npOverlay = document.getElementById('nowplaying-overlay');
      if (npOverlay && npOverlay.style.display !== 'none') {
        updateNowPlayingProgress();
      }
    }
  });

  audio.addEventListener('loadedmetadata', function() {
    var d = formatTime(audio.duration);
    document.getElementById('total-time').textContent = d;
    document.getElementById('queue-total-time').textContent = d;
  });

  audio.addEventListener('ended', function() {
    nextTrack();
  });

  audio.addEventListener('play', function() {
    qs('#play-btn').innerHTML = iconPause();
    document.getElementById('np-play').innerHTML = iconPause();
    applyAnimations();
  });
  audio.addEventListener('pause', function() {
    qs('#play-btn').innerHTML = iconPlay();
    document.getElementById('np-play').innerHTML = iconPlay();
    applyAnimations();
  });
}

window.doLogin = async function() {
  var u=document.getElementById('login-username').value, p=document.getElementById('login-password').value;
  if (!u||!p) return; try{await login(u,p);}catch(e){alert(e.message);}
};
window.doRegister = async function() {
  var u=document.getElementById('reg-username').value, p=document.getElementById('reg-password').value, e=document.getElementById('reg-email').value||null;
  if (!u||!p) return; try{await register(u,p,e);}catch(ex){alert(ex.message);}
};
window.showNewPlaylistForm = showNewPlaylistForm;
window.closeModal = closeModal;
window.cancelSleepTimer = cancelSleepTimer;
window.showSleepTimerPicker = showSleepTimerPicker;

function initResizable(handleId, targetSelector, cssVar, minW, maxW, storageKey) {
  var handle = document.getElementById(handleId);
  if (!handle) return;
  var saved = localStorage.getItem(storageKey);
  if (saved) {
    document.documentElement.style.setProperty(cssVar, saved + 'px');
  }
  handle.addEventListener('mousedown', function(e) {
    e.preventDefault();
    document.body.classList.add('resizing');
    var target = document.querySelector(targetSelector);
    var startX = e.clientX;
    var startW = target.offsetWidth;
    function onMove(ev) {
      var w = cssVar === '--sidebar-w' ? startW + (ev.clientX - startX) : startW - (ev.clientX - startX);
      w = Math.max(minW, Math.min(maxW, w));
      document.documentElement.style.setProperty(cssVar, w + 'px');
    }
    function onUp() {
      document.body.classList.remove('resizing');
      var finalW = parseInt(document.documentElement.style.getPropertyValue(cssVar));
      localStorage.setItem(storageKey, finalW);
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    }
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  });
}

document.addEventListener('keydown', function(e) {
  if (e.key==='Enter' && document.getElementById('modal-overlay').style.display!=='none') {
    var btn = qs('#modal .btn-primary');
    if (btn) btn.click();
  }
  if (e.key==='Escape') clearSelection();
});

function initSeekWave() {
  var wave = document.getElementById('seek-wave');
  if (!wave) return;
  var ns = 'http://www.w3.org/2000/svg';
  var svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('viewBox', '0 0 200 16');
  svg.setAttribute('preserveAspectRatio', 'none');
  svg.style.width = '200%';
  svg.style.height = '100%';

  var path = document.createElementNS(ns, 'path');
  var d = 'M0 12';
  for (var x = 0; x <= 200; x += 1) {
    var y = 12 + 8 * Math.sin((x / 200) * 8 * Math.PI);
    d += ' L' + x + ' ' + y;
  }
  path.setAttribute('d', d);
  path.setAttribute('fill', 'none');
  path.setAttribute('stroke', 'var(--accent)');
  path.setAttribute('stroke-width', '1.5');
  path.setAttribute('stroke-linecap', 'round');
  svg.appendChild(path);

  var pos = 0;
  function tick() {
    pos -= 0.4;
    svg.style.transform = 'translateX(' + pos + 'px)';
    if (pos <= -100) pos = 0;
    requestAnimationFrame(tick);
  }

  wave.appendChild(svg);
  requestAnimationFrame(tick);
}

function initSeekSmooth() {
  var a = document.getElementById('audio');
  if (!a) return;
  if (!state.smoothSeek) return;
  var curTime = document.getElementById('current-time');
  var queueCur = document.getElementById('queue-current-time');
  var npCur = document.getElementById('np-current-time');
  var npSeek = document.getElementById('np-seek');
  function tick() {
    if (a.duration && !a.paused) {
      var pct = (a.currentTime / a.duration) * 100;
      document.documentElement.style.setProperty('--seek-pct', pct + '%');
      var t = formatTime(a.currentTime);
      curTime.textContent = t;
      queueCur.textContent = t;
      if (npCur) npCur.textContent = t;
      if (npSeek) { npSeek.value = pct; npSeek.style.setProperty('--seek-pct', pct + '%'); }
    }
    requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);
}

var _sessionSaveTimer = null;

function saveSession() {
  if (!state.queue.length || state.currentIndex < 0) return;
  try {
    var session = {
      queue: state.queue.slice(0, 200),
      currentIndex: state.currentIndex,
      currentTime: audio.currentTime || 0,
      shuffle: state.shuffle,
      repeatMode: state.repeatMode,
      shuffleOrder: state.shuffleOrder,
      shuffleIndex: state.shuffleIndex,
      timestamp: Date.now(),
    };
    localStorage.setItem('moidify_session', JSON.stringify(session));
  } catch(e) { /* storage full, ignore */ }
}

function scheduleSessionSave() {
  if (_sessionSaveTimer) clearInterval(_sessionSaveTimer);
  _sessionSaveTimer = setInterval(saveSession, 10000);
}

function restoreSession() {
  var raw = localStorage.getItem('moidify_session');
  if (!raw) return;
  try {
    var session = JSON.parse(raw);
    if (!session.queue || !session.queue.length) return;
    // Discard sessions older than 1 hour
    if (Date.now() - (session.timestamp || 0) > 3600000) {
      localStorage.removeItem('moidify_session');
      return;
    }
    // Restore queue and modes
    state.queue = session.queue;
    state.currentIndex = session.currentIndex;
    state.shuffle = session.shuffle || false;
    state.repeatMode = session.repeatMode || 'off';
    state.shuffleOrder = session.shuffleOrder || [];
    state.shuffleIndex = session.shuffleIndex || 0;
    renderRepeatShuffleButtons();
    // Start periodic saves so we always have fresh state
    scheduleSessionSave();
    // Restore playback after a short delay
    var savedTime = session.currentTime || 0;
    setTimeout(function() {
      if (state.currentIndex >= 0 && state.queue[state.currentIndex]) {
        loadAndPlay(state.queue[state.currentIndex], state.queue, state.currentIndex, savedTime);
      }
    }, 500);
    // Don't remove session yet — clear it after playback actually resumes
    var resumeCheck = setInterval(function() {
      if (audio.currentTime > 1 || audio.paused === false) {
        clearInterval(resumeCheck);
        localStorage.removeItem('moidify_session');
      }
    }, 1000);
    // Safety: clear after 30s regardless
    setTimeout(function() { localStorage.removeItem('moidify_session'); }, 30000);
  } catch(e) { console.warn('Session restore failed', e); }
}

function init() {
  applyAccent(state.accentColor);
  applyTheme();
  applyAnimationSettings();
  applyLanguage();
  applyFontSize(localStorage.getItem('moidify_font_size') || 'normal');
  var bgB = localStorage.getItem('moidify_bg_brightness');
  if (bgB) applyBgBrightness(parseInt(bgB));
  initResizable('sidebar-drag', '#sidebar', '--sidebar-w', 180, 400, 'moidify_sidebar_w');
  initResizable('queue-drag', '#queue-panel', '--queue-w', 220, 500, 'moidify_queue_w');
  setupSearch();
  setupEvents();
  setupKeyboard();
  renderRepeatShuffleButtons();
  updateVolumeFill();
  initSeekWave();
  initSeekSmooth();
  applyTrackCovers();
  checkAuth().then(function() {
    restoreSession();
    // always show albums view
    navigate('albums');
  });
  if (window.matchMedia) {
    var mq = window.matchMedia('(prefers-color-scheme: light)');
    mq.addEventListener('change', function() {
      if (state.autoTheme) applyTheme();
    });
  }
  window.addEventListener('beforeunload', saveSession);
  window.addEventListener('pagehide', saveSession);
  // Also save on timeupdate (every 10 seconds via the interval)
  scheduleSessionSave();
}

document.addEventListener('DOMContentLoaded', init);
