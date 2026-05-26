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
  overlay.onclick = function(e) { if (e.target === overlay) closeModal(); };
}

function closeModal() { document.getElementById('modal-overlay').style.display = 'none'; }

function showToast(msg, type) {
  type = type || 'info';
  var c = document.getElementById('toast-container');
  var t = document.createElement('div');
  t.className = 'toast toast-' + type;
  t.textContent = msg;
  c.appendChild(t);
  setTimeout(function() {
    t.classList.add('toast-out');
    setTimeout(function() { if (t.parentNode) t.parentNode.removeChild(t); }, 300);
  }, 3000);
}

function showSettings() {
  var html =
    '<div class="settings-layout">'+
      '<div class="settings-sidebar">'+
        '<div class="settings-sidebar-item active" data-tab="theme" data-i18n="settings.theme">Theme</div>'+
        '<div class="settings-sidebar-item" data-tab="animations" data-i18n="settings.animations">Animations</div>'+
        '<div class="settings-sidebar-item" data-tab="playback" data-i18n="settings.playback">Playback</div>'+
        '<div class="settings-sidebar-item" data-tab="about" data-i18n="settings.about">About</div>'+
      '</div>'+
      '<div class="settings-main">'+
        '<div class="settings-tab-content active" id="tab-theme"></div>'+
        '<div class="settings-tab-content" id="tab-animations"></div>'+
        '<div class="settings-tab-content" id="tab-playback"></div>'+
        '<div class="settings-tab-content" id="tab-about"></div>'+
      '</div>'+
    '</div>';
  showModal(html);
  translateDOM(document.getElementById('modal'));
  renderThemeTab();
  renderAnimationsTab();
  renderPlaybackTab();
  renderAboutTab();

  qsa('.settings-sidebar-item').forEach(function(el) {
    el.addEventListener('click', function() {
      qsa('.settings-sidebar-item').forEach(function(t){t.classList.remove('active');});
      qsa('.settings-tab-content').forEach(function(t){t.classList.remove('active');});
      this.classList.add('active');
      document.getElementById('tab-'+this.dataset.tab).classList.add('active');
    });
  });
}

function renderThemeTab() {
  var container = document.getElementById('tab-theme');
  var colors = ['#a855f7','#ef4444','#f97316','#eab308','#1db954','#06b6d4','#3b82f6','#ec4899','#14b8a6'];
  var swatches = '';
  colors.forEach(function(c) {
    swatches += '<div class="color-swatch'+(c===state.accentColor?' active':'')+'" data-color="'+c+'" style="background:'+c+';"></div>';
  });

  var lightChecked = state.lightMode ? ' checked' : '';
  var autoChecked = state.autoTheme ? ' checked' : '';
  var currentLang = getLanguage();
  var langOpts = Object.keys(TRANSLATIONS).map(function(code) {
    var label = code === 'en' ? 'English' : (code === 'de' ? 'Deutsch' : code);
    return '<option value="'+code+'"'+(currentLang===code?' selected':'')+'>'+label+'</option>';
  }).join('');

  container.innerHTML =
    '<div class="settings-section" data-i18n-section="themeColor"><h3 data-i18n="settings.themeColor">Theme Color</h3><div class="color-grid">'+swatches+'</div>'+
    '<div class="color-custom-wrap"><input type="color" id="custom-color" value="'+state.accentColor+'"><span style="color:var(--text-muted);font-size:13px;">Custom</span></div></div>'+
    '<div class="settings-section"><h3 data-i18n="settings.appearance">Appearance</h3>'+
    '<label class="toggle-row" data-i18n-row="light"><span data-i18n="settings.lightMode">Light Mode</span><input type="checkbox" id="light-toggle"'+lightChecked+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row" data-i18n-row="autoTheme"><span data-i18n="settings.autoTheme">Follow system theme</span><input type="checkbox" id="auto-theme-toggle"'+autoChecked+'><span class="toggle-slider"></span></label></div>'+
    '<div class="settings-section"><h3 data-i18n="settings.language">Language</h3><select class="lang-select" id="lang-select">'+langOpts+'</select></div>';

  qsa('.color-swatch', container).forEach(function(el) {
    el.addEventListener('click', function() {
      qsa('.color-swatch', container).forEach(function(s){s.classList.remove('active');});
      this.classList.add('active');
      applyAccent(this.dataset.color);
      var cc = document.getElementById('custom-color');
      if (cc) cc.value = this.dataset.color;
    });
  });

  var cc = document.getElementById('custom-color');
  if (cc) {
    cc.addEventListener('input', function() {
      qsa('.color-swatch', container).forEach(function(s){s.classList.remove('active');});
      applyAccent(this.value);
    });
  }

  var lt = document.getElementById('light-toggle');
  if (lt) {
    lt.addEventListener('change', function() {
      state.lightMode = this.checked;
      applyTheme();
    });
  }

  var at = document.getElementById('auto-theme-toggle');
  if (at) {
    at.addEventListener('change', function() {
      state.autoTheme = this.checked;
      if (state.autoTheme) {
        document.getElementById('light-toggle').disabled = true;
      } else {
        document.getElementById('light-toggle').disabled = false;
      }
      applyTheme();
    });
    if (state.autoTheme) document.getElementById('light-toggle').disabled = true;
  }

  var ls = document.getElementById('lang-select');
  if (ls) {
    ls.addEventListener('change', function() {
      setLanguage(this.value);
    });
  }
}

