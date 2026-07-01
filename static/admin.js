var TOKEN = localStorage.getItem('moidify_token');
var IS_ADMIN = false;
var allTracks = [];
var filteredTracks = [];
var page = 0;
var perPage = 50;
var contextUserId = null;

function api(path, opts) {
  opts = opts || {};
  var h = {};
  if (TOKEN) h['token'] = TOKEN;
  if (opts.body && !(opts.body instanceof FormData)) { h['Content-Type'] = 'application/json'; opts.body = JSON.stringify(opts.body); }
  return fetch(path, Object.assign({}, opts, {headers: Object.assign(h, opts.headers || {})}));
}

document.querySelectorAll('.nav li').forEach(function(li) {
  li.addEventListener('click', function() {
    document.querySelectorAll('.nav li').forEach(function(l) { l.classList.remove('active'); });
    document.querySelectorAll('.tab').forEach(function(t) { t.classList.remove('active'); });
    this.classList.add('active');
    document.getElementById('tab-'+this.dataset.tab).classList.add('active');
  });
});

async function checkAdmin() {
  if (!TOKEN) { document.body.innerHTML = '<div style="padding:40px;text-align:center;"><h1 style="color:var(--accent);">Access Denied</h1><p style="color:var(--text2);margin-top:8px;">Not logged in. <a href="/" style="color:var(--accent);">Go to player</a></p></div>'; return false; }
  try {
    var r = await api('/api/admin/stats');
    if (r.status === 403) { document.body.innerHTML = '<div style="padding:40px;text-align:center;"><h1 style="color:var(--danger);">Access Denied</h1><p style="color:var(--text2);margin-top:8px;">Only the <strong>admin</strong> account can access this page. <a href="/" style="color:var(--accent);">Go to player</a></p></div>'; return false; }
    if (!r.ok) { document.body.innerHTML = '<div style="padding:40px;text-align:center;"><h1 style="color:var(--danger);">Error</h1><p style="color:var(--text2);margin-top:8px;">'+r.statusText+'</p></div>'; return false; }
    IS_ADMIN = true;
    return true;
  } catch(e) { document.body.innerHTML = '<div style="padding:40px;text-align:center;"><h1 style="color:var(--danger);">Error</h1><p style="color:var(--text2);margin-top:8px;">'+e.message+'</p></div>'; return false; }
}

async function loadStats() {
  try {
    var r = await api('/api/admin/stats');
    if (!r.ok) { document.getElementById('stats').innerHTML = '<p style="color:var(--danger)">Not authenticated. Log in first.</p>'; return; }
    var d = await r.json();
    document.getElementById('stats').innerHTML =
      '<div class="stat"><div class="stat-num">'+d.tracks+'</div><div class="stat-label">Tracks</div></div>'+
      '<div class="stat"><div class="stat-num">'+d.artists+'</div><div class="stat-label">Artists</div></div>'+
      '<div class="stat"><div class="stat-num">'+d.albums+'</div><div class="stat-label">Albums</div></div>'+
      '<div class="stat"><div class="stat-num">'+Math.round(d.total_duration/60)+'m</div><div class="stat-label">Total Playtime</div></div>'+
      '<div class="stat"><div class="stat-num">'+(d.disk_usage_bytes > 1e9 ? (d.disk_usage_bytes/1e9).toFixed(1)+' GB' : (d.disk_usage_bytes/1e6).toFixed(0)+' MB')+'</div><div class="stat-label">Disk Usage</div></div>';
  } catch(e) { document.getElementById('stats').innerHTML = '<p style="color:var(--danger)">Error loading stats</p>'; }
}

function drawBarChart(canvas, labels, values, color, opts) {
  opts = opts || {};
  var ctx = canvas.getContext('2d');
  var dpr = window.devicePixelRatio || 1;
  var w = canvas.clientWidth, h = canvas.clientHeight;
  canvas.width = w * dpr; canvas.height = h * dpr;
  ctx.scale(dpr, dpr);

  var pad = {top:8, bottom:22, left:8, right:8};
  var chartW = w - pad.left - pad.right;
  var chartH = h - pad.top - pad.bottom;
  var max = Math.max(1, Math.max.apply(null, values));
  var barW = Math.min(28, chartW / values.length * 0.7);
  var gap = chartW / values.length;

  ctx.clearRect(0, 0, w, h);

  var isLight = document.body.classList.contains('light-mode') || localStorage.getItem('moidify_light') === 'true';
  ctx.strokeStyle = isLight ? 'rgba(0,0,0,0.08)' : 'rgba(255,255,255,0.06)';
  ctx.lineWidth = 1;
  for (var g = 0; g <= 4; g++) {
    var y = pad.top + chartH * (1 - g/4);
    ctx.beginPath(); ctx.moveTo(pad.left, y); ctx.lineTo(w - pad.right, y); ctx.stroke();
  }

  var accent = color || getComputedStyle(document.documentElement).getPropertyValue('--accent').trim() || '#a0a0a0';
  for (var i = 0; i < values.length; i++) {
    var x = pad.left + i * gap + (gap - barW) / 2;
    var barH = (values[i] / max) * chartH;
    var grad = ctx.createLinearGradient(0, pad.top + chartH - barH, 0, pad.top + chartH);
    grad.addColorStop(0, accent);
    grad.addColorStop(1, accent + '40');
    ctx.fillStyle = grad;
    ctx.beginPath();
    var r = 3;
    var bx = x, by = pad.top + chartH - barH, bw = barW, bh = barH;
    ctx.moveTo(bx + r, by);
    ctx.lineTo(bx + bw - r, by);
    ctx.quadraticCurveTo(bx + bw, by, bx + bw, by + r);
    ctx.lineTo(bx + bw, by + bh);
    ctx.lineTo(bx, by + bh);
    ctx.lineTo(bx, by + r);
    ctx.quadraticCurveTo(bx, by, bx + r, by);
    ctx.fill();
  }

  var isLight = localStorage.getItem('moidify_light') === 'true';
  ctx.fillStyle = isLight ? 'rgba(0,0,0,0.5)' : 'rgba(255,255,255,0.5)';
  ctx.font = '10px sans-serif';
  ctx.textAlign = 'center';
  var skip = Math.max(1, Math.floor(values.length / 12));
  for (var i = 0; i < labels.length; i++) {
    if (labels.length > 12 && i % skip !== 0 && i !== labels.length - 1) continue;
    var label = labels[i].length > 8 ? labels[i].slice(0, 7) + '\u2026' : labels[i];
    ctx.fillText(label, pad.left + i * gap + gap / 2, h - 4);
  }
}

function drawPieChart(canvas, labels, values, colors) {
  var ctx = canvas.getContext('2d');
  var dpr = window.devicePixelRatio || 1;
  var w = canvas.clientWidth, h = canvas.clientHeight;
  canvas.width = w * dpr; canvas.height = h * dpr;
  ctx.scale(dpr, dpr);

  var total = values.reduce(function(a,b){return a+b}, 0);
  if (!total) return;
  var cx = w/2, cy = h/2, r = Math.min(cx, cy) - 12;
  var palette = ['#a0a0a0','#3b82f6','#22c55e','#f59e0b','#ef4444','#ec4899','#14b8a6','#f97316','#8b5cf6','#06b6d4'];
  var start = -Math.PI / 2;
  for (var i = 0; i < values.length; i++) {
    var angle = (values[i] / total) * 2 * Math.PI;
    ctx.fillStyle = colors && colors[i] ? colors[i] : palette[i % palette.length];
    ctx.beginPath();
    ctx.moveTo(cx, cy);
    ctx.arc(cx, cy, r, start, start + angle);
    ctx.closePath();
    ctx.fill();
    start += angle;
  }
  var cardColor = getComputedStyle(document.documentElement).getPropertyValue('--card').trim() || '#1a1a1a';
  ctx.fillStyle = cardColor;
  ctx.beginPath();
  ctx.arc(cx, cy, r * 0.5, 0, 2 * Math.PI);
  ctx.fill();
}

