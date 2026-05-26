function playFromQueue(queue, index) {
  if (state.currentIndex >= 0 && state.currentIndex !== index && state.currentIndex < state.queue.length) {
    state.playHistory.push(state.currentIndex);
  }
  state.queue = queue; state.currentIndex = index;

  if (state.shuffle && (!state.shuffleOrder.length || state.shuffleOrder.length !== queue.length)) {
    state.shuffleOrder = generateShuffleOrder(queue.length);
    state.shuffleIndex = 0;
    for (var si = 0; si < state.shuffleOrder.length; si++) {
      if (state.shuffleOrder[si] === index) { state.shuffleIndex = si; break; }
    }
  }

  var track = queue[index];
  if (!track) return;

  audio.src = '/api/stream/' + track.id + '?quality=' + (state.streamQuality || 'high');
  audio.play();
  fetch('/api/play/' + track.id, {method:'POST'});

  updatePlayerUI(track);
  saveSession();
}

function loadAndPlay(track, queue, index, startTime) {
  if (state.currentIndex >= 0 && state.currentIndex !== index && state.currentIndex < state.queue.length) {
    state.playHistory.push(state.currentIndex);
  }
  state.queue = queue; state.currentIndex = index;

  if (state.shuffle && (!state.shuffleOrder.length || state.shuffleOrder.length !== queue.length)) {
    state.shuffleOrder = generateShuffleOrder(queue.length);
    state.shuffleIndex = 0;
    for (var si = 0; si < state.shuffleOrder.length; si++) {
      if (state.shuffleOrder[si] === index) { state.shuffleIndex = si; break; }
    }
  }

  if (!track) return;

  updatePlayerUI(track);
  audio.src = '/api/stream/' + track.id + '?quality=' + (state.streamQuality || 'high');
  audio.addEventListener('loadedmetadata', function onMeta() {
    audio.removeEventListener('loadedmetadata', onMeta);
    audio.currentTime = startTime || 0;
    audio.play();
  });
  fetch('/api/play/' + track.id, {method:'POST'});
}

function updatePlayerUI(track) {
  document.getElementById('player-title').textContent = track.title||'';
  document.getElementById('player-artist').textContent = (track.artist||'Unknown')+(track.album?'  '+track.album:'');
  document.getElementById('player-cover').src = '/api/cover/'+track.id;
  qs('#play-btn').innerHTML = iconPause();

  qsa('.track-row.playing').forEach(function(el){el.classList.remove('playing');});
  var rows = qsa('.track-row');
  if (rows[state.currentIndex]) rows[state.currentIndex].classList.add('playing');

  checkFavoriteStatus(track.id);
  renderQueuePanel();
  updateBackdrop(track.id);
  updateMediaSession(track);
  applyEQ();
  autoShowQueue();
  applyAnimations();
  refreshLyricsForCurrentTrack();
  var npOverlay = document.getElementById('nowplaying-overlay');
  if (npOverlay && npOverlay.style.display !== 'none') updateNowPlaying();
}

function togglePlay() {
  if (audio.paused && audio.src) { audio.play(); qs('#play-btn').innerHTML = iconPause(); }
  else if (!audio.paused) { audio.pause(); qs('#play-btn').innerHTML = iconPlay(); }
}

function nextTrack() {
  if (state.queue.length===0) return;
  if (state.repeatMode === 'one' && state.currentIndex >= 0) {
    audio.currentTime = 0; audio.play(); return;
  }

  if (state.currentIndex >= 0) state.playHistory.push(state.currentIndex);

  var next;
  if (state.shuffle && state.queue.length > 1 && state.shuffleOrder.length) {
    state.shuffleIndex++;
    if (state.shuffleIndex >= state.shuffleOrder.length) {
      state.shuffleIndex = 0;
      if (state.repeatMode === 'off') {
        state.currentIndex = -1; audio.src = ''; qs('#play-btn').innerHTML = iconPlay();
        renderQueuePanel(); checkSleepTimer('ended'); clearAnimations(); return;
      }
    }
    next = state.shuffleOrder[state.shuffleIndex];
  } else {
    next = state.currentIndex + 1;
    if (next >= state.queue.length) {
      if (state.repeatMode === 'all') { next = 0; }
      else {
        state.currentIndex = -1; audio.src = ''; qs('#play-btn').innerHTML = iconPlay();
        renderQueuePanel(); checkSleepTimer('ended'); clearAnimations(); return;
      }
    }
  }

  if (state.crossfade > 0 && audio.src) {
    crossfadeTo(state.queue, next);
  } else {
    playFromQueue(state.queue, next);
  }
}