function renderAnimationsTab() {
  var container = document.getElementById('tab-animations');
  var animEnabled = state.animations;
  var allOn = Object.values(animEnabled).every(function(v) { return v === true; });
  var speeds = ['slow','normal','fast','off'];
  var speedLabels = {slow:'Slow',normal:'Normal',fast:'Fast',off:'Off'};

  container.innerHTML =
    '<div class="settings-section">'+
      '<div class="anim-toggle-row" style="border-bottom:1px solid var(--accent-dim);padding-bottom:12px;margin-bottom:12px;">'+
        '<span style="font-weight:600;color:var(--text-primary);">All animations</span>'+
        '<button class="toggle-switch'+(allOn?' on':'')+'" id="anim-toggle-all"></button>'+
      '</div>'+
    '</div>'+
    '<div class="settings-section"><h3>Elements</h3>'+
    '<div class="anim-toggle-row"><span>Card hover lift</span><button class="toggle-switch'+(animEnabled.cards?' on':'')+'" data-anim="cards"></button></div>'+
    '<div class="anim-toggle-row"><span>Cover zoom on hover</span><button class="toggle-switch'+(animEnabled.coverZoom?' on':'')+'" data-anim="coverZoom"></button></div>'+
    '<div class="anim-toggle-row"><span>Track row actions fade</span><button class="toggle-switch'+(animEnabled.rows?' on':'')+'" data-anim="rows"></button></div>'+
    '<div class="anim-toggle-row"><span>View transitions</span><button class="toggle-switch'+(animEnabled.transitions?' on':'')+'" data-anim="transitions"></button></div>'+
    '</div>'+
    '<div class="settings-section"><h3>Player</h3>'+
    '<div class="anim-toggle-row"><span>Vinyl spin on album art</span><button class="toggle-switch'+(animEnabled.vinylSpin?' on':'')+'" data-anim="vinylSpin"></button></div>'+
    '<div id="vinyl-extras"'+(animEnabled.vinylSpin?'':' style="display:none"')+'>'+
    '<div class="anim-toggle-row"><span>CD hole in center</span><button class="toggle-switch'+(animEnabled.cdHole?' on':'')+'" data-anim="cdHole"></button></div>'+
    '<div class="anim-toggle-row" style="flex-wrap:wrap;"><span>Vinyl spin speed</span><input type="range" id="vinyl-speed-slider" min="2" max="10" step="1" value="'+state.vinylSpinSpeed+'" style="width:100px;height:4px;"><span style="font-size:12px;color:var(--text-muted);min-width:20px;">'+state.vinylSpinSpeed+'s</span></div>'+
    '</div>'+
    '<div class="anim-toggle-row"><span>Glow pulse on play button</span><button class="toggle-switch'+(animEnabled.glowPulse?' on':'')+'" data-anim="glowPulse"></button></div>'+
    '<div class="anim-toggle-row"><span>Equalizer visualizer bars</span><button class="toggle-switch'+(animEnabled.eqAnim?' on':'')+'" data-anim="eqAnim"></button></div>'+
    '<div class="anim-toggle-row"><span>Seek bar shimmer effect</span><button class="toggle-switch'+(animEnabled.seekShimmer?' on':'')+'" data-anim="seekShimmer"></button></div>'+
    '<div class="anim-toggle-row"><span>Queue slide animation</span><button class="toggle-switch'+(animEnabled.queueSlide?' on':'')+'" data-anim="queueSlide"></button></div>'+
    '</div>'+
    '<div class="settings-section"><h3>Speed</h3><div class="anim-speed-wrap">'+
    speeds.map(function(s){return '<button class="anim-speed-btn'+(state.animSpeed===s?' active':'')+'" data-speed="'+s+'">'+speedLabels[s]+'</button>';}).join('')+
    '</div></div>'+
    '<div class="settings-section"><h3>Performance</h3>'+
    '<div class="anim-toggle-row"><span>Smooth seek (60fps)</span><button class="toggle-switch'+(state.smoothSeek?' on':'')+'" id="toggle-smooth-seek"></button></div>'+
    '<div class="anim-toggle-row"><span>Track cover thumbnails</span><button class="toggle-switch'+(state.showTrackCovers?' on':'')+'" id="toggle-track-covers"></button></div>'+
    '</div>';

  qsa('.toggle-switch', container).forEach(function(el) {
    el.addEventListener('click', function() {
      var key = this.dataset.anim;
      if (key) {
        state.animations[key] = !state.animations[key];
        this.classList.toggle('on');
        localStorage.setItem('moidify_animations', JSON.stringify(state.animations));
        applyAnimationSettings();
        applyAnimations();
        if (key === 'vinylSpin') {
          var extras = document.getElementById('vinyl-extras');
          if (extras) extras.style.display = state.animations.vinylSpin ? '' : 'none';
        }
      }
    });
  });

  document.getElementById('anim-toggle-all').addEventListener('click', function() {
    var val = !Object.values(state.animations).every(function(v) { return v === true; });
    Object.keys(state.animations).forEach(function(k) { state.animations[k] = val; });
    localStorage.setItem('moidify_animations', JSON.stringify(state.animations));
    qsa('.toggle-switch', container).forEach(function(t) { t.classList.toggle('on', val); });
    this.classList.toggle('on', val);
    applyAnimationSettings();
    applyAnimations();
    var extras = document.getElementById('vinyl-extras');
    if (extras) extras.style.display = state.animations.vinylSpin ? '' : 'none';
  });

  qsa('.anim-speed-btn', container).forEach(function(el) {
    el.addEventListener('click', function() {
      qsa('.anim-speed-btn', container).forEach(function(s){s.classList.remove('active');});
      this.classList.add('active');
      state.animSpeed = this.dataset.speed;
      localStorage.setItem('moidify_anim_speed', state.animSpeed);
      applyAnimationSettings();
    });
  });

  var smoothToggle = document.getElementById('toggle-smooth-seek');
  if (smoothToggle) {
    smoothToggle.addEventListener('click', function() {
      state.smoothSeek = !state.smoothSeek;
      this.classList.toggle('on');
      localStorage.setItem('moidify_smooth_seek', state.smoothSeek);
    });
  }

  var coversToggle = document.getElementById('toggle-track-covers');
  if (coversToggle) {
    coversToggle.addEventListener('click', function() {
      state.showTrackCovers = !state.showTrackCovers;
      this.classList.toggle('on');
      localStorage.setItem('moidify_show_track_covers', state.showTrackCovers);
      applyTrackCovers();
    });
  }

  var vinylSpeed = document.getElementById('vinyl-speed-slider');
  if (vinylSpeed) {
    vinylSpeed.addEventListener('input', function() {
      state.vinylSpinSpeed = parseInt(this.value);
      localStorage.setItem('moidify_vinyl_speed', this.value);
      var label = this.parentElement.querySelector('span:last-child');
      if (label) label.textContent = this.value + 's';
      applyVinylSpeed();
    });
  }
}