function bytesLabel(b) {
  if (b > 1e9) return (b/1e9).toFixed(1)+' GB';
  if (b > 1e6) return (b/1e6).toFixed(0)+' MB';
  return (b/1e3).toFixed(0)+' KB';
}

async function loadDashboard() {
  try {
    var r = await api('/api/admin/dashboard');
    if (!r.ok) { document.getElementById('chart-grid').innerHTML = '<p style="color:var(--danger)">Failed to load dashboard data</p>'; return; }
    var d = await r.json();
    var grid = document.getElementById('chart-grid');

    var genreHtml = '<div class="chart-card"><h3>Tracks by Genre</h3><canvas id="chart-genre"></canvas></div>';
    var monthHtml = '<div class="chart-card"><h3>Tracks Added per Month</h3><canvas id="chart-monthly"></canvas></div>';
    var playsHtml = '<div class="chart-card"><h3>Plays per Day (last 14 days)</h3><canvas id="chart-plays"></canvas></div>';

    var totalUsed = d.disk_by_format.reduce(function(a,f){ return a + f.bytes }, 0);
    var diskHtml = '<div class="chart-card"><h3>Disk Usage</h3><canvas id="chart-disk"></canvas><div class="stats-line">' +
      '<span style="display:inline-block;margin-right:16px"><span style="color:var(--accent)">' + bytesLabel(totalUsed) + '</span> Used</span>' +
      (d.disk_free ? '<span style="display:inline-block"><span style="color:#666">' + bytesLabel(d.disk_free) + '</span> Free</span>' : '') +
      '</div></div>';

    grid.innerHTML = genreHtml + monthHtml + playsHtml + diskHtml;

    setTimeout(function() {
      var gc = document.getElementById('chart-genre');
      if (gc) {
        var glabels = d.genres.map(function(g){ return g.genre });
        var gvals = d.genres.map(function(g){ return g.count });
        drawBarChart(gc, glabels, gvals);
      }

      var mc = document.getElementById('chart-monthly');
      if (mc) {
        var mlabels = d.monthly_adds.map(function(m){
          var parts = m.month.split('-');
          return parts[0] === String(new Date().getFullYear()) ? parts[1] : m.month;
        });
        var mvals = d.monthly_adds.map(function(m){ return m.count });
        drawBarChart(mc, mlabels, mvals);
      }

      var pc = document.getElementById('chart-plays');
      if (pc) {
        var plabels = d.plays_per_day.map(function(p){ return p.day.slice(5); });
        var pvals = d.plays_per_day.map(function(p){ return p.count });
        drawBarChart(pc, plabels, pvals);
      }

      var dc = document.getElementById('chart-disk');
      if (dc) {
        var dvals = [totalUsed];
        var dcolors = [getComputedStyle(document.documentElement).getPropertyValue('--accent').trim() || '#a0a0a0'];
        var dlabels = ['Used'];
        if (d.disk_free) { dvals.push(d.disk_free); dcolors.push('#555555'); dlabels.push('Free'); }
        drawPieChart(dc, dlabels, dvals, dcolors);
      }
    }, 50);
  } catch(e) { document.getElementById('chart-grid').innerHTML = '<p style="color:var(--danger)">Error loading dashboard</p>'; }
}

async function loadRegistrationStatus() {
  try {
    var r = await api('/api/admin/registration-status');
    if (!r.ok) return;
    var d = await r.json();
    var label = document.getElementById('registration-status-label');
    var btn = document.getElementById('toggle-registration-btn');
    if (d.registration_open) {
      label.textContent = 'Open \u2014 anyone can create an account';
      btn.textContent = 'Close Registration';
    } else {
      label.textContent = 'Closed \u2014 only admins can create users';
      btn.textContent = 'Open Registration';
    }
  } catch(e) {}
}

function toggleRegistration() {
  api('/api/admin/registration-toggle', {method:'POST'}).then(function(r){ return r.json(); }).then(function(d) {
    loadRegistrationStatus();
  });
}

async function loadFailedLogins() {
  try {
    var r = await api('/api/admin/failed-logins');
    if (!r.ok) return;
    var d = await r.json();
    var card = document.getElementById('failed-logins-card');
    if (!d.length) { card.style.display = 'none'; return; }
    card.style.display = '';
    document.getElementById('failed-logins-badge').textContent = d.length;
    var html = '<table style="width:100%;border-collapse:collapse;font-size:13px;">';
    html += '<tr style="color:var(--text3);font-size:11px;text-transform:uppercase;"><th style="text-align:left;padding:4px 8px;">User</th><th style="text-align:center;padding:4px 8px;">Failed Attempts</th><th style="text-align:center;padding:4px 8px;">Status</th><th style="padding:4px 8px;"></th></tr>';
    d.forEach(function(u) {
      var status = u.locked ? '<span style="color:var(--danger);font-weight:600;">LOCKED</span>' : '<span style="color:var(--text3);">Active</span>';
      var unlockBtn = u.locked ? '<button class="admin-btn" onclick="unlockUser('+u.id+')" style="font-size:11px;padding:2px 8px;">Unlock</button>' : '';
      html += '<tr><td style="padding:4px 8px;">'+escHtml(u.username)+'</td><td style="text-align:center;padding:4px 8px;">'+u.failed_attempts+'</td><td style="text-align:center;padding:4px 8px;">'+status+'</td><td style="text-align:right;padding:4px 8px;">'+unlockBtn+'</td></tr>';
    });
    html += '</table>';
    document.getElementById('failed-logins-list').innerHTML = html;
  } catch(e) {}
}

function unlockUser(userId) {
  api('/api/admin/users/'+userId+'/unlock', {method:'POST'}).then(function() {
    loadFailedLogins();
  });
}

async function loadSecurityTab() {
  try {
    var r = await api('/api/admin/registration-status');
    if (!r.ok) return;
    var d = await r.json();
    var label = document.getElementById('sec-registration-label');
    if (label) label.textContent = d.registration_open ? 'Open \u2014 anyone can create an account' : 'Closed \u2014 only admins can create users';
  } catch(e) {}
  try {
    var r2 = await api('/api/admin/failed-logins');
    if (!r2.ok) return;
    var d2 = await r2.json();
    var container = document.getElementById('sec-failed-logins-list');
    if (!d2.length) { container.innerHTML = '<span style="color:var(--text3);">No failed login attempts</span>'; return; }
    var html = '<table style="width:100%;border-collapse:collapse;font-size:13px;">';
    html += '<tr style="color:var(--text3);font-size:11px;text-transform:uppercase;"><th style="text-align:left;padding:6px 8px;">User</th><th style="text-align:center;padding:6px 8px;">Failed Attempts</th><th style="text-align:center;padding:6px 8px;">Status</th><th style="padding:6px 8px;"></th></tr>';
    d2.forEach(function(u) {
      var status = u.locked ? '<span style="color:var(--danger);font-weight:600;">LOCKED</span>' : '<span style="color:var(--text3);">Active</span>';
      var unlockBtn = u.locked ? '<button class="admin-btn" onclick="unlockUser('+u.id+')" style="font-size:11px;padding:2px 8px;">Unlock</button>' : '';
      html += '<tr><td style="padding:6px 8px;">'+escHtml(u.username)+'</td><td style="text-align:center;padding:6px 8px;">'+u.failed_attempts+'</td><td style="text-align:center;padding:6px 8px;">'+status+'</td><td style="text-align:right;padding:6px 8px;">'+unlockBtn+'</td></tr>';
    });
    html += '</table>';
    container.innerHTML = html;
  } catch(e) {}
}

