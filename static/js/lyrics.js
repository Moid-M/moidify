var syncedLyrics = [];
var vizActive = false;
var lyricsVizRAF = null;
var lyricsSyncInterval = null;

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
  syncedLyrics = [];
  try {
    var params = new URLSearchParams({
      artist_name: track.artist || '',
      track_name: track.title || '',
      album_name: track.album || '',
    });
    var controller = new AbortController();
    var timeoutId = setTimeout(function() { controller.abort(); }, 8000);
    var res = await fetch('https://lrclib.net/api/get?' + params.toString(), { signal: controller.signal });
    clearTimeout(timeoutId);
    if (!res.ok) throw new Error('Not found');
    var data = await res.json();
    var text = data.syncedLyrics || data.plainLyrics || 'No lyrics available';
    if (data.syncedLyrics) {
      content.innerHTML = '';
      syncedLyrics = data.syncedLyrics.split('\n').map(function(line) {
        var match = line.match(/\[(\d{2}):(\d{2})\.(\d{2})\]\s*(.*)/);
        if (match) {
          var mins = parseInt(match[1]), secs = parseInt(match[2]), ms = parseInt(match[3]);
          return { time: mins * 60 + secs + ms / 100, text: match[4].trim() };
        }
        return { time: -1, text: line.trim() };
      }).filter(function(l) { return l.text; });
      syncedLyrics.forEach(function(l) {
        var div = document.createElement('div');
        div.className = 'lyrics-line';
        div.dataset.time = l.time;
        div.textContent = l.text;
        content.appendChild(div);
      });
      window.currentLyrics = syncedLyrics;
      if (syncedLyrics.length === 0) {
        content.innerHTML = '<div class="lyrics-placeholder"><p>No lyrics available</p></div>';
      }
    } else {
      content.innerHTML = '<div style="white-space:pre-wrap;line-height:1.8;">' + esc(text) + '</div>';
    }
  } catch(e) {
    content.innerHTML = '<div class="lyrics-placeholder"><svg viewBox="0 0 24 24" width="48" height="48" fill="none" stroke="currentColor" stroke-width="1.5" opacity="0.3"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg><p>No lyrics found</p><p style="font-size:13px;color:var(--text-muted);">Try a different track or check your connection</p></div>';
  }
}

function refreshLyricsForCurrentTrack() {
  var overlay = document.getElementById('lyrics-overlay');
  if (overlay && overlay.style.display !== 'none' && state.queue[state.currentIndex]) {
    fetchLyrics(state.queue[state.currentIndex]);
  }
}

function closeLyricsPanel() {
  var overlay = document.getElementById('lyrics-overlay');
  if (overlay) { overlay.style.display = 'none'; }
  var btn = document.getElementById('lyrics-btn');
  if (btn) btn.classList.remove('active');
  stopLyricsVisualizer();
}

function toggleLyricsVisualizer() {
  var canvas = document.getElementById('lyrics-visualizer');
  var btn = document.getElementById('viz-toggle-btn');
  var panel = document.getElementById('lyrics-panel');
  if (!canvas) return;
  vizActive = !vizActive;
  if (vizActive) {
    canvas.style.display = 'block';
    btn.classList.add('active');
    panel.classList.add('viz-open');
    document.getElementById('lyrics-body').style.flexDirection = 'row';
    startLyricsVisualizer();
  } else {
    canvas.style.display = 'none';
    btn.classList.remove('active');
    panel.classList.remove('viz-open');
    stopLyricsVisualizer();
  }
}

function startLyricsVisualizer() {
  stopLyricsVisualizer();
  var canvas = document.getElementById('lyrics-visualizer');
  if (!canvas || !analyserNode) return;
  var body = document.getElementById('lyrics-body');
  canvas.width = canvas.offsetWidth || 200;
  canvas.height = (body.offsetHeight || 400) - 10;
  var ctx = canvas.getContext('2d');
  var bufferLength = analyserNode.frequencyBinCount;
  var dataArray = new Uint8Array(bufferLength);

  function draw() {
    lyricsVizRAF = requestAnimationFrame(draw);
    analyserNode.getByteFrequencyData(dataArray);
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    var barCount = Math.min(bufferLength, 32);
    var barWidth = canvas.width / barCount;
    var gap = 2;
    var accent = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim();

    for (var i = 0; i < barCount; i++) {
      var barHeight = (dataArray[i] / 255) * canvas.height * 0.9;
      var gradient = ctx.createLinearGradient(0, canvas.height, 0, 0);
      gradient.addColorStop(0, accent + '22');
      gradient.addColorStop(0.5, accent + '88');
      gradient.addColorStop(1, accent);
      ctx.fillStyle = gradient;
      ctx.fillRect(i * barWidth + gap / 2, canvas.height - barHeight, barWidth - gap, barHeight);
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

function updateSyncedLyrics() {
  if (syncedLyrics.length === 0 || !audio.duration) return;
  var currentTime = audio.currentTime;
  var activeIndex = -1;
  for (var i = 0; i < syncedLyrics.length; i++) {
    if (syncedLyrics[i].time <= currentTime) {
      activeIndex = i;
    }
  }
  var lines = qsa('.lyrics-line');
  lines.forEach(function(line, i) {
    if (i === activeIndex) {
      line.classList.add('active');
      line.scrollIntoView({ block: 'center', behavior: 'smooth' });
    } else {
      line.classList.remove('active');
    }
  });
  syncedLyrics.forEach(function(l, i) { l.active = (i === activeIndex); });
  window.currentLyrics = syncedLyrics;
  var npOverlay = document.getElementById('nowplaying-overlay');
  if (npOverlay && npOverlay.style.display !== 'none') {
    updateNowPlayingLyricsSync();
  }
}