function renderPlaybackTab() {
  var container = document.getElementById('tab-playback');
  var freqs = [32,64,125,250,500,'1K','2K','4K','8K','16K'];
  var eqValues = state.eq || EQ_PRESETS[state.eqPreset] || EQ_PRESETS['Normal'];
  var bands = '';
  eqValues.forEach(function(v,i) {
    bands += '<div class="eq-band"><input type="range" min="-12" max="12" step="1" value="'+v+'" data-band="'+i+'"><span class="eq-label">'+freqs[i]+'</span></div>';
  });

  var presetHtml = '';
  Object.keys(EQ_PRESETS).forEach(function(name) {
    presetHtml += '<button class="eq-preset-btn'+(state.eqPreset===name?' active':'')+'" data-preset="'+name+'">'+name+'</button>';
  });

  var qualityLabels = {high:'Original', medium:'Medium (256k)', low:'Low (128k)', voice:'Voice (64k Opus)'};
  var qualityBtns = Object.keys(qualityLabels).map(function(k) {
    return '<button class="eq-preset-btn'+(state.streamQuality===k?' active':'')+'" data-quality="'+k+'">'+qualityLabels[k]+'</button>';
  }).join('');

  container.innerHTML =
    '<div class="settings-section"><h3>Equalizer</h3><div class="eq-presets">'+presetHtml+'</div><div class="eq-grid">'+bands+'</div></div>'+
    '<div class="settings-section"><h3>Stream Quality</h3>'+
    '<div class="eq-presets">'+qualityBtns+'</div>'+
    '<p style="font-size:12px;color:var(--text-muted);margin-top:4px">Requires ffmpeg on the server for transcoding. Otherwise plays original.</p></div>'+
    '<div class="settings-section"><h3>Crossfade</h3>'+
    '<div class="crossfade-wrap"><span style="font-size:12px;color:var(--text-muted);min-width:30px;">'+state.crossfade+'s</span>'+
    '<input type="range" id="crossfade-slider" min="0" max="12" step="1" value="'+state.crossfade+'"><span style="font-size:12px;color:var(--text-muted);">12s</span></div></div>'+
    '<div class="settings-section"><h3>Sleep Timer</h3>'+
    '<p style="color:var(--text-secondary);font-size:13px;margin-bottom:8px;">'+(state.sleepTimer?'Timer is active ('+SLEEP_OPTIONS.find(function(o){return o.value===state.sleepTimer.value})?.label+')':'No timer active')+'</p>'+
    '<button onclick="showSleepTimerPicker()" class="btn-secondary" style="padding:6px 16px;font-size:13px;border-radius:20px;border:1px solid var(--text-muted);background:transparent;color:var(--text-secondary);cursor:pointer;">'+(state.sleepTimer?'Change Timer':'Set Timer')+'</button>'+
    (state.sleepTimer?' <button onclick="cancelSleepTimer()" class="btn-secondary" style="padding:6px 16px;font-size:13px;border-radius:20px;border:1px solid var(--danger);color:var(--danger);background:transparent;cursor:pointer;">Cancel</button>':'')+
    '</div>';

  qsa('.eq-preset-btn', container).forEach(function(el) {
    el.addEventListener('click', function() {
      qsa('.eq-preset-btn', container).forEach(function(s){s.classList.remove('active');});
      this.classList.add('active');
      var name = this.dataset.preset;
      setEQPreset(name);
      var values = EQ_PRESETS[name];
      qsa('.eq-band input', container).forEach(function(input, i) {
        input.value = values[i];
      });
    });
  });

  qsa('.eq-band input', container).forEach(function(el) {
    el.addEventListener('input', function() {
      var i = parseInt(this.dataset.band);
      setEQBand(i, parseFloat(this.value));
      qsa('.eq-preset-btn', container).forEach(function(s){s.classList.remove('active');});
    });
  });

  qsa('[data-quality]', container).forEach(function(el) {
    el.addEventListener('click', function() {
      qsa('[data-quality]', container).forEach(function(s){s.classList.remove('active');});
      this.classList.add('active');
      state.streamQuality = this.dataset.quality;
      localStorage.setItem('moidify_stream_quality', state.streamQuality);
    });
  });

  var cf = document.getElementById('crossfade-slider');
  if (cf) {
    cf.addEventListener('input', function() {
      state.crossfade = parseFloat(this.value);
      localStorage.setItem('moidify_crossfade', this.value);
      var label = this.parentElement.querySelector('span:first-child');
      if (label) label.textContent = this.value + 's';
    });
  }
}