function escHtml(s) {
  if (!s) return '';
  var d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

fetch('/api/setup/status').then(function(r) { return r.json(); }).then(function(d) {
  if (d.setup_needed) { window.location.href = '/setup'; }
}).catch(function(){});

var scanPollTimer = null;

async function rescanMusic(clean) {
  var msg = document.getElementById('rescan-msg');
  var btns = document.querySelectorAll('#rescan-btn, #clean-rescan-btn');
  var progressBar = document.getElementById('scanner-progress');
  btns.forEach(function(b){ b.disabled = true; b.style.opacity = '0.5'; });
  progressBar.style.display = 'block';
  msg.style.display = 'block';
  msg.textContent = clean ? 'Cleaning and rescanning...' : 'Starting scan...';
  try {
    var r = await api('/api/admin/rescan', {method:'POST', body:{clean: !!clean}});
    if (!r.ok) { msg.textContent = 'Scan failed'; return; }
    msg.textContent = 'Scan in progress...';
    if (scanPollTimer) clearInterval(scanPollTimer);
    scanPollTimer = setInterval(async function() {
      try {
        var pr = await api('/api/admin/scanner');
        if (!pr.ok) return;
        var ps = await pr.json();
        var el = document.getElementById('scanner-status');
        el.innerHTML = '<div style="display:flex;gap:20px;flex-wrap:wrap">'+
          '<span>Last scan: <strong>'+esc(ps.last_scan||'Never')+'</strong></span>'+
          '<span>Files found: <strong>'+ps.files_found+'</strong></span>'+
          '<span>Imported: <strong>'+ps.files_imported+'</strong></span>'+
          (ps.errors && ps.errors.length ? '<span style="color:var(--danger)">; '+ps.errors.length+' error(s)</span>' : '')+
          '</div>';
        var bar = document.getElementById('scan-progress-bar');
        var txt = document.getElementById('scan-progress-text');
        if (ps.files_found > 0) {
          bar.max = ps.files_found;
          bar.value = ps.files_imported;
        } else {
          bar.max = 1;
          bar.value = 0;
        }
        txt.textContent = ps.files_imported + ' / ' + ps.files_found + ' files imported';
        if (!ps.running) {
          clearInterval(scanPollTimer);
          scanPollTimer = null;
          btns.forEach(function(b){ b.disabled = false; b.style.opacity = ''; });
          msg.textContent = 'Done. Found ' + ps.files_found + ' files, imported ' + ps.files_imported + '.';
          setTimeout(function(){ msg.style.display = 'none'; progressBar.style.display = 'none'; }, 5000);
          loadStats();
          loadTracks();
        }
      } catch(e) {}
    }, 1000);
  } catch(e) {
    msg.textContent = 'Error: '+e.message;
    btns.forEach(function(b){ b.disabled = false; b.style.opacity = ''; });
  }
}

async function loadSchedule() {
  try {
    var r = await api('/api/admin/schedule-rescan');
    if (!r.ok) return;
    var d = await r.json();
    document.getElementById('schedule-hours').value = d.interval_hours || 0;
    var statusEl = document.getElementById('schedule-status');
    if (d.interval_hours > 0) {
      statusEl.innerHTML = 'Running: every <strong>'+d.interval_hours+'h</strong>' +
        (d.last_scan ? ' (last scan: '+d.last_scan+')' : '');
    } else {
      statusEl.textContent = 'Scheduled rescan is disabled.';
    }
  } catch(e) {}
}

async function setSchedule() {
  var hours = parseFloat(document.getElementById('schedule-hours').value) || 0;
  try {
    var r = await api('/api/admin/schedule-rescan', {method:'POST', body:{interval_hours: hours}});
    if (!r.ok) { var e = await r.json(); alert(e.detail||'Error'); return; }
    loadSchedule();
    showToast('Schedule updated', 'success');
  } catch(e) { alert('Error: '+e.message); }
}

var importPollTimer = null;

async function submitImportUrl() {
  var input = document.getElementById('import-url-input');
  var url = input.value.trim();
  if (!url) { showToast('Enter a URL', 'error'); return; }
  var fmt = document.getElementById('import-format').value;
  var statusEl = document.getElementById('import-status');
  statusEl.style.display = 'block';
  statusEl.innerHTML = 'Submitting...';
  input.disabled = true;
  try {
    var r = await api('/api/admin/import-url', {method:'POST', body:{url: url, format: fmt}});
    if (!r.ok) { var e = await r.json(); statusEl.innerHTML = '<span style="color:var(--danger)">'+(e.detail||'Error')+'</span>'; input.disabled = false; return; }
    var d = await r.json();
    statusEl.innerHTML = 'Queued. Job ID: '+d.job_id;
    var poll = function() {
      importPollTimer = setTimeout(async function() {
        try {
          var pr = await api('/api/admin/import-status/'+d.job_id);
          if (!pr.ok) { statusEl.innerHTML = '<span style="color:var(--danger)">Failed to check status</span>'; input.disabled = false; return; }
          var ps = await pr.json();
          if (ps.status === 'done') {
            statusEl.innerHTML = '<span style="color:var(--accent)">Complete!</span>';
            input.value = '';
            input.disabled = false;
            loadStats();
            loadTracks();
          } else if (ps.status === 'error') {
            statusEl.innerHTML = '<div style="color:var(--danger);white-space:pre-wrap;font-size:13px;">Error: '+(esc(ps.error)||'Unknown')+'</div>';
            input.disabled = false;
          } else if (ps.status === 'importing') {
            statusEl.innerHTML = 'Importing to library...';
            poll();
          } else if (ps.status === 'downloading') {
            statusEl.innerHTML = 'Downloading: '+ps.progress+'%';
            poll();
          } else {
            statusEl.innerHTML = 'Status: '+ps.status;
            poll();
          }
        } catch(e) { statusEl.innerHTML = '<span style="color:var(--danger)">Error: '+e.message+'</span>'; input.disabled = false; }
      }, 2000);
    };
    poll();
  } catch(e) { statusEl.innerHTML = '<span style="color:var(--danger)">Error: '+e.message+'</span>'; input.disabled = false; }
}

async function submitAlbumImport() {
  var urlInput = document.getElementById('album-url-input');
  var artistInput = document.getElementById('album-artist-input');
  var albumInput = document.getElementById('album-name-input');
  var fmt = document.getElementById('album-format').value;
  var statusEl = document.getElementById('album-import-status');
  var url = urlInput.value.trim();
  var artist = artistInput.value.trim();
  var album = albumInput.value.trim();
  if (!artist || !album) { showToast('Enter artist and album name', 'error'); return; }
  statusEl.style.display = 'block';
  statusEl.innerHTML = 'Starting album import...';
  artistInput.disabled = true;
  albumInput.disabled = true;
  urlInput.disabled = true;
  try {
    var r = await api('/api/admin/import-album', {method:'POST', body:{url: url, artist: artist, album: album, format: fmt}});
    if (!r.ok) { var e = await r.json(); statusEl.innerHTML = '<span style="color:var(--danger)">'+(e.detail||'Error')+'</span>'; artistInput.disabled = false; albumInput.disabled = false; urlInput.disabled = false; return; }
    var d = await r.json();
    statusEl.innerHTML = 'Album import queued. Job ID: '+d.job_id;
    var albumPoll = function() {
      setTimeout(async function() {
        try {
          var pr = await api('/api/admin/import-status/'+d.job_id);
          if (!pr.ok) { statusEl.innerHTML = '<span style="color:var(--danger)">Failed to check status</span>'; artistInput.disabled = false; albumInput.disabled = false; urlInput.disabled = false; return; }
          var ps = await pr.json();
          if (ps.status === 'done') {
            statusEl.innerHTML = '<span style="color:var(--accent)">Complete! Downloaded '+(ps.downloaded||0)+' of '+(ps.total||'?')+' tracks.</span>';
            artistInput.value = '';
            albumInput.value = '';
            urlInput.value = '';
            artistInput.disabled = false;
            albumInput.disabled = false;
            urlInput.disabled = false;
            loadStats();
            loadTracks();
          } else if (ps.status === 'error') {
            statusEl.innerHTML = '<div style="color:var(--danger);white-space:pre-wrap;font-size:13px;">Error: '+(esc(ps.error)||'Unknown')+'</div>';
            artistInput.disabled = false;
            albumInput.disabled = false;
            urlInput.disabled = false;
          } else if (ps.status === 'extracting') {
            statusEl.innerHTML = 'Fetching track list...';
            albumPoll();
          } else if (ps.status === 'importing') {
            var pct = ps.current && ps.total ? Math.round(ps.current/ps.total*100) : 0;
            statusEl.innerHTML = 'Track '+(ps.current||'?')+'/'+(ps.total||'?')+': "'+esc(ps.current_title||'')+'" — Importing to library... ('+pct+'%)';
            albumPoll();
          } else if (ps.status === 'downloading') {
            var pct = ps.current && ps.total ? Math.round(ps.current/ps.total*100) : 0;
            statusEl.innerHTML = 'Track '+(ps.current||'?')+'/'+(ps.total||'?')+': "'+esc(ps.current_title||'')+'" — Downloading... ('+pct+'%)';
            albumPoll();
          } else {
            statusEl.innerHTML = 'Status: '+ps.status+(ps.current_title ? ' - '+esc(ps.current_title) : '');
            albumPoll();
          }
        } catch(e) { statusEl.innerHTML = '<span style="color:var(--danger)">Error: '+esc(e.message)+'</span>'; artistInput.disabled = false; albumInput.disabled = false; urlInput.disabled = false; }
      }, 2000);
    };
    albumPoll();
  } catch(e) { statusEl.innerHTML = '<span style="color:var(--danger)">Error: '+esc(e.message)+'</span>'; artistInput.disabled = false; albumInput.disabled = false; urlInput.disabled = false; }
}

function showToast(msg, type) {
  type = type || 'info';
  var el = document.createElement('div');
  el.style.cssText = 'position:fixed;bottom:20px;right:20px;background:var(--accent);color:#000;padding:10px 20px;border-radius:8px;font-size:13px;font-weight:600;z-index:999;animation:fadeIn .2s;';
  el.textContent = msg;
  document.body.appendChild(el);
  setTimeout(function() { el.style.opacity = '0'; el.style.transition = 'opacity .3s'; setTimeout(function(){el.remove()},300); }, 2000);
}

async function loadScannerStatus() {
  try {
    var r = await api('/api/admin/scanner');
    if (!r.ok) return;
    var d = await r.json();
    var el = document.getElementById('scanner-card');
    var content = document.getElementById('scanner-status');
    var progressBar = document.getElementById('scanner-progress');
    if (!d.last_scan && !d.running) { el.style.display = 'none'; return; }
    el.style.display = '';
    var errHtml = d.errors && d.errors.length ? '<span style="color:var(--danger)">; '+d.errors.length+' error(s)</span>' : '';
    content.innerHTML =
      '<div style="display:flex;gap:20px;flex-wrap:wrap">'+
        '<span>Last scan: <strong>'+esc(d.last_scan||'Never')+'</strong></span>'+
        '<span>Files found: <strong>'+d.files_found+'</strong></span>'+
        '<span>Imported: <strong>'+d.files_imported+'</strong></span>'+
        (d.running ? '<span style="color:var(--accent)">\u26a1 Scanning...</span>' : '')+
        errHtml+
      '</div>';
    if (d.running) {
      progressBar.style.display = 'block';
      var bar = document.getElementById('scan-progress-bar');
      var txt = document.getElementById('scan-progress-text');
      if (d.files_found > 0) { bar.max = d.files_found; bar.value = d.files_imported; }
      else { bar.max = 1; bar.value = 0; }
      txt.textContent = d.files_imported + ' / ' + d.files_found + ' files imported';
    }
  } catch(e) {}
}

async function loadPlayStats() {
  try {
    var r = await api('/api/admin/plays');
    if (!r.ok) { document.getElementById('play-summary-stats').innerHTML = '<p style="color:var(--danger)">Failed to load</p>'; return; }
    var d = await r.json();

    var totalMin = Math.round(d.total_listen_time / 60);
    document.getElementById('play-summary-stats').innerHTML =
      '<div class="stat"><div class="stat-num">'+d.total_plays+'</div><div class="stat-label">Total Plays</div></div>'+
      '<div class="stat"><div class="stat-num">'+totalMin+'m</div><div class="stat-label">Listening Time</div></div>'+
      '<div class="stat"><div class="stat-num">'+(d.total_plays && d.total_plays > 0 ? (d.total_listen_time/d.total_plays).toFixed(0) : 0)+'s</div><div class="stat-label">Avg Per Play</div></div>';

    var tracksHtml = '';
    for (var i = 0; i < d.top_tracks.length; i++) {
      var t = d.top_tracks[i];
      if (!t.play_count) continue;
      tracksHtml += '<li><span>' + esc(t.title || 'Unknown') + ' \u2014 ' + esc(t.artist || 'Unknown') + '</span><span class="play-count">' + t.play_count + ' plays</span></li>';
    }
    document.getElementById('top-tracks').innerHTML = tracksHtml || '<li style="color:var(--text3)">No plays yet</li>';

    var artistsHtml = '';
    for (var i = 0; i < d.top_artists.length; i++) {
      var a = d.top_artists[i];
      if (!a.play_count) continue;
      artistsHtml += '<li><span>' + esc(a.artist || 'Unknown') + '</span><span class="play-count">' + a.play_count + ' plays</span></li>';
    }
    document.getElementById('top-artists').innerHTML = artistsHtml || '<li style="color:var(--text3)">No plays yet</li>';

    var albumsHtml = '';
    for (var i = 0; i < d.top_albums.length; i++) {
      var al = d.top_albums[i];
      if (!al.play_count) continue;
      albumsHtml += '<li><span>' + esc(al.album || 'Unknown') + ' \u2014 ' + esc(al.artist || 'Unknown') + '</span><span class="play-count">' + al.play_count + ' plays</span></li>';
    }
    document.getElementById('top-albums').innerHTML = albumsHtml || '<li style="color:var(--text3)">No plays yet</li>';
  } catch(e) { document.getElementById('play-summary-stats').innerHTML = '<p style="color:var(--danger)">Error loading</p>'; }
}

async function loadTrends() {
  try {
    var r = await api('/api/admin/listening-trends');
    if (!r.ok) { document.getElementById('trends-grid').style.display = 'none'; return; }
    var d = await r.json();
    document.getElementById('trends-grid').style.display = '';

    setTimeout(function() {
      var hc = document.getElementById('chart-hour');
      if (hc && d.by_hour.length) {
        var labels = [];
        var vals = [];
        for (var i = 0; i < 24; i++) {
          var found = d.by_hour.find(function(h){ return h.hour === i });
          labels.push(i + ':00');
          vals.push(found ? found.count : 0);
        }
        drawBarChart(hc, labels, vals);
      }

      var dc = document.getElementById('chart-weekday');
      if (dc && d.by_day.length) {
        var dayOrder = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];
        var wlabels = [];
        var wvals = [];
        for (var i = 0; i < dayOrder.length; i++) {
          var found = d.by_day.find(function(dd){ return dd.day === dayOrder[i] });
          wlabels.push(dayOrder[i]);
          wvals.push(found ? found.count : 0);
        }
        drawBarChart(dc, wlabels, wvals);
      }
    }, 50);
  } catch(e) { document.getElementById('trends-grid').style.display = 'none'; }
}