function prevTrack() {
  if (state.queue.length===0) return;
  if (state.shuffle && state.playHistory.length) {
    var prev = state.playHistory.pop();
    playFromQueue(state.queue, prev);
    return;
  }
  var prev = state.currentIndex - 1;
  if (prev < 0) {
    if (state.repeatMode === 'all') { prev = state.queue.length - 1; }
    else {
      if (audio.currentTime > 3) { audio.currentTime = 0; return; }
      return;
    }
  }
  if (state.crossfade > 0 && audio.src) {
    crossfadeTo(state.queue, prev);
  } else {
    playFromQueue(state.queue, prev);
  }
}

function seekRelative(seconds) {
  if (!audio.duration) return;
  audio.currentTime = Math.max(0, Math.min(audio.duration, audio.currentTime + seconds));
}

function crossfadeTo(queue, index) {
  if (isCrossfading) return;
  isCrossfading = true;
  var duration = state.crossfade;
  var fadeInterval = 50;
  var steps = Math.floor((duration * 1000) / fadeInterval);
  var step = 0;

  var timer = setInterval(function() {
    step++;
    var progress = step / steps;
    var vol = Math.max(0, 1 - progress);
    try { audio.volume = vol; } catch(e) {}
    if (progress >= 1) {
      clearInterval(timer);
      audio.volume = parseFloat(document.getElementById('volume').value) || 1;
      playFromQueue(queue, index);
      var newVol = 0;
      var fadeIn = setInterval(function() {
        newVol += 0.05;
        var target = parseFloat(document.getElementById('volume').value) || 1;
        try { audio.volume = Math.min(target, newVol); } catch(e) {}
        if (newVol >= target) { clearInterval(fadeIn); isCrossfading = false; }
      }, fadeInterval);
    }
  }, fadeInterval);
}

function updateMediaSession(track) {
  if (!('mediaSession' in navigator)) return;
  navigator.mediaSession.metadata = new MediaMetadata({
    title: track.title || '',
    artist: track.artist || 'Unknown',
    album: track.album || '',
    artwork: [{ src: '/api/cover/'+track.id, sizes: '300x300', type: 'image/jpeg' }],
  });
  navigator.mediaSession.setActionHandler('play', function(){togglePlay();});
  navigator.mediaSession.setActionHandler('pause', function(){togglePlay();});
  navigator.mediaSession.setActionHandler('previoustrack', function(){prevTrack();});
  navigator.mediaSession.setActionHandler('nexttrack', function(){nextTrack();});
  navigator.mediaSession.setActionHandler('seekto', function(e){if(e.seekTime)audio.currentTime=e.seekTime;});
}

function getAudioCtx() {
  if (!audioCtx) {
    try {
      audioCtx = new (window.AudioContext || window.webkitAudioContext)();
      var source = audioCtx.createMediaElementSource(audio);
      gainNode = audioCtx.createGain();
      gainNode.gain.value = 1;
      source.connect(gainNode);
      buildEQChain();
    } catch(e) { return null; }
  }
  if (audioCtx.state === 'suspended') audioCtx.resume();
  return audioCtx;
}

function buildEQChain() {
  if (!gainNode) return;
  eqNodes.forEach(function(n){n.disconnect();});
  eqNodes = [];

  var freqs = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
  var prevNode = gainNode;

  freqs.forEach(function(freq) {
    var filter = audioCtx.createBiquadFilter();
    filter.type = 'peaking';
    filter.frequency.value = freq;
    filter.Q.value = 1;
    filter.gain.value = 0;
    prevNode.connect(filter);
    eqNodes.push(filter);
    prevNode = filter;
  });

  // Create analyser for visualizer
  analyserNode = audioCtx.createAnalyser();
  analyserNode.fftSize = 128;
  prevNode.connect(analyserNode);
  analyserNode.connect(audioCtx.destination);
}