function renderAboutTab() {
  var container = document.getElementById('tab-about');
  var verEl = document.createElement('div');
  verEl.className = 'about-row';
  verEl.innerHTML = '<span>Version</span><span class="about-val" id="app-version">...</span>';
  fetch('/api/version').then(function(r){return r.json();}).then(function(d){
    document.getElementById('app-version').textContent = d.version || '?';
  }).catch(function(){document.getElementById('app-version').textContent='?';});
  container.innerHTML =
    '<div class="about-section"><h3>Moidify</h3>'+
    verEl.outerHTML +
    '<div class="about-row"><span>Database</span><span class="about-val">SQLite</span></div>'+
    '<div class="about-row"><span>Music folder</span><span class="about-val" style="font-size:11px;">music/</span></div>'+
    '<div class="about-row"><span>Admin</span><span class="about-val"><a href="/admin" style="color:var(--accent);text-decoration:none;font-size:13px;">Open &rarr;</a></span></div>'+
    '</div>'+
    '<div class="about-section"><h3>Keyboard Shortcuts</h3>'+
    '<div class="about-row"><span>Play/Pause</span><span class="about-val"><kbd>Space</kbd></span></div>'+
    '<div class="about-row"><span>Seek -10s / +10s</span><span class="about-val"><kbd>&larr;</kbd> <kbd>&rarr;</kbd></span></div>'+
    '<div class="about-row"><span>Volume</span><span class="about-val"><kbd>&uarr;</kbd> <kbd>&darr;</kbd></span></div>'+
    '<div class="about-row"><span>Next / Previous</span><span class="about-val"><kbd>N</kbd> <kbd>P</kbd></span></div>'+
    '<div class="about-row"><span>Like</span><span class="about-val"><kbd>L</kbd></span></div>'+
    '<div class="about-row"><span>Cycle Repeat</span><span class="about-val"><kbd>R</kbd></span></div>'+
    '<div class="about-row"><span>Search</span><span class="about-val"><kbd>S</kbd></span></div>'+
    '<div class="about-row"><span>Close modals</span><span class="about-val"><kbd>Esc</kbd></span></div>'+
    '</div>';
}