async function loadTracks() {
  try {
    var r = await api('/api/tracks');
    if (!r.ok) return;
    allTracks = await r.json();
    filterTracks();
  } catch(e) {}
}

function filterTracks() {
  var q = (document.getElementById('track-filter').value || '').toLowerCase();
  filteredTracks = allTracks.filter(function(t) {
    return (t.title && t.title.toLowerCase().indexOf(q) !== -1) ||
           (t.artist && t.artist.toLowerCase().indexOf(q) !== -1) ||
           (t.album && t.album.toLowerCase().indexOf(q) !== -1);
  });
  page = 0;
  renderTracks();
}

var selectedTrackIds = {};

function renderTracks() {
  var start = page * perPage;
  var end = Math.min(start + perPage, filteredTracks.length);
  var html = '';
  for (var i = start; i < end; i++) {
    var t = filteredTracks[i];
    var dur = t.duration ? Math.floor(t.duration/60)+':'+String(Math.floor(t.duration%60)).padStart(2,'0') : '?';
    var checked = selectedTrackIds[t.id] ? 'checked' : '';
    var hasLyrics = t.lyrics ? 'lyrics-has' : 'lyrics-none';
    html += '<tr data-track-id="'+t.id+'"><td><input type="checkbox" class="track-check" data-id="'+t.id+'" '+checked+'></td>'+
      '<td>'+t.id+'</td><td>'+esc(t.title||'')+'</td><td>'+esc(t.artist||'')+'</td><td>'+esc(t.album||'')+'</td><td>'+dur+'</td>'+
      '<td><button class="lrc-btn '+hasLyrics+'" data-id="'+t.id+'" title="Left-click: upload LRC, Right-click: remove lyrics">♪</button>'+
      '<button class="fetch-lyrics-btn" data-id="'+t.id+'" title="Fetch from LRCLib">↻</button></td>'+
      '<td><button class="del-btn" data-id="'+t.id+'" data-title="'+esc(t.title||'')+'">Delete</button></td></tr>';
  }
  document.getElementById('track-body').innerHTML = html;
  document.getElementById('track-count').textContent = filteredTracks.length + ' tracks';

  document.querySelectorAll('.track-check').forEach(function(cb) {
    cb.addEventListener('change', function() {
      var id = parseInt(this.dataset.id);
      if (this.checked) selectedTrackIds[id] = true;
      else delete selectedTrackIds[id];
      updateSelectionBar();
    });
  });

  document.querySelectorAll('.del-btn').forEach(function(b) {
    b.addEventListener('click', function() {
      if (!confirm('Delete "'+this.dataset.title+'"?')) return;
      deleteTrack(parseInt(this.dataset.id));
    });
  });

  document.querySelectorAll('.lrc-btn').forEach(function(b) {
    b.addEventListener('click', function(e) {
      e.preventDefault();
      lrcUploadInput.dataset.trackId = this.dataset.id;
      lrcUploadInput.click();
    });
    b.addEventListener('contextmenu', function(e) {
      e.preventDefault();
      if (this.classList.contains('lyrics-has')) {
        deleteTrackLrc(this.dataset.id);
      }
    });
  });

  document.querySelectorAll('.fetch-lyrics-btn').forEach(function(b) {
    b.addEventListener('click', function() {
      fetchTrackLyrics(this.dataset.id);
    });
  });

  var totalPages = Math.ceil(filteredTracks.length / perPage);
  document.getElementById('pagination').innerHTML =
    '<button onclick="page=Math.max(0,page-1);renderTracks()"'+(page===0?' disabled':'')+'>Prev</button>'+
    '<span>'+(page+1)+' / '+totalPages+'</span>'+
    '<button onclick="page=Math.min('+(totalPages-1)+',page+1);renderTracks()"'+(page>=totalPages-1?' disabled':'')+'>Next</button>';
}

