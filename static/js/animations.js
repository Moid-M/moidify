function updateBackdrop(trackId) {
  var img = new Image();
  img.crossOrigin = 'anonymous';
  img.src = '/api/cover/'+trackId;
  img.onload = function() {
    try {
      var canvas = document.createElement('canvas');
      canvas.width = 1; canvas.height = 1;
      var ctx = canvas.getContext('2d');
      ctx.drawImage(img,0,0,1,1);
      var p = ctx.getImageData(0,0,1,1).data;
      var color = 'rgba('+p[0]+','+p[1]+','+p[2]+',0.15)';
      var bd = document.getElementById('backdrop');
      bd.style.background = 'radial-gradient(ellipse at 50% 0%, '+color+' 0%, transparent 70%)';
      bd.classList.add('visible');
    } catch(e) { console.error(e); }
  };
}

function applyAnimations() {
  var cover = document.getElementById('queue-cover');
  var wrap = document.getElementById('queue-cover-wrap');
  var showVinyl = state.animations.vinylSpin !== false && state.currentIndex >= 0;
  cover.classList.toggle('vinyl-spin', showVinyl);
  if (wrap) wrap.classList.toggle('cd-hole', state.animations.cdHole !== false && showVinyl);

  var playBtn = document.getElementById('play-btn');
  var isPlaying = !audio.paused && audio.src;
  playBtn.classList.toggle('playing', state.animations.glowPulse !== false && isPlaying);

  var eqAnim = document.querySelector('.player-eq');
  if (eqAnim) {
    eqAnim.style.display = state.animations.eqAnim !== false && isPlaying ? 'flex' : 'none';
  }

  applyVinylSpeed();
}

function applyVinylSpeed() {
  var speed = state.vinylSpinSpeed || 4;
  document.documentElement.style.setProperty('--vinyl-speed', speed + 's');
}

function clearAnimations() {
  document.getElementById('queue-cover').classList.remove('vinyl-spin');
  var w = document.getElementById('queue-cover-wrap');
  if (w) w.classList.remove('cd-hole');
  document.getElementById('play-btn').classList.remove('playing');
}

function applyAnimationSettings() {
  var app = document.getElementById('app');
  var speed = state.animSpeed === 'off' || (!state.animations.cards && !state.animations.rows) ? 'anim-disabled' : '';
  app.classList.toggle('anim-disabled', !!speed);

  var durs = {slow:'0.3s', normal:'', fast:'0.08s', off:'0s'};
  var dur = durs[state.animSpeed] !== undefined ? durs[state.animSpeed] : '';
  if (dur !== undefined) {
    document.documentElement.style.setProperty('--anim-dur', dur || '');
    document.documentElement.style.setProperty('--transition-dur', dur || '');
  }

  app.classList.toggle('anim-cover-zoom', state.animations.coverZoom !== false);
  app.classList.toggle('anim-seek-shimmer', state.animations.seekShimmer !== false);

  applyVinylSpeed();
}

function applyTrackCovers() {
  var show = state.showTrackCovers;
  qsa('.track-row .track-cover').forEach(function(img) {
    img.style.display = show ? '' : 'none';
  });
  if (show) {
    qsa('.track-row').forEach(function(row) {
      var img = row.querySelector('.track-cover');
      if (!img) {
        var num = row.querySelector('.track-num');
        if (num && row.dataset.trackId) {
          img = document.createElement('img');
          img.className = 'track-cover';
          img.src = '/api/cover/' + row.dataset.trackId;
          img.alt = '';
          img.loading = 'lazy';
          img.onerror = function() { this.style.display = 'none'; };
          num.after(img);
        }
      }
    });
  }
}

function updateVolumeFill() {
  var vol = parseFloat(document.getElementById('volume').value) || 1;
  document.documentElement.style.setProperty('--vol-pct', (vol * 100) + '%');
}

function updateSeekFill() {
  if (audio.duration) {
    var pct = (audio.currentTime / audio.duration) * 100;
    var pctStr = pct + '%';
    document.documentElement.style.setProperty('--seek-pct', pctStr);
  }
}