function showLoginModal() {
  showModal('<h2>Log In</h2><input type="text" id="login-username" placeholder="Username" autofocus><input type="password" id="login-password" placeholder="Password"><div class="modal-actions"><button onclick="doLogin()" class="btn-primary">Log In</button><button onclick="closeModal()" class="btn-secondary">Cancel</button></div>');
  setTimeout(function(){var el=document.getElementById('login-username');if(el)el.focus();},100);
}

function showRegisterModal() {
  showModal('<h2>Register</h2><input type="text" id="reg-username" placeholder="Username" autofocus><input type="email" id="reg-email" placeholder="Email (optional)"><input type="password" id="reg-password" placeholder="Password"><div class="modal-actions"><button onclick="doRegister()" class="btn-primary">Register</button><button onclick="closeModal()" class="btn-secondary">Cancel</button></div>');
  setTimeout(function(){var el=document.getElementById('reg-username');if(el)el.focus();},100);
}

function showSleepTimerPicker() {
  var html = '<h2>Sleep Timer</h2>';
  SLEEP_OPTIONS.forEach(function(opt) {
    var active = state.sleepTimer && state.sleepTimer.value === opt.value ? ' active' : '';
    html += '<div class="context-menu-item'+active+'" data-sleep="'+opt.value+'" style="padding:10px 14px;border-radius:6px;cursor:pointer;">'+opt.label+'</div>';
  });
  html += '<div class="modal-actions"><button onclick="closeModal()" class="btn-secondary">Cancel</button></div>';
  showModal(html);
  setTimeout(function() {
    qsa('[data-sleep]',document.getElementById('modal')).forEach(function(el) {
      el.addEventListener('click', function() {
        setSleepTimer(parseInt(this.dataset.sleep));
      });
    });
  },0);
}