document.getElementById('select-all-tracks').addEventListener('change', function() {
  var checked = this.checked;
  for (var i = 0; i < filteredTracks.length; i++) {
    var id = filteredTracks[i].id;
    if (checked) selectedTrackIds[id] = true;
    else delete selectedTrackIds[id];
  }
  renderTracks();
  updateSelectionBar();
});

function updateSelectionBar() {
  var bar = document.getElementById('track-selection-bar');
  var count = Object.keys(selectedTrackIds).length;
  if (count > 0) {
    bar.style.display = 'flex';
    document.getElementById('track-sel-count').textContent = count;
  } else {
    bar.style.display = 'none';
  }
}

document.getElementById('clear-track-sel').addEventListener('click', function() {
  selectedTrackIds = {};
  document.getElementById('select-all-tracks').checked = false;
  renderTracks();
  updateSelectionBar();
});

document.getElementById('delete-selected-btn').addEventListener('click', function() {
  var ids = Object.keys(selectedTrackIds).map(Number);
  if (!ids.length) return;
  if (!confirm('Delete '+ids.length+' selected track(s)? This cannot be undone.')) return;
  deleteSelectedTracks(ids);
});

async function deleteSelectedTracks(ids) {
  for (var i = 0; i < ids.length; i++) {
    try {
      await api('/api/admin/tracks/'+ids[i], {method:'DELETE'});
    } catch(e) {}
  }
  selectedTrackIds = {};
  document.getElementById('select-all-tracks').checked = false;
  loadTracks();
  loadStats();
  updateSelectionBar();
}

var lrcUploadInput = document.createElement('input');
lrcUploadInput.type = 'file';
lrcUploadInput.accept = '.lrc';
lrcUploadInput.style.display = 'none';
document.body.appendChild(lrcUploadInput);

lrcUploadInput.addEventListener('change', async function() {
  var file = this.files && this.files[0];
  if (!file) return;
  var id = this.dataset.trackId;
  var form = new FormData();
  form.append('file', file);
  try {
    var r = await api('/api/admin/tracks/'+id+'/lyrics-upload', {method:'POST', body:form});
    if (!r.ok) { var e = await r.json(); alert(e.detail||'Upload failed'); return; }
    reloadTrack(id);
  } catch(e) { alert('Error: '+e.message); }
  this.value = '';
});

async function reloadTrack(id) {
  try {
    var r = await api('/api/tracks/'+id);
    if (!r.ok) return;
    var updated = await r.json();
    for (var i = 0; i < allTracks.length; i++) {
      if (allTracks[i].id === id) { allTracks[i] = updated; break; }
    }
    filterTracks();
  } catch(e) {}
}