function applyEQ() {
  var values;
  if (state.eqPreset && EQ_PRESETS[state.eqPreset]) {
    values = EQ_PRESETS[state.eqPreset];
    if (state.eq) values = state.eq;
  } else if (state.eq) {
    values = state.eq;
  } else {
    values = EQ_PRESETS['Normal'];
  }
  try {
    var ctx = getAudioCtx();
    if (!ctx) return;
    eqNodes.forEach(function(node, i) {
      if (values[i] !== undefined) node.gain.value = values[i];
    });
  } catch(e) {}
}

function setEQPreset(name) {
  state.eqPreset = name;
  state.eq = null;
  localStorage.setItem('moidify_eq_preset', name);
  localStorage.removeItem('moidify_eq');
  applyEQ();
}

function setEQBand(index, value) {
  state.eqPreset = 'Custom';
  if (!state.eq) state.eq = EQ_PRESETS['Normal'].slice();
  state.eq[index] = value;
  localStorage.setItem('moidify_eq', JSON.stringify(state.eq));
  localStorage.setItem('moidify_eq_preset', 'Custom');
  applyEQ();
}

function setupHoldRepeat(btnId, fn) {
  var btn = document.getElementById(btnId);
  var interval = null;
  btn.addEventListener('mousedown', function(){fn();interval=setInterval(fn,200);});
  btn.addEventListener('mouseup', function(){clearInterval(interval);});
  btn.addEventListener('mouseleave', function(){clearInterval(interval);});
}

/* ---- Now Playing view ---- */
function toggleNowPlaying() {
  var overlay = document.getElementById('nowplaying-overlay');
  if (overlay.style.display !== 'none') {
    overlay.style.display = 'none';
    return;
  }
  if (!state.queue[state.currentIndex]) return;
  overlay.style.display = 'flex';
  updateNowPlaying();
}

function updateNowPlaying() {
  var track = state.queue[state.currentIndex];
  if (!track) return;
  document.getElementById('np-cover').src = '/api/cover/' + track.id;
  document.getElementById('np-title').textContent = track.title || '';
  document.getElementById('np-artist').textContent = (track.artist || 'Unknown') + (track.album ? ' \u2022 ' + track.album : '');
  document.getElementById('np-total-time').textContent = formatTime(audio.duration || track.duration || 0);
  updateNowPlayingProgress();
  renderNowPlayingLyrics();
}

function updateNowPlayingProgress() {
  if (!audio.duration) return;
  var pct = (audio.currentTime / audio.duration) * 100;
  document.getElementById('np-seek').value = pct;
  document.getElementById('np-current-time').textContent = formatTime(audio.currentTime);
  document.getElementById('np-seek').style.setProperty('--seek-pct', pct + '%');
}

function renderNowPlayingLyrics() {
  var container = document.getElementById('np-lyrics-content');
  var track = state.queue[state.currentIndex];
  if (!track) { container.innerHTML = ''; return; }
  if (window.currentLyrics && window.currentLyrics.length) {
    container.innerHTML = window.currentLyrics.map(function(l) {
      return '<div class="lyrics-line' + (l.active ? ' active' : '') + '">' + esc(l.text || l) + '</div>';
    }).join('');
  } else if (window.currentLyricsText) {
    container.innerHTML = '<div class="lyrics-line">' + window.currentLyricsText.replace(/\n/g, '<br>') + '</div>';
  } else {
    container.innerHTML = '<div class="lyrics-line" style="color:var(--text-muted);">Loading lyrics...</div>';
    fetchLyrics(track).then(function() { renderNowPlayingLyrics(); });
  }
}

function updateNowPlayingLyricsSync() {
  var container = document.getElementById('np-lyrics-content');
  if (!container || !window.currentLyrics) return;
  var lines = container.querySelectorAll('.lyrics-line');
  for (var i = 0; i < window.currentLyrics.length; i++) {
    if (lines[i]) {
      var isActive = window.currentLyrics[i].active;
      lines[i].className = 'lyrics-line' + (isActive ? ' active' : '');
      if (isActive) {
        lines[i].scrollIntoView({ block: 'center', behavior: 'smooth' });
      }
    }
  }
}