function setSleepTimer(value) {
  closeModal();
  if (value === 0) { cancelSleepTimer(); return; }

  state.sleepTimer = {
    value: value,
    startTime: Date.now(),
    duration: value > 0 ? value : null,
  };
  localStorage.setItem('moidify_sleep_timer', JSON.stringify(state.sleepTimer));
  renderSleepTimer();

  if (sleepTimerInterval) clearInterval(sleepTimerInterval);
  sleepTimerInterval = setInterval(function() {
    if (value === -1) {
      // handled by audio.onended
    } else if (value === -2) {
      // handled in nextTrack/ended
    } else {
      var elapsed = (Date.now() - state.sleepTimer.startTime) / 1000;
      var remaining = value - elapsed;
      renderSleepTimer();
      if (remaining <= 0) {
        clearInterval(sleepTimerInterval);
        audio.pause();
        qs('#play-btn').innerHTML = iconPlay();
        cancelSleepTimer();
      }
    }
  }, 1000);
}

function checkSleepTimer(reason) {
  if (!state.sleepTimer) return;
  if (reason === 'ended') {
    if (state.sleepTimer.value === -1) {
      audio.pause();
      qs('#play-btn').innerHTML = iconPlay();
      cancelSleepTimer();
    } else if (state.sleepTimer.value === -2) {
      if (state.currentIndex < 0 || state.currentIndex >= state.queue.length - 1) {
        audio.pause();
        qs('#play-btn').innerHTML = iconPlay();
        cancelSleepTimer();
      }
    }
  }
}

function cancelSleepTimer() {
  state.sleepTimer = null;
  localStorage.removeItem('moidify_sleep_timer');
  if (sleepTimerInterval) { clearInterval(sleepTimerInterval); sleepTimerInterval = null; }
  renderSleepTimer();
}

function renderSleepTimer() {
  var section = document.getElementById('sleep-timer-section');
  var display = document.getElementById('sleep-timer-display');
  var label = document.getElementById('sleep-timer-label');
  var progress = document.getElementById('sleep-timer-progress');
  if (!section) return;

  if (!state.sleepTimer || state.sleepTimer.value === 0) {
    section.style.display = 'none';
    return;
  }
  section.style.display = 'block';

  var val = state.sleepTimer.value;
  if (val === -1) {
    display.textContent = 'End of Track';
    label.textContent = 'Stops when track ends';
    progress.value = 50;
    progress.style.display = 'none';
  } else if (val === -2) {
    display.textContent = 'End of Queue';
    label.textContent = 'Stops when queue ends';
    progress.value = 50;
    progress.style.display = 'none';
  } else {
    var elapsed = (Date.now() - state.sleepTimer.startTime) / 1000;
    var remaining = Math.max(0, val - elapsed);
    display.textContent = formatTimeLong(remaining);
    var pct = ((val - remaining) / val) * 100;
    progress.value = pct;
    progress.style.display = '';
    label.textContent = 'Stops in ' + formatTimeLong(remaining);
  }
}

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
        });
      } else if (action === 'album-shuffle') {
        apiJson('/api/albums/tracks?album='+encodeURIComponent(album.album)+(album.artist?'&artist='+encodeURIComponent(album.artist):'')).then(function(tracks) {
          var shuffled = tracks.slice();
          for (var i = shuffled.length - 1; i > 0; i--) {
            var j = Math.floor(Math.random() * (i + 1));
            var tmp = shuffled[i]; shuffled[i] = shuffled[j]; shuffled[j] = tmp;
          }
          playFromQueue(shuffled, 0);
        });
      } else if (action === 'album-add-queue') {
        apiJson('/api/albums/tracks?album='+encodeURIComponent(album.album)+(album.artist?'&artist='+encodeURIComponent(album.artist):'')).then(function(tracks) {
          tracks.forEach(function(t) { addTrackToQueueEnd(t); });
        });
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
                // Refresh current view to update star displays
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
        });
      } else if (action === 'artist-shuffle') {
        apiJson('/api/artists/tracks?artist='+encodeURIComponent(artist.artist)).then(function(tracks) {
          var shuffled = tracks.slice();
          for (var i = shuffled.length - 1; i > 0; i--) {
            var j = Math.floor(Math.random() * (i + 1));
            var tmp = shuffled[i]; shuffled[i] = shuffled[j]; shuffled[j] = tmp;
          }
          playFromQueue(shuffled, 0);
        });
      } else if (action === 'artist-add-queue') {
        apiJson('/api/artists/tracks?artist='+encodeURIComponent(artist.artist)).then(function(tracks) {
          tracks.forEach(function(t) { addTrackToQueueEnd(t); });
        });
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
}