async function scanAllLyrics(force) {
  var resultEl = document.getElementById('lyrics-scan-result');
  resultEl.textContent = 'Scanning...';
  resultEl.style.color = 'var(--text2)';
  try {
    var r = await api('/api/admin/lyrics/scan-all'+(force?'?force=true':''), {method:'POST'});
    if (!r.ok) { var e = await r.json(); resultEl.textContent = 'Error: '+(e.detail||'Request failed'); resultEl.style.color = 'var(--danger)'; return; }
    var d = await r.json();
    resultEl.textContent = 'Scanned '+d.scanned+' track(s), found '+d.found+' lyrics'+(d.errors?' ('+d.errors+' errors)':'');
    resultEl.style.color = d.found > 0 ? 'var(--accent)' : 'var(--text2)';
    loadTracks();
  } catch(e) { resultEl.textContent = 'Error: '+e.message; resultEl.style.color = 'var(--danger)'; }
}

async function fetchTrackLyrics(id) {
  try {
    var r = await api('/api/admin/tracks/'+id+'/lyrics/fetch', {method:'POST'});
    if (!r.ok) { var e = await r.json(); alert(e.detail||'Error'); return; }
    var d = await r.json();
    if (d.lyrics) {
      reloadTrack(parseInt(id));
    } else {
      alert('No lyrics found for this track on LRCLib');
    }
  } catch(e) { alert('Error: '+e.message); }
}

async function deleteTrackLrc(id) {
  if (!confirm('Remove lyrics for this track?')) return;
  try {
    var r = await api('/api/admin/tracks/'+id+'/lyrics', {method:'DELETE'});
    if (!r.ok) { var e = await r.json(); alert(e.detail||'Error'); return; }
    reloadTrack(id);
  } catch(e) { alert('Error: '+e.message); }
}

async function deleteTrack(id) {
  try {
    var r = await api('/api/admin/tracks/'+id, {method:'DELETE'});
    if (!r.ok) { var e = await r.json(); alert(e.detail||'Error'); return; }
    allTracks = allTracks.filter(function(t) { return t.id !== id; });
    filterTracks();
    loadStats();
  } catch(e) { alert('Error: '+e.message); }
}

async function loadUsers() {
  try {
    var r = await api('/api/admin/users');
    if (!r.ok) { document.getElementById('user-list').innerHTML = '<p style="color:var(--danger)">Failed to load users</p>'; return; }
    var users = await r.json();
    var html = '<table class="user-table" id="user-table"><thead><tr><th>ID</th><th>Username</th><th>Email</th><th>Role</th><th>Created</th></tr></thead><tbody>';
    for (var i = 0; i < users.length; i++) {
      var u = users[i];
      var isRoot = u.username === 'admin';
      var roleHtml = u.is_admin ? '<span class="admin-badge">Admin</span>' : '<span style="color:var(--text3);font-size:12px">User</span>';
      html += '<tr data-user-id="'+u.id+'" data-username="'+esc(u.username)+'" data-admin="'+u.is_admin+'" data-root="'+isRoot+'">'+
        '<td>'+u.id+'</td>'+
        '<td>'+esc(u.username)+(isRoot?' <span style="color:var(--accent);font-size:11px">\u2605</span>':'')+'</td>'+
        '<td>'+esc(u.email||'\u2014')+'</td>'+
        '<td>'+roleHtml+'</td>'+
        '<td style="color:var(--text3);font-size:12px">'+u.created_at+'</td></tr>';
    }
    html += '</tbody></table>';
    document.getElementById('user-list').innerHTML = html;

    document.querySelectorAll('#user-table tbody tr').forEach(function(row) {
      row.addEventListener('contextmenu', function(e) {
        e.preventDefault();
        contextUserId = parseInt(this.dataset.userId);
        var username = this.dataset.username;
        var isAdmin = this.dataset.admin === '1';
        var isRoot = this.dataset.root === 'true';
        showUserContextMenu(e.clientX, e.clientY, username, isAdmin, isRoot);
      });
    });
  } catch(e) { document.getElementById('user-list').innerHTML = '<p style="color:var(--danger)">Error loading users</p>'; }
}

var contextMenuEl = document.getElementById('context-menu');

function showUserContextMenu(x, y, username, isAdmin, isRoot) {
  var items = contextMenuEl.querySelectorAll('.context-menu-item');
  items.forEach(function(item) {
    var action = item.dataset.action;
    if (action === 'toggle-admin') {
      item.textContent = isRoot ? 'Cannot modify root' : (isAdmin ? 'Revoke Admin' : 'Grant Admin');
      item.style.display = isRoot ? 'none' : '';
      item.style.color = isAdmin ? 'var(--danger)' : 'var(--text2)';
    } else if (action === 'change-pw') {
      item.textContent = 'Change Password \u2014 ' + username;
      item.style.display = '';
    } else if (action === 'delete-user') {
      item.textContent = 'Delete ' + username;
      item.style.display = isRoot ? 'none' : '';
    }
  });

  contextMenuEl.style.left = x + 'px';
  contextMenuEl.style.top = y + 'px';
  contextMenuEl.classList.add('visible');
}

contextMenuEl.addEventListener('click', function(e) {
  var item = e.target.closest('.context-menu-item');
  if (!item) return;
  var action = item.dataset.action;
  var row = document.querySelector('#user-table tbody tr[data-user-id="'+contextUserId+'"]');
  contextMenuEl.classList.remove('visible');

  if (action === 'toggle-admin') {
    if (!row) return;
    var makeAdmin = row.dataset.admin !== '1';
    toggleAdmin(contextUserId, makeAdmin);
  } else if (action === 'change-pw') {
    document.getElementById('pw-user-label').textContent = row ? row.dataset.username : '';
    openModal('password-modal');
  } else if (action === 'delete-user') {
    if (confirm('Delete user "' + (row ? row.dataset.username : '') + '"? This will also remove their playlists and favorites.')) {
      deleteUser(contextUserId);
    }
  }
});

document.addEventListener('click', function() {
  contextMenuEl.classList.remove('visible');
});

async function toggleAdmin(userId, makeAdmin) {
  try {
    var r = await api('/api/admin/users/'+userId+'/admin', { method:'POST', body: { is_admin: makeAdmin } });
    if (!r.ok) { var e = await r.json(); alert(e.detail||'Error'); return; }
    loadUsers();
  } catch(e) { alert('Error: '+e.message); }
}

