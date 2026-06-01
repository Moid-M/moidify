var syncedLyrics = [];
var vizActive = false;
var lyricsVizRAF = null;
var lyricsSyncInterval = null;
var lyricsEditMode = false;
var lyricsSourceText = '';
var lyricsScrollRAF = null;

function toggleLyrics() {
  var overlay = document.getElementById('lyrics-overlay');
  if (overlay.style.display !== 'none') {
    overlay.style.display = 'none';
    qs('#lyrics-btn').classList.remove('active');
    clearLyricsSync();
    return;
  }
  overlay.style.display = 'flex';
  qs('#lyrics-btn').classList.add('active');
  startLyricsSync();
  if (state.queue[state.currentIndex]) {
    fetchLyrics(state.queue[state.currentIndex]);
  } else {
    document.getElementById('lyrics-content').innerHTML = '<div class="lyrics-placeholder"><p>No track playing</p></div>';
    document.getElementById('lyrics-source').textContent = '';
  }
}

function startLyricsSync() {
  clearLyricsSync();
  lyricsSyncInterval = setInterval(function() {
    updateSyncedLyrics();
  }, 80);
}

function clearLyricsSync() {
  if (lyricsSyncInterval) { clearInterval(lyricsSyncInterval); lyricsSyncInterval = null; }
}

async function fetchLyrics(track) {
  var content = document.getElementById('lyrics-content');
  document.getElementById('lyrics-title').textContent = track.title || '';
  document.getElementById('lyrics-artist').textContent = (track.artist || '') + (track.album ? '  ' + track.album : '');
  content.innerHTML = '<div class="lyrics-placeholder"><p>Searching for lyrics...</p></div>';
  document.getElementById('lyrics-source').textContent = '';
  syncedLyrics = [];
  lyricsEditMode = false;
  lyricsSourceText = '';
  var editBtn = document.getElementById('lyrics-edit-btn');
  var saveBtn = document.getElementById('lyrics-save-btn');
  if (editBtn) editBtn.classList.remove('active');
  if (saveBtn) saveBtn.style.display = 'none';

  var trackId = track.id || track.track_id;
  var lyricsText = null;
  var source = null;

  if (trackId) {
    try {
      var res = await fetch('/api/tracks/' + trackId + '/lyrics');
      if (res.ok) {
        var data = await res.json();
        if (data.lyrics && data.lyrics.trim()) {
          lyricsText = data.lyrics;
        }
      }
    } catch(e) {}
  }

  if (!lyricsText) {
    content.innerHTML = '<div class="lyrics-placeholder"><svg viewBox="0 0 24 24" width="48" height="48" fill="none" stroke="currentColor" stroke-width="1.5" opacity="0.3"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg><p>No lyrics found</p><p style="font-size:13px;color:var(--text-muted);margin-top:4px;">Click retry to search again</p><button onclick="fetchLyrics(state.queue[state.currentIndex])" class="btn-secondary" style="margin-top:12px;">Retry</button></div>';
    return;
  }

  document.getElementById('lyrics-source').textContent = source || '';
  lyricsSourceText = lyricsText;

  var syncedPattern = /\[(\d{2}):(\d{2})[\.:](\d{2,3})\]\s*(.*)/;
  var hasSynced = lyricsText.split('\n').some(function(line) { return syncedPattern.test(line); });

  if (hasSynced) {
    content.innerHTML = '';
    syncedLyrics = lyricsText.split('\n').map(function(line) {
      var match = line.match(syncedPattern);
      if (match) {
        var mins = parseInt(match[1]), secs = parseInt(match[2]), ms = parseInt(match[3]);
        if (match[3].length === 2) ms *= 10;
        return { time: mins * 60 + secs + ms / 1000, text: match[4].trim() };
      }
      return { time: -1, text: line.trim() };
    }).filter(function(l) { return l.text; });

    syncedLyrics.forEach(function(l) {
      var div = document.createElement('div');
      div.className = 'lyrics-line';
      div.dataset.time = l.time;
      div.innerHTML = '<span class="lyrics-text">' + esc(l.text) + '</span><span class="lyrics-progress-bar"></span>';
      content.appendChild(div);
    });
    window.currentLyrics = syncedLyrics;
    window.currentLyricsText = null;
    if (syncedLyrics.length === 0) {
      content.innerHTML = '<div class="lyrics-placeholder"><p>No lyrics available</p></div>';
    }
  } else {
    content.innerHTML = '<div class="lyrics-plain">' + esc(lyricsText).split('\n').map(function(p) {
      return '<p>' + esc(p) + '</p>';
    }).join('') + '</div>';
    window.currentLyrics = [];
    window.currentLyricsText = lyricsText;
  }
}

function refreshLyricsForCurrentTrack() {
  var overlay = document.getElementById('lyrics-overlay');
  if (overlay && overlay.style.display !== 'none' && state.queue[state.currentIndex]) {
    fetchLyrics(state.queue[state.currentIndex]);
  }
}

