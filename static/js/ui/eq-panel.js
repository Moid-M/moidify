var visualizerRAF = null;

function openEQPanel() {
  var overlay = document.getElementById('eq-overlay');
  if (overlay.style.display !== 'none') return;
  overlay.style.display = 'flex';
  qs('#eq-btn').classList.add('active');
  renderEQPanel();
  startVisualizer();
}

function closeEQPanel() {
  document.getElementById('eq-overlay').style.display = 'none';
  qs('#eq-btn').classList.remove('active');
  stopVisualizer();
}

function renderEQPanel() {
  var freqs = [32,64,125,250,500,'1K','2K','4K','8K','16K'];
  var eqValues = state.eq || EQ_PRESETS[state.eqPreset] || EQ_PRESETS['Normal'];
  var bands = '';
  eqValues.forEach(function(v,i) {
    bands += '<div class="eq-band"><input type="range" min="-12" max="12" step="1" value="'+v+'" data-band="'+i+'" style="height:100px;"><span class="eq-label">'+freqs[i]+'</span></div>';
  });
  document.getElementById('eq-band-grid').innerHTML = bands;

  var presetHtml = '';
  Object.keys(EQ_PRESETS).forEach(function(name) {
    presetHtml += '<button class="eq-preset-btn'+(state.eqPreset===name?' active':'')+'" data-preset="'+name+'">'+name+'</button>';
  });
  document.getElementById('eq-preset-strip').innerHTML = presetHtml;

  var cf = document.getElementById('eq-crossfade');
  cf.value = state.crossfade;
  document.getElementById('eq-crossfade-label').textContent = state.crossfade + 's';

  qsa('#eq-preset-strip .eq-preset-btn').forEach(function(el) {
    el.addEventListener('click', function() {
      qsa('#eq-preset-strip .eq-preset-btn').forEach(function(s){s.classList.remove('active');});
      this.classList.add('active');
      var name = this.dataset.preset;
      setEQPreset(name);
      var values = EQ_PRESETS[name];
      qsa('#eq-band-grid .eq-band input').forEach(function(input, i) { input.value = values[i]; });
    });
  });

  qsa('#eq-band-grid .eq-band input').forEach(function(el) {
    el.addEventListener('input', function() {
      var i = parseInt(this.dataset.band);
      setEQBand(i, parseFloat(this.value));
      qsa('#eq-preset-strip .eq-preset-btn').forEach(function(s){s.classList.remove('active');});
    });
  });

  cf.addEventListener('input', function() {
    state.crossfade = parseFloat(this.value);
    localStorage.setItem('moidify_crossfade', this.value);
    document.getElementById('eq-crossfade-label').textContent = this.value + 's';
  });
}

function startVisualizer() {
  stopVisualizer();
  var canvas = document.getElementById('eq-visualizer');
  if (!canvas || !analyserNode) return;
  var ctx = canvas.getContext('2d');
  var bufferLength = analyserNode.frequencyBinCount;
  var dataArray = new Uint8Array(bufferLength);

  function draw() {
    visualizerRAF = requestAnimationFrame(draw);
    analyserNode.getByteFrequencyData(dataArray);
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    var barCount = Math.min(state.vizBars || 64, bufferLength);
    var mirror = state.vizMirror;
    var style = state.vizStyle || 'bars';
    var accent = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim();

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

    if (mirror) {
      var halfCount = Math.ceil(barCount / 2);
      var w = canvas.width / halfCount;
      var gap = 1;
      var centerX = canvas.width / 2;
      for (var i = 0; i < halfCount; i++) {
        var idx = Math.min(i * 2, bufferLength - 1);
        var barHeight = (dataArray[idx] / 255) * canvas.height;
        var gradient = ctx.createLinearGradient(0, canvas.height, 0, 0);
        gradient.addColorStop(0, accent + '33');
        gradient.addColorStop(1, accent);
        ctx.fillStyle = gradient;
        var barW = w - gap;
        ctx.beginPath();
        ctx.roundRect(centerX - (i + 1) * w, canvas.height - barHeight, barW, barHeight, 2);
        ctx.fill();
        ctx.beginPath();
        ctx.roundRect(centerX + i * w + (w - barW) / 2, canvas.height - barHeight, barW, barHeight, 2);
        ctx.fill();
      }
    } else {
      var barWidth = canvas.width / barCount;
      var gap = 1;
      for (var i = 0; i < barCount; i++) {
        var barHeight = (dataArray[i] / 255) * canvas.height;
        var gradient = ctx.createLinearGradient(0, canvas.height, 0, 0);
        gradient.addColorStop(0, accent + '33');
        gradient.addColorStop(1, accent);
        ctx.fillStyle = gradient;
        ctx.beginPath();
        ctx.roundRect(i * barWidth + gap / 2, canvas.height - barHeight, barWidth - gap, barHeight, 2);
        ctx.fill();
      }
    }
  }
  draw();
}

function stopVisualizer() {
  if (visualizerRAF) { cancelAnimationFrame(visualizerRAF); visualizerRAF = null; }
  var canvas = document.getElementById('eq-visualizer');
  if (canvas) {
    var ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, canvas.width, canvas.height);
  }
}