async function changePassword() {
  var pw = document.getElementById('pw-password').value;
  if (!pw || pw.length < 8) { alert('Password must be at least 8 characters'); return; }
  if (!/[a-z]/.test(pw)) { alert('Password must contain a lowercase letter'); return; }
  if (!/[A-Z]/.test(pw)) { alert('Password must contain an uppercase letter'); return; }
  if (!/[0-9]/.test(pw)) { alert('Password must contain a digit'); return; }
  if (!/[!@#$%^&*(),.?":{}|<>_\-]/.test(pw)) { alert('Password must contain a special character'); return; }
  try {
    var r = await api('/api/admin/users/'+contextUserId+'/password', { method:'POST', body: { password: pw } });
    if (!r.ok) { var e = await r.json(); alert(e.detail||'Error'); return; }
    document.getElementById('pw-password').value = '';
    closeModal('password-modal');
    alert('Password changed');
  } catch(e) { alert('Error: '+e.message); }
}

async function deleteUser(userId) {
  try {
    var r = await api('/api/admin/users/'+userId, { method:'DELETE' });
    if (!r.ok) { var e = await r.json(); alert(e.detail||'Error'); return; }
    loadUsers();
  } catch(e) { alert('Error: '+e.message); }
}

document.getElementById('create-user-btn').addEventListener('click', function() {
  openModal('create-modal');
});

async function createUser() {
  var username = document.getElementById('create-username').value.trim();
  var password = document.getElementById('create-password').value;
  var isAdmin = document.getElementById('create-admin').checked;
  if (!username) { alert('Username is required'); return; }
  if (!password || password.length < 8) { alert('Password must be at least 8 characters'); return; }
  if (!/[a-z]/.test(password)) { alert('Password must contain a lowercase letter'); return; }
  if (!/[A-Z]/.test(password)) { alert('Password must contain an uppercase letter'); return; }
  if (!/[0-9]/.test(password)) { alert('Password must contain a digit'); return; }
  if (!/[!@#$%^&*(),.?":{}|<>_\-]/.test(password)) { alert('Password must contain a special character'); return; }
  try {
    var r = await api('/api/admin/users', { method:'POST', body: { username: username, password: password, is_admin: isAdmin } });
    if (!r.ok) { var e = await r.json(); alert(e.detail||'Error'); return; }
    document.getElementById('create-username').value = '';
    document.getElementById('create-password').value = '';
    document.getElementById('create-admin').checked = false;
    closeModal('create-modal');
    loadUsers();
  } catch(e) { alert('Error: '+e.message); }
}

function openModal(id) { document.getElementById(id).classList.add('visible'); }
function closeModal(id) { document.getElementById(id).classList.remove('visible'); }

function esc(s) {
  if (!s) return '';
  var d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

var dropZone = document.getElementById('drop-zone');
var fileInput = document.getElementById('file-input');

dropZone.addEventListener('click', function() { fileInput.click(); });
dropZone.addEventListener('dragover', function(e) { e.preventDefault(); e.stopPropagation(); this.classList.add('dragover'); });
dropZone.addEventListener('dragleave', function(e) { e.preventDefault(); this.classList.remove('dragover'); });
dropZone.addEventListener('drop', function(e) {
  e.preventDefault(); this.classList.remove('dragover');
  handleFiles(e.dataTransfer.files);
});
fileInput.addEventListener('change', function() { handleFiles(this.files); });

async function handleFiles(files) {
  if (!files.length) return;
  var formData = new FormData();
  for (var i = 0; i < files.length; i++) {
    if (files[i].size === 0) continue;
    formData.append('files', files[i]);
  }
  if (!formData.has('files')) return;
  var progress = document.getElementById('upload-progress');
  var bar = document.getElementById('upload-bar');
  var msg = document.getElementById('upload-msg');
  progress.classList.add('visible');
  bar.value = 0;
  msg.textContent = 'Uploading '+files.length+' file(s)...';
  try {
    var r = await api('/api/admin/upload', { method:'POST', body: formData });
    bar.value = 1;
    if (!r.ok) { var d; try { d = await r.json(); } catch(e) {} msg.textContent = 'Error: ' + ((d && d.detail) || r.statusText); return; }
    var d = await r.json();
    msg.textContent = 'Imported: ' + (d.imported ? d.imported.join(', ') : 'none');
  } catch(e) {
    msg.textContent = 'Error: '+e.message;
  }
  loadTracks();
  loadStats();
}

function makeEditable(cell, trackId, field) {
  var original = cell.textContent;
  var input = document.createElement('input');
  input.type = 'text';
  input.value = original;
  input.style.cssText = 'width:100%;padding:2px 4px;border-radius:4px;border:1px solid var(--accent);background:var(--bg);color:var(--text);font-size:13px;outline:none;';
  cell.textContent = '';
  cell.appendChild(input);
  input.focus();
  input.select();

  function save() {
    var val = input.value.trim();
    if (val && val !== original) {
      api('/api/admin/tracks/' + trackId + '/metadata', {
        method: 'PATCH',
        body: (function(){ var o = {}; o[field] = val; return o; })()
      });
    }
    cell.textContent = val || original;
  }

  input.addEventListener('blur', save);
  input.addEventListener('keydown', function(e) {
    if (e.key === 'Enter') { input.blur(); }
    if (e.key === 'Escape') { cell.textContent = original; }
  });
}

var libView = 'albums';

function switchLibView(view) {
  libView = view;
  document.getElementById('lib-albums-btn').style.opacity = view === 'albums' ? '1' : '0.5';
  document.getElementById('lib-artists-btn').style.opacity = view === 'artists' ? '1' : '0.5';
  loadLib();
}

document.getElementById('lib-filter').addEventListener('input', function() {
  loadLib();
});

async function loadLib() {
  var filter = (document.getElementById('lib-filter').value || '').toLowerCase();
  var container = document.getElementById('lib-content');
  if (libView === 'albums') {
    try {
      var r = await api('/api/admin/albums');
      if (!r.ok) { container.innerHTML = 'Failed to load'; return; }
      var albums = await r.json();
      if (filter) {
        albums = albums.filter(function(a) {
          return (a.album && a.album.toLowerCase().indexOf(filter) !== -1) ||
                 (a.artist && a.artist.toLowerCase().indexOf(filter) !== -1);
        });
      }
      var html = '<table class="track-table"><thead><tr><th>Album</th><th>Artist</th><th>Tracks</th><th>Duration</th><th>Cover</th></tr></thead><tbody>';
      for (var i = 0; i < albums.length; i++) {
        var a = albums[i];
        var dur = Math.round(a.total_duration / 60) + 'm';
        html += '<tr style="cursor:pointer" onclick="showAlbumTracks(\'' + encodeURIComponent(a.album || '') + '\')">' +
          '<td>' + esc(a.album) + '</td>' +
          '<td>' + esc(a.artist) + '</td>' +
          '<td>' + a.track_count + '</td>' +
          '<td>' + dur + '</td>' +
          '<td>' + (a.has_cover ? '\u2713' : '\u2014') + '</td></tr>';
      }
      html += '</tbody></table>' +
        '<div style="margin-top:8px;font-size:12px;color:var(--text3);">' + albums.length + ' album(s)</div>';
      container.innerHTML = html;
    } catch(e) { container.innerHTML = 'Error loading albums'; }
  } else {
    try {
      var r = await api('/api/admin/artists');
      if (!r.ok) { container.innerHTML = 'Failed to load'; return; }
      var artists = await r.json();
      if (filter) {
        artists = artists.filter(function(a) {
          return a.artist && a.artist.toLowerCase().indexOf(filter) !== -1;
        });
      }
      var html = '<table class="track-table"><thead><tr><th>Artist</th><th>Tracks</th><th>Albums</th><th>Duration</th></tr></thead><tbody>';
      for (var i = 0; i < artists.length; i++) {
        var a = artists[i];
        var dur = Math.round(a.total_duration / 60) + 'm';
        html += '<tr>' +
          '<td><strong>' + esc(a.artist) + '</strong></td>' +
          '<td>' + a.track_count + '</td>' +
          '<td>' + a.album_count + '</td>' +
          '<td>' + dur + '</td></tr>';
      }
      html += '</tbody></table>' +
        '<div style="margin-top:8px;font-size:12px;color:var(--text3);">' + artists.length + ' artist(s)</div>';
      container.innerHTML = html;
    } catch(e) { container.innerHTML = 'Error loading artists'; }
  }
}

async function showAlbumTracks(encodedName) {
  var albumName = decodeURIComponent(encodedName);
  try {
    var r = await api('/api/admin/albums/' + encodeURIComponent(albumName));
    if (!r.ok) return;
    var tracks = await r.json();
    var container = document.getElementById('lib-content');
    var html = '<button class="admin-btn" onclick="loadLib()" style="margin-bottom:12px">&larr; Back to albums</button>';
    html += '<h3 style="font-size:16px;color:var(--text);margin-bottom:8px">' + esc(albumName) + '</h3>';
    html += '<table class="track-table"><thead><tr><th>#</th><th>Title</th><th>Artist</th><th>Duration</th></tr></thead><tbody>';
    for (var i = 0; i < tracks.length; i++) {
      var t = tracks[i];
      var dur = t.duration ? Math.floor(t.duration/60)+':'+String(Math.floor(t.duration%60)).padStart(2,'0') : '?';
      html += '<tr>' +
        '<td>' + (t.track_number || '\u2014') + '</td>' +
        '<td>' + esc(t.title || '') + '</td>' +
        '<td>' + esc(t.artist || '') + '</td>' +
        '<td>' + dur + '</td></tr>';
    }
    html += '</tbody></table>';
    container.innerHTML = html;
  } catch(e) {}
}

function updateScannerErrorLog(d) {
  var el = document.getElementById('scanner-errors');
  var list = document.getElementById('scanner-error-list');
  var summary = document.getElementById('scanner-error-summary');
  if (d.errors && d.errors.length) {
    el.style.display = '';
    var errors = d.errors.slice(-50);
    summary.textContent = errors.length + ' error(s) \u2014 click to expand';
    list.textContent = errors.join('\n');
  } else {
    el.style.display = 'none';
  }
}

var origLoadScannerStatus = loadScannerStatus;
loadScannerStatus = async function() {
  try {
    var r = await api('/api/admin/scanner');
    if (!r.ok) return;
    var d = await r.json();
    var el = document.getElementById('scanner-card');
    var content = document.getElementById('scanner-status');
    var progressBar = document.getElementById('scanner-progress');
    if (!d.last_scan && !d.running) { el.style.display = 'none'; return; }
    el.style.display = '';
    var errHtml = d.errors && d.errors.length ? '<span style="color:var(--danger)">; ' + d.errors.length + ' error(s)</span>' : '';
    content.innerHTML =
      '<div style="display:flex;gap:20px;flex-wrap:wrap">' +
        '<span>Last scan: <strong>' + esc(d.last_scan||'Never') + '</strong></span>' +
        '<span>Files found: <strong>' + d.files_found + '</strong></span>' +
        '<span>Imported: <strong>' + d.files_imported + '</strong></span>' +
        (d.running ? '<span style="color:var(--accent)"> Scanning...</span>' : '') +
        errHtml +
      '</div>';
    updateScannerErrorLog(d);
    if (d.running) {
      progressBar.style.display = 'block';
      var bar = document.getElementById('scan-progress-bar');
      var txt = document.getElementById('scan-progress-text');
      if (d.files_found > 0) { bar.max = d.files_found; bar.value = d.files_imported; }
      else { bar.max = 1; bar.value = 0; }
      txt.textContent = d.files_imported + ' / ' + d.files_found + ' files imported';
    }
  } catch(e) {}
};

var origRenderTracks = renderTracks;
renderTracks = function() {
  origRenderTracks();
  document.querySelectorAll('#track-body tr').forEach(function(tr) {
    var tid = parseInt(tr.dataset.trackId);
    var cells = tr.querySelectorAll('td');
    if (cells.length >= 5) {
      cells[2].style.cursor = 'pointer';
      cells[2].title = 'Click to edit';
      cells[2].addEventListener('dblclick', function(e) {
        makeEditable(this, tid, 'title');
      });
      cells[3].style.cursor = 'pointer';
      cells[3].title = 'Click to edit';
      cells[3].addEventListener('dblclick', function(e) {
        makeEditable(this, tid, 'artist');
      });
      cells[4].style.cursor = 'pointer';
      cells[4].title = 'Click to edit';
      cells[4].addEventListener('dblclick', function(e) {
        makeEditable(this, tid, 'album');
      });
    }
  });
};

async function loadDbStats() {
  try {
    var r = await api('/api/admin/db/stats');
    if (!r.ok) return;
    var d = await r.json();
    document.getElementById('db-stats').innerHTML =
      '<div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">' +
      '<span>DB Size: <strong>' + d.db_size_display + '</strong></span>' +
      '<span>Tracks: <strong>' + d.track_count + '</strong></span>' +
      '<span>Users: <strong>' + d.user_count + '</strong></span>' +
      '<span>Playlists: <strong>' + d.playlist_count + '</strong></span>' +
      '<span>Play History: <strong>' + d.play_history_count + '</strong></span>' +
      '</div>';
  } catch(e) {}
}

async function dbVacuum() {
  var msg = document.getElementById('db-msg');
  msg.style.display = 'block';
  msg.textContent = 'Vacuuming database...';
  try {
    var r = await api('/api/admin/db/vacuum', {method:'POST'});
    var d = await r.json();
    msg.textContent = d.message || 'Done';
    loadDbStats();
  } catch(e) {
    msg.textContent = 'Error: ' + e.message;
  }
  setTimeout(function() { msg.style.display = 'none'; }, 3000);
}

async function dbIntegrity() {
  var msg = document.getElementById('db-msg');
  msg.style.display = 'block';
  msg.textContent = 'Checking integrity...';
  try {
    var r = await api('/api/admin/db/integrity');
    var d = await r.json();
    if (d.ok) {
      msg.style.color = '#22c55e';
      msg.textContent = '\u2713 Integrity check passed';
    } else {
      msg.style.color = 'var(--danger)';
      msg.textContent = '\u2717 Integrity check failed: ' + d.result;
    }
  } catch(e) {
    msg.textContent = 'Error: ' + e.message;
  }
  setTimeout(function() { msg.style.display = 'none'; }, 5000);
}

async function reExtractCovers() {
  var msg = document.getElementById('cover-extract-msg');
  msg.style.display = 'block';
  msg.textContent = 'Processing...';
  try {
    var r = await api('/api/admin/re-extract-covers', {method:'POST'});
    var d = await r.json();
    msg.textContent = 'Done. Processed: ' + d.processed + ', Errors: ' + d.errors + ', Total checked: ' + d.total_checked;
    loadStats();
  } catch(e) {
    msg.textContent = 'Error: ' + e.message;
  }
  setTimeout(function() { msg.style.display = 'none'; }, 5000);
}

async function loadServerInfo() {
  try {
    var r = await api('/api/admin/server-info');
    if (!r.ok) return;
    var d = await r.json();
    document.getElementById('server-info').innerHTML =
      '<div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">' +
      '<span>Python: <strong>' + esc(d.python_version) + '</strong></span>' +
      '<span>Platform: <strong>' + esc(d.platform) + '</strong></span>' +
      (d.mutagen_version ? '<span>Mutagen: <strong>' + esc(d.mutagen_version) + '</strong></span>' : '') +
      (d.yt_dlp_version ? '<span>yt-dlp: <strong>' + esc(d.yt_dlp_version) + '</strong></span>' : '') +
      (d.process_uptime ? '<span>Uptime: <strong>' + esc(d.process_uptime) + '</strong></span>' : '') +
      '<span>Server time: <strong>' + esc(d.server_time) + '</strong></span>' +
      '</div>';
  } catch(e) {
    document.getElementById('server-info').innerHTML = 'Error loading server info';
  }
}

document.getElementById('track-filter').addEventListener('input', filterTracks);

checkAdmin().then(function(ok) {
  if (ok) {
    document.getElementById('admin-content').style.display = '';
    loadStats(); loadDashboard(); loadRegistrationStatus(); loadFailedLogins();
    loadSecurityTab(); loadScannerStatus(); loadPlayStats(); loadTrends();
    loadUsers(); loadTracks(); loadSchedule();
    loadLib(); loadDbStats(); loadServerInfo();
    document.getElementById('lib-albums-btn').style.opacity = '1';
    document.getElementById('lib-artists-btn').style.opacity = '0.5';
  }
});