function toggleLyricsEdit() {
  lyricsEditMode = !lyricsEditMode;
  var editBtn = document.getElementById('lyrics-edit-btn');
  var saveBtn = document.getElementById('lyrics-save-btn');
  var content = document.getElementById('lyrics-content');
  if (!editBtn || !saveBtn) return;

  if (lyricsEditMode) {
    editBtn.classList.add('active');
    saveBtn.style.display = '';
    content.innerHTML = '<textarea id="lyrics-editor">' + esc(lyricsSourceText) + '</textarea>';
    var ta = document.getElementById('lyrics-editor');
    ta.addEventListener('keydown', function(e) {
      if ((e.ctrlKey || e.metaKey) && e.key === 's') {
        e.preventDefault();
        saveLyricsEdit();
      }
    });
    setTimeout(function() { ta.focus(); }, 100);
  } else {
    editBtn.classList.remove('active');
    saveBtn.style.display = 'none';
    if (lyricsSourceText) {
      fetchLyrics(state.queue[state.currentIndex]);
    } else {
      content.innerHTML = '<div class="lyrics-placeholder"><p>No lyrics to display</p></div>';
    }
  }
}

async function saveLyricsEdit() {
  var textarea = document.getElementById('lyrics-editor');
  if (!textarea) return;
  var track = state.queue[state.currentIndex];
  var trackId = track && (track.id || track.track_id);
  if (!trackId) return;
  var newText = textarea.value;
  try {
    var res = await fetch('/api/tracks/' + trackId + '/lyrics', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ lyrics: newText }),
    });
    if (res.ok) {
      showToast('Lyrics saved', 'success');
      lyricsSourceText = newText;
      lyricsEditMode = false;
      qs('#lyrics-edit-btn').classList.remove('active');
      document.getElementById('lyrics-save-btn').style.display = 'none';
      document.getElementById('lyrics-source').textContent = 'Edited';
      fetchLyrics(track);
    } else {
      showToast('Failed to save lyrics', 'error');
    }
  } catch(e) {
    showToast('Failed to save lyrics', 'error');
  }
}

function updateSyncedLyrics() {
  if (syncedLyrics.length === 0 || !audio.duration || !audio.currentTime) return;
  var currentTime = audio.currentTime;
  var activeIndex = -1;
  for (var i = 0; i < syncedLyrics.length; i++) {
    if (syncedLyrics[i].time <= currentTime) {
      activeIndex = i;
    }
  }

  var lines = qsa('.lyrics-line');
  var useFade = state.animations.lyricsFade !== false;
  var scrollTarget = null;
  lines.forEach(function(line, i) {
    if (i === activeIndex) {
      line.classList.add('active');
      line.style.opacity = '1';
      line.style.transform = 'scale(1)';
      if (!line.dataset.scrolled) {
        line.dataset.scrolled = '1';
        scrollTarget = line;
      }
    } else {
      line.classList.remove('active');
      line.dataset.scrolled = '';
      if (useFade) {
        var dist = Math.abs(i - activeIndex);
        if (dist === 1) {
          line.style.opacity = '0.5';
          line.style.transform = 'scale(0.95)';
        } else if (dist === 2) {
          line.style.opacity = '0.25';
          line.style.transform = 'scale(0.9)';
        } else {
          line.style.opacity = '0.08';
          line.style.transform = 'scale(0.85)';
        }
      } else {
        line.style.opacity = '1';
        line.style.transform = 'scale(1)';
      }
    }
  });

  if (activeIndex >= 0 && lines[activeIndex]) {
    var progressBar = lines[activeIndex].querySelector('.lyrics-progress-bar');
    if (progressBar) {
      var lineStart = syncedLyrics[activeIndex].time;
      var lineEnd = activeIndex < syncedLyrics.length - 1
        ? syncedLyrics[activeIndex + 1].time
        : lineStart + 5;
      var progress = Math.min(Math.max((currentTime - lineStart) / (lineEnd - lineStart), 0), 1);
      progressBar.style.width = (progress * 100) + '%';
    }
  }

  if (scrollTarget) {
    scrollTarget.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }

  syncedLyrics.forEach(function(l, i) { l.active = (i === activeIndex); });
  window.currentLyrics = syncedLyrics;

  var npOverlay = document.getElementById('nowplaying-overlay');
  if (npOverlay && npOverlay.style.display !== 'none') {
    updateNowPlayingLyricsSync();
  }
}

function closeLyricsPanel() {
  var overlay = document.getElementById('lyrics-overlay');
  if (overlay) { overlay.style.display = 'none'; }
  var btn = document.getElementById('lyrics-btn');
  if (btn) btn.classList.remove('active');
  var editBtn = document.getElementById('lyrics-edit-btn');
  var saveBtn = document.getElementById('lyrics-save-btn');
  if (editBtn) editBtn.classList.remove('active');
  if (saveBtn) saveBtn.style.display = 'none';
  lyricsEditMode = false;
  stopLyricsVisualizer();
  var body = document.getElementById('lyrics-body');
  if (body) body.classList.remove('viz-open');
  var vizBtn = document.getElementById('viz-toggle-btn');
  if (vizBtn) vizBtn.classList.remove('active');
}

