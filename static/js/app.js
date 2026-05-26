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
  document.getElementById('player-cover').addEventListener('click', toggleNowPlaying);
  document.getElementById('close-np-btn').addEventListener('click', toggleNowPlaying);
  var npOverlay = document.getElementById('nowplaying-overlay');
  npOverlay.addEventListener('click', function(e) { if (e.target === npOverlay) toggleNowPlaying(); });
  document.getElementById('np-play').addEventListener('click', togglePlay);
  document.getElementById('np-next').addEventListener('click', nextTrack);
  document.getElementById('np-prev').addEventListener('click', prevTrack);

  setupHoldRepeat('rewind-btn', function(){seekRelative(-10);});
  setupHoldRepeat('forward-btn', function(){seekRelative(10);});

  document.getElementById('volume').addEventListener('input', function() {
    audio.volume = parseFloat(this.value);
    updateVolumeFill();
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

  document.getElementById('fav-btn').addEventListener('click', function() {
    if (this.dataset.track) toggleFavorite(parseInt(this.dataset.track), this);
    else if (!state.user) showLoginModal();
  });

  document.getElementById('login-btn').addEventListener('click', showLoginModal);
  document.getElementById('register-btn').addEventListener('click', showRegisterModal);
  document.getElementById('logout-btn').addEventListener('click', logout);
  document.getElementById('new-playlist-btn').addEventListener('click', function() {
    if (!state.user) { showLoginModal(); return; }
    showNewPlaylistForm(null);
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

function init() {
  applyAccent(state.accentColor);
  applyTheme();
  applyAnimationSettings();
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
  checkAuth().then(function() { navigate('albums'); });
}

document.addEventListener('DOMContentLoaded', init);