function hideContextMenuOnce(e) {
  if (!e.target.closest('#context-menu')) {
    hideContextMenu();
    document.removeEventListener('click', hideContextMenuOnce);
  }
}

// ---- EQ Panel ----

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

    var barCount = Math.min(bufferLength, 64);
    var barWidth = canvas.width / barCount;
    var gradient = ctx.createLinearGradient(0, canvas.height, 0, 0);
    var accent = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim();
    gradient.addColorStop(0, accent + '33');
    gradient.addColorStop(1, accent);

    for (var i = 0; i < barCount; i++) {
      var barHeight = (dataArray[i] / 255) * canvas.height;
      ctx.fillStyle = gradient;
      ctx.fillRect(i * barWidth, canvas.height - barHeight, barWidth - 1, barHeight);
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

function setupSearch() {
  var input = document.getElementById('search');

  input.addEventListener('input', function() {
    clearTimeout(searchTimeout);
    var val = this.value.trim();
    if (val) {
      searchTimeout = setTimeout(function() { navigate('search', val); }, 300);
    } else if (state.currentView === 'search') {
      navigate(state._prevView || 'albums', state._prevData || null);
    }
  });

  input.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') {
      this.value = '';
      this.blur();
      navigate(state._prevView || 'albums', state._prevData || null);
    }
    if (e.key === 'Enter') {
      clearTimeout(searchTimeout);
      var val = this.value.trim();
      if (val) navigate('search', val);
    }
  });
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

function setupKeyboard() {
  document.addEventListener('keydown', function(e) {
    if (e.target.tagName === 'INPUT') return;
    switch (e.key) {
      case ' ': e.preventDefault(); togglePlay(); break;
      case 'ArrowLeft': e.preventDefault(); seekRelative(-10); break;
      case 'ArrowRight': e.preventDefault(); seekRelative(10); break;
      case 'ArrowUp': e.preventDefault(); audio.volume=Math.min(1,audio.volume+0.1); document.getElementById('volume').value=audio.volume; break;
      case 'ArrowDown': e.preventDefault(); audio.volume=Math.max(0,audio.volume-0.1); document.getElementById('volume').value=audio.volume; break;
      case 'n': case 'N': nextTrack(); break;
      case 'p': case 'P': prevTrack(); break;
      case 'l': case 'L': { if (state.queue[state.currentIndex]) toggleFavorite(state.queue[state.currentIndex].id); break; }
      case 'r': case 'R': cycleRepeat(); break;
      case 's': case 'S': e.preventDefault(); document.getElementById('search').focus(); break;
      case 'Escape': closeModal(); hideContextMenu(); closeLyricsPanel(); closeEQPanel(); break;
    }
  });
}