function toggleLyricsVisualizer() {
  var canvas = document.getElementById('lyrics-visualizer');
  var btn = document.getElementById('viz-toggle-btn');
  var body = document.getElementById('lyrics-body');
  if (!canvas) return;
  vizActive = !vizActive;
  if (vizActive) {
    body.classList.add('viz-open');
    btn.classList.add('active');
    startLyricsVisualizer();
  } else {
    body.classList.remove('viz-open');
    btn.classList.remove('active');
    stopLyricsVisualizer();
  }
}

function startLyricsVisualizer() {
  stopLyricsVisualizer();
  var canvas = document.getElementById('lyrics-visualizer');
  if (!canvas || !analyserNode) return;
  var body = document.getElementById('lyrics-body');
  canvas.width = 200;
  canvas.height = (body.offsetHeight || 400);
  var ctx = canvas.getContext('2d');
  var bufferLength = analyserNode.frequencyBinCount;
  var dataArray = new Uint8Array(bufferLength);
  var accent = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim();

  function draw() {
    lyricsVizRAF = requestAnimationFrame(draw);
    analyserNode.getByteFrequencyData(dataArray);
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    var barCount = Math.min(state.vizBars || 32, bufferLength);
    var mirror = state.vizMirror;
    var style = state.vizStyle || 'bars';
    var w = canvas.width / (mirror ? Math.ceil(barCount / 2) : barCount);
    var gap = 2;
    var centerX = canvas.width / 2;

    if (style === 'wave') {
      ctx.beginPath();
      var step = Math.max(1, Math.floor(bufferLength / 80));
      for (var i = 0; i < bufferLength; i += step) {
        var x = (i / bufferLength) * canvas.width;
        var y = canvas.height - (dataArray[i] / 255) * canvas.height * 0.8;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.strokeStyle = accent + 'aa';
      ctx.lineWidth = 2;
      ctx.stroke();

      if (mirror) {
        ctx.beginPath();
        for (var i = 0; i < bufferLength; i += step) {
          var x = (i / bufferLength) * canvas.width;
          var y = (dataArray[i] / 255) * canvas.height * 0.8;
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.stroke();
      }
      return;
    }

    if (style === 'blend') {
      var barWidth = canvas.width / barCount;
      for (var i = 0; i < barCount; i++) {
        var barHeight = (dataArray[i] / 255) * canvas.height;
        var hue = 260 + (dataArray[i] / 255) * 60;
        ctx.fillStyle = 'hsla(' + hue + ', 70%, 60%, 0.7)';
        ctx.fillRect(i * barWidth, canvas.height - barHeight, barWidth - gap, barHeight);
      }
      return;
    }

    // bars style
    if (mirror) {
      var halfCount = Math.ceil(barCount / 2);
      for (var i = 0; i < halfCount; i++) {
        var idx = Math.min(i * 2, bufferLength - 1);
        var barHeight = (dataArray[idx] / 255) * canvas.height * 0.9;
        var gradient = ctx.createLinearGradient(0, canvas.height, 0, 0);
        gradient.addColorStop(0, accent + '22');
        gradient.addColorStop(0.5, accent + '88');
        gradient.addColorStop(1, accent);
        ctx.fillStyle = gradient;

        var barW = w - gap;
        var xL = centerX - (i + 1) * w;
        var xR = centerX + i * w + (w - barW) / 2;
        ctx.beginPath();
        ctx.roundRect(xL, canvas.height - barHeight, barW, barHeight, 2);
        ctx.fill();
        ctx.beginPath();
        ctx.roundRect(xR, canvas.height - barHeight, barW, barHeight, 2);
        ctx.fill();
      }
    } else {
      for (var i = 0; i < barCount; i++) {
        var barHeight = (dataArray[i] / 255) * canvas.height * 0.9;
        var gradient = ctx.createLinearGradient(0, canvas.height, 0, 0);
        gradient.addColorStop(0, accent + '22');
        gradient.addColorStop(0.5, accent + '88');
        gradient.addColorStop(1, accent);
        ctx.fillStyle = gradient;

        var barW = w - gap;
        ctx.beginPath();
        ctx.roundRect(i * w + gap / 2, canvas.height - barHeight, barW, barHeight, 2);
        ctx.fill();
      }
    }
  }
  draw();
}

function stopLyricsVisualizer() {
  if (lyricsVizRAF) { cancelAnimationFrame(lyricsVizRAF); lyricsVizRAF = null; }
  var canvas = document.getElementById('lyrics-visualizer');
  if (canvas) {
    var ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, canvas.width, canvas.height);
  }
}
