function showSettings() {
  var tabs = [
    { id:'theme', icon:'<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>', label:'Theme' },
    { id:'layout', icon:'<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>', label:'Layout' },
    { id:'animations', icon:'<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>', label:'Animations' },
    { id:'playback', icon:'<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>', label:'Playback' },
    { id:'shortcuts', icon:'<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="3" y1="9" x2="21" y2="9"/><line x1="9" y1="3" x2="9" y2="21"/></svg>', label:'Shortcuts' },
    { id:'lastfm', icon:'<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 16V8a3 3 0 0 1 3-3h4a3 3 0 0 1 3 3v1"/><path d="M17 16v-2a3 3 0 0 0-3-3h-2"/></svg>', label:'Last.fm' },
    { id:'advanced', icon:'<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>', label:'Advanced' },
    { id:'about', icon:'<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>', label:'About' },
  ];

  var sidebarHtml = tabs.map(function(t,i) {
    return '<div class="settings-sidebar-item'+(i===0?' active':'')+'" data-tab="'+t.id+'">'+t.icon+'<span>'+t.label+'</span></div>';
  }).join('');

  var contentHtml = tabs.map(function(t,i) {
    return '<div class="settings-tab-content'+(i===0?' active':'')+'" id="tab-'+t.id+'"></div>';
  }).join('');

  showModal('<div class="settings-layout"><div class="settings-sidebar">'+sidebarHtml+'</div><div class="settings-main">'+contentHtml+'</div></div>');
  var settingsModal = document.getElementById('modal');
  if (settingsModal) settingsModal.classList.add('settings-open');
  translateDOM(settingsModal);
  renderThemeTab();
  renderLayoutTab();
  renderAnimationsTab();
  renderPlaybackTab();
  renderShortcutsTab();
  renderLastfmTab();
  renderAdvancedTab();
  renderAboutTab();

  qsa('.settings-sidebar-item').forEach(function(el) {
    el.addEventListener('click', function() {
      if (this.classList.contains('active')) return;
      qsa('.settings-sidebar-item').forEach(function(t){t.classList.remove('active');});
      this.classList.add('active');

      var activeTab = qs('.settings-tab-content.active');
      var newTab = document.getElementById('tab-'+this.dataset.tab);
      if (activeTab && activeTab !== newTab) {
        activeTab.classList.remove('active');
      }
      if (newTab) newTab.classList.add('active');
    });
  });
}

function renderThemeTab() {
  var container = document.getElementById('tab-theme');
  var colors = ['#a0a0a0','#ef4444','#f97316','#eab308','#1db954','#3b82f6','#14b8a6'];
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

  var bgBrightness = localStorage.getItem('moidify_bg_brightness') || '100';
  var fontSize = localStorage.getItem('moidify_font_size') || 'normal';
  var fontSizeOpts = [
    {value:'small', label:'Small'},
    {value:'normal', label:'Normal'},
    {value:'large', label:'Large'},
  ];

  container.innerHTML =
    '<div class="settings-section" data-i18n-section="themeColor"><h3 data-i18n="settings.themeColor">Theme Color</h3><div class="color-grid">'+swatches+'</div>'+
    '<div class="color-custom-wrap"><input type="color" id="custom-color" value="'+state.accentColor+'"><span style="color:var(--text-muted);font-size:13px;">Custom</span></div></div>'+
    '<div class="settings-section"><h3 data-i18n="settings.appearance">Appearance</h3>'+
    '<label class="toggle-row" data-i18n-row="light"><span data-i18n="settings.lightMode">Light Mode</span><input type="checkbox" id="light-toggle"'+lightChecked+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row" data-i18n-row="autoTheme"><span data-i18n="settings.autoTheme">Follow system theme</span><input type="checkbox" id="auto-theme-toggle"'+autoChecked+'><span class="toggle-slider"></span></label></div>'+
    '<div class="settings-section"><h3>Background Brightness</h3>'+
    '<div class="crossfade-wrap"><span style="font-size:12px;color:var(--text-muted);min-width:30px;" id="bg-brightness-label">'+bgBrightness+'%</span>'+
    '<input type="range" id="bg-brightness-slider" min="50" max="150" step="5" value="'+bgBrightness+'"><span style="font-size:12px;color:var(--text-muted);">150%</span></div></div>'+
    '<div class="settings-section"><h3>Font Size</h3>'+
    '<div class="anim-speed-wrap" id="font-size-wrap">'+
    fontSizeOpts.map(function(f){return '<button class="anim-speed-btn'+(fontSize===f.value?' active':'')+'" data-size="'+f.value+'">'+f.label+'</button>';}).join('')+
    '</div></div>'+
    '<div class="settings-section"><h3 data-i18n="settings.language">Language</h3><select class="lang-select" id="lang-select">'+langOpts+'</select></div>'+
    '<div class="settings-section"><h3>Custom Colors</h3>'+
    '<p style="color:var(--text-muted);font-size:12px;margin-bottom:8px;">Override specific CSS variables. Leave default to keep current theme.</p>'+
    '<div class="theme-slider-row"><label>Background</label><input type="color" id="custom-bg-color" value="'+hexFromVar('--bg')+'"><span class="slider-val" id="custom-bg-label">bg</span></div>'+
    '<div class="theme-slider-row"><label>Cards</label><input type="color" id="custom-card-color" value="'+hexFromVar('--bg-card')+'"><span class="slider-val" id="custom-card-label">card</span></div>'+
    '<div class="theme-slider-row"><label>Text</label><input type="color" id="custom-text-color" value="'+hexFromVar('--text-primary')+'"><span class="slider-val" id="custom-text-label">text</span></div>'+
    '<div class="theme-slider-row"><label>Muted text</label><input type="color" id="custom-muted-color" value="'+hexFromVar('--text-muted')+'"><span class="slider-val" id="custom-muted-label">muted</span></div>'+
    '<div class="theme-slider-row"><label>Borders</label><input type="color" id="custom-border-color" value="'+hexFromVar('--border')+'"><span class="slider-val" id="custom-border-label">border</span></div>'+
    '</div>';

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

  var bgSlider = document.getElementById('bg-brightness-slider');
  if (bgSlider) {
    bgSlider.addEventListener('input', function() {
      var val = parseInt(this.value);
      document.getElementById('bg-brightness-label').textContent = val + '%';
      localStorage.setItem('moidify_bg_brightness', val);
      document.documentElement.style.setProperty('--bg-brightness', val + '%');
      applyBgBrightness(val);
    });
  }

  qsa('#font-size-wrap .anim-speed-btn', container).forEach(function(el) {
    el.addEventListener('click', function() {
      qsa('#font-size-wrap .anim-speed-btn', container).forEach(function(s){s.classList.remove('active');});
      this.classList.add('active');
      var size = this.dataset.size;
      localStorage.setItem('moidify_font_size', size);
      applyFontSize(size);
    });
  });

  // Custom color overrides
  var customVars = [
    { id:'custom-bg-color', var:'--bg' },
    { id:'custom-card-color', var:'--bg-card' },
    { id:'custom-text-color', var:'--text-primary' },
    { id:'custom-muted-color', var:'--text-muted' },
    { id:'custom-border-color', var:'--border' },
  ];
  customVars.forEach(function(cfg) {
    var input = document.getElementById(cfg.id);
    if (!input) return;
    input.addEventListener('input', function() {
      document.documentElement.style.setProperty(cfg.var, this.value);
      localStorage.setItem('moidify_theme_' + cfg.var.replace('--', ''), this.value);
    });
  });
}

function hexFromVar(name) {
  var val = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  if (val.startsWith('#')) return val;
  // Try stored override
  var stored = localStorage.getItem('moidify_theme_' + name.replace('--', ''));
  if (stored) return stored;
  return '#000000';
}

function applyBgBrightness(val) {
  var ratio = val / 100;
  var isLight = document.body.classList.contains('light-mode');
  var baseR = isLight ? 245 : 13;
  var baseG = isLight ? 245 : 13;
  var baseB = isLight ? 245 : 13;
  var r = Math.min(255, Math.round(baseR * ratio));
  var g = Math.min(255, Math.round(baseG * ratio));
  var b = Math.min(255, Math.round(baseB * ratio));
  var bgColor = 'rgb(' + r + ', ' + g + ', ' + b + ')';
  document.documentElement.style.setProperty('--bg', bgColor);
  if (isLight) {
    document.documentElement.style.setProperty('--bg-elevated', bgColor);
  }
}

function applyFontSize(size) {
  var sizes = {small: '13px', normal: '14px', large: '16px'};
  document.documentElement.style.setProperty('--font-size-base', sizes[size] || '14px');
  document.body.dataset.font = size;
}

function renderAnimationsTab() {
  var container = document.getElementById('tab-animations');
  var animEnabled = state.animations;
  var allOn = Object.values(animEnabled).every(function(v) { return v === true; });
  var speeds = ['slow','normal','fast','off'];
  var speedLabels = {slow:'Slow',normal:'Normal',fast:'Fast',off:'Off'};

  var vizStyles = ['bars', 'wave', 'blend'];
  var vizStyleLabels = {bars:'Bars', wave:'Wave', blend:'Blend'};

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
    '<div class="anim-toggle-row"><span>Visualizer bars</span><button class="toggle-switch'+(animEnabled.eqAnim?' on':'')+'" data-anim="eqAnim"></button></div>'+
    '<div class="anim-toggle-row"><span>Seek bar shimmer effect</span><button class="toggle-switch'+(animEnabled.seekShimmer?' on':'')+'" data-anim="seekShimmer"></button></div>'+
    '<div class="anim-toggle-row"><span>Queue slide animation</span><button class="toggle-switch'+(animEnabled.queueSlide?' on':'')+'" data-anim="queueSlide"></button></div>'+
    '<div class="anim-toggle-row"><span>Lyrics fade effect</span><button class="toggle-switch'+(animEnabled.lyricsFade?' on':'')+'" data-anim="lyricsFade"></button></div>'+
    '</div>'+
    '<div class="settings-section"><h3>Speed</h3><div class="anim-speed-wrap">'+
    speeds.map(function(s){return '<button class="anim-speed-btn'+(state.animSpeed===s?' active':'')+'" data-speed="'+s+'">'+speedLabels[s]+'</button>';}).join('')+
    '</div></div>'+
    '<div class="settings-section"><h3>Visualizer</h3>'+
    '<div class="anim-toggle-row"><span>Bar count</span><div style="display:flex;align-items:center;gap:8px;"><input type="range" id="viz-bars-slider" min="8" max="128" step="8" value="'+state.vizBars+'" style="width:80px;height:4px;"><span id="viz-bars-label" style="font-size:12px;color:var(--text-muted);min-width:20px;">'+state.vizBars+'</span></div></div>'+
    '<div class="anim-toggle-row"><span>Mirror mode</span><button class="toggle-switch'+(state.vizMirror?' on':'')+'" id="toggle-viz-mirror"></button></div>'+
    '<div class="anim-toggle-row" style="flex-wrap:wrap;"><span>Style</span><div class="anim-speed-wrap" style="margin:0;">'+
    vizStyles.map(function(s){return '<button class="anim-speed-btn viz-style-btn'+(state.vizStyle===s?' active':'')+'" data-style="'+s+'">'+vizStyleLabels[s]+'</button>';}).join('')+
    '</div></div>'+
    '</div>'+
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

  var mirrorToggle = document.getElementById('toggle-viz-mirror');
  if (mirrorToggle) {
    mirrorToggle.addEventListener('click', function() {
      state.vizMirror = !state.vizMirror;
      this.classList.toggle('on');
      localStorage.setItem('moidify_viz_mirror', state.vizMirror);
    });
  }

  var vizBarsSlider = document.getElementById('viz-bars-slider');
  if (vizBarsSlider) {
    vizBarsSlider.addEventListener('input', function() {
      var val = parseInt(this.value);
      state.vizBars = val;
      localStorage.setItem('moidify_viz_bars', val);
      var label = document.getElementById('viz-bars-label');
      if (label) label.textContent = val;
    });
  }

  qsa('.viz-style-btn', container).forEach(function(el) {
    el.addEventListener('click', function() {
      qsa('.viz-style-btn', container).forEach(function(s){s.classList.remove('active');});
      this.classList.add('active');
      state.vizStyle = this.dataset.style;
      localStorage.setItem('moidify_viz_style', state.vizStyle);
    });
  });
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

  var qualityLabels = {original:'Original (passthrough)', high:'High (192k Opus)', medium:'Medium (128k Opus)', low:'Low (96k Opus)', voice:'Voice (64k Opus)'};
  var qualityBtns = Object.keys(qualityLabels).map(function(k) {
    return '<button class="eq-preset-btn'+(state.streamQuality===k?' active':'')+'" data-quality="'+k+'">'+qualityLabels[k]+'</button>';
  }).join('');

  var gaplessChecked = state.gapless ? ' checked' : '';
  var autoplayChecked = state.autoplay ? ' checked' : '';
  var volumeNormChecked = state.volumeNorm ? ' checked' : '';

  container.innerHTML =
    '<div class="settings-section"><h3>Equalizer</h3><div class="eq-presets">'+presetHtml+'</div><div class="eq-grid">'+bands+'</div></div>'+
    '<div class="settings-section"><h3>Stream Quality</h3>'+
    '<div class="eq-presets">'+qualityBtns+'</div>'+
    '<p style="font-size:12px;color:var(--text-muted);margin-top:4px">Requires ffmpeg on the server for transcoding. Otherwise plays original.</p></div>'+
    '<div class="settings-section"><h3>Gapless Playback</h3>'+
    '<label class="toggle-row"><span>Preload and crossfade next track</span><input type="checkbox" id="gapless-toggle"'+gaplessChecked+'><span class="toggle-slider"></span></label></div>'+
    '<div class="settings-section"><h3>Autoplay / Radio</h3>'+
    '<label class="toggle-row"><span>Auto-play similar tracks when queue ends</span><input type="checkbox" id="autoplay-toggle"'+autoplayChecked+'><span class="toggle-slider"></span></label></div>'+
    '<div class="settings-section"><h3>Volume Normalization</h3>'+
    '<label class="toggle-row"><span>Apply ReplayGain volume normalization</span><input type="checkbox" id="volume-norm-toggle"'+volumeNormChecked+'><span class="toggle-slider"></span></label></div>'+
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

  var gaplessToggle = document.getElementById('gapless-toggle');
  if (gaplessToggle) {
    gaplessToggle.addEventListener('change', function() {
      state.gapless = this.checked;
      localStorage.setItem('moidify_gapless', state.gapless);
    });
  }

  var autoplayToggle = document.getElementById('autoplay-toggle');
  if (autoplayToggle) {
    autoplayToggle.addEventListener('change', function() {
      state.autoplay = this.checked;
      localStorage.setItem('moidify_autoplay', state.autoplay);
    });
  }

  var volumeNormToggle = document.getElementById('volume-norm-toggle');
  if (volumeNormToggle) {
    volumeNormToggle.addEventListener('change', function() {
      state.volumeNorm = this.checked;
      localStorage.setItem('moidify_volume_norm', state.volumeNorm);
    });
  }
}

function renderLastfmTab() {
  var container = document.getElementById('tab-lastfm');
  var token = getAuthToken();
  if (!token) {
    container.innerHTML = '<div class="settings-section"><h3>Last.fm Scrobbling</h3><p style="color:var(--text-muted);">Log in to connect your Last.fm account.</p></div>';
    return;
  }

  fetch('/api/lastfm/status', {headers:{'token':token}}).then(function(r){return r.json();}).then(function(status){
    if (status.connected) {
      container.innerHTML =
        '<div class="settings-section"><h3>Last.fm Scrobbling</h3>'+
        '<p style="color:var(--text-secondary);margin-bottom:12px;">Connected as <strong>'+esc(status.username)+'</strong></p>'+
        '<button onclick="disconnectLastfm()" class="btn-secondary" style="padding:6px 16px;font-size:13px;border-radius:20px;border:1px solid var(--danger);color:var(--danger);background:transparent;cursor:pointer;">Disconnect</button>'+
        '</div>';
    } else {
      container.innerHTML =
        '<div class="settings-section"><h3>Last.fm Scrobbling</h3>'+
        '<p style="color:var(--text-muted);margin-bottom:12px;">Connect your Last.fm account to scrobble your listens.</p>'+
        '<div style="display:flex;flex-direction:column;gap:8px;max-width:300px;">'+
        '<input type="text" id="lastfm-username" placeholder="Last.fm username" style="padding:8px;border-radius:8px;border:1px solid var(--border);background:var(--bg);color:var(--text-primary);">'+
        '<input type="password" id="lastfm-password" placeholder="Password" style="padding:8px;border-radius:8px;border:1px solid var(--border);background:var(--bg);color:var(--text-primary);">'+
        '<button onclick="connectLastfm()" class="btn-primary" style="padding:8px;border-radius:20px;background:var(--accent);color:white;border:none;cursor:pointer;">Connect</button>'+
        '<p id="lastfm-error" style="color:var(--danger);font-size:13px;display:none;"></p>'+
        '</div>'+
        '</div>';
    }
  }).catch(function(){
    container.innerHTML = '<div class="settings-section"><h3>Last.fm Scrobbling</h3><p style="color:var(--text-muted);">Could not load status.</p></div>';
  });
}

function connectLastfm() {
  var username = document.getElementById('lastfm-username').value.trim();
  var password = document.getElementById('lastfm-password').value;
  var errEl = document.getElementById('lastfm-error');
  if (!username || !password) {
    if (errEl) { errEl.textContent = 'Please fill in both fields.'; errEl.style.display = ''; }
    return;
  }
  var token = getAuthToken();
  fetch('/api/lastfm/connect', {
    method:'POST',
    headers:{'Content-Type':'application/json', 'token':token},
    body:JSON.stringify({username:username, password:password}),
  }).then(function(r){
    if (!r.ok) { return r.json().then(function(d){throw new Error(d.detail||'Connection failed');}); }
    renderLastfmTab();
  }).catch(function(e){
    if (errEl) { errEl.textContent = e.message; errEl.style.display = ''; }
  });
}

function disconnectLastfm() {
  var token = getAuthToken();
  fetch('/api/lastfm/disconnect', {method:'POST', headers:{'token':token}}).then(function(r){
    renderLastfmTab();
  });
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

function renderLayoutTab() {
  var container = document.getElementById('tab-layout');
  var density = localStorage.getItem('moidify_density') || 'normal';
  var colCovers = localStorage.getItem('moidify_col_covers') !== '0';
  var colTitle = localStorage.getItem('moidify_col_title') !== '0';
  var colArtist = localStorage.getItem('moidify_col_artist') !== '0';
  var colAlbum = localStorage.getItem('moidify_col_album') !== '0';
  var colDuration = localStorage.getItem('moidify_col_duration') !== '0';
  var colGenre = localStorage.getItem('moidify_col_genre') === '1';
  var colPlays = localStorage.getItem('moidify_col_plays') === '1';
  container.innerHTML =
    '<div class="settings-section"><h3>Grid Density</h3><div class="anim-speed-wrap">'+
    ['compact','normal','comfortable'].map(function(d) {
      return '<button class="anim-speed-btn'+(density===d?' active':'')+'" data-density="'+d+'">'+
        d.charAt(0).toUpperCase()+d.slice(1)+'</button>';
    }).join('')+'</div></div>'+
    '<div class="settings-section"><h3>Track List Columns</h3>'+
    '<label class="toggle-row"><span>Cover art</span><input type="checkbox" class="col-toggle" data-col="covers"'+(colCovers?' checked':'')+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row"><span>Title</span><input type="checkbox" class="col-toggle" data-col="title"'+(colTitle?' checked':'')+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row"><span>Artist</span><input type="checkbox" class="col-toggle" data-col="artist"'+(colArtist?' checked':'')+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row"><span>Album</span><input type="checkbox" class="col-toggle" data-col="album"'+(colAlbum?' checked':'')+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row"><span>Duration</span><input type="checkbox" class="col-toggle" data-col="duration"'+(colDuration?' checked':'')+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row"><span>Genre</span><input type="checkbox" class="col-toggle" data-col="genre"'+(colGenre?' checked':'')+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row"><span>Play count</span><input type="checkbox" class="col-toggle" data-col="plays"'+(colPlays?' checked':'')+'><span class="toggle-slider"></span></label></div>'+
    '<div class="settings-section"><h3>Sidebar</h3>'+
    '<label class="toggle-row"><span>Show All Tracks</span><input type="checkbox" id="nav-tracks-toggle"'+(localStorage.getItem('moidify_nav_tracks')!=='0'?' checked':'')+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row"><span>Show Albums</span><input type="checkbox" id="nav-albums-toggle"'+(localStorage.getItem('moidify_nav_albums')!=='0'?' checked':'')+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row"><span>Show Artists</span><input type="checkbox" id="nav-artists-toggle"'+(localStorage.getItem('moidify_nav_artists')!=='0'?' checked':'')+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row"><span>Show Genres</span><input type="checkbox" id="nav-genres-toggle"'+(localStorage.getItem('moidify_nav_genres')!=='0'?' checked':'')+'><span class="toggle-slider"></span></label>'+
    '<label class="toggle-row"><span>Show Liked Songs</span><input type="checkbox" id="nav-favorites-toggle"'+(localStorage.getItem('moidify_nav_favorites')!=='0'?' checked':'')+'><span class="toggle-slider"></span></label></div>';
  qsa('.anim-speed-btn[data-density]', container).forEach(function(btn) {
    btn.addEventListener('click', function() {
      qsa('.anim-speed-btn', container).forEach(function(b){b.classList.remove('active');});
      this.classList.add('active');
      localStorage.setItem('moidify_density', this.dataset.density);
      document.body.dataset.density = this.dataset.density;
    });
  });
  qsa('.col-toggle', container).forEach(function(cb) {
    cb.addEventListener('change', function() {
      var col = this.dataset.col;
      var key = 'moidify_col_' + col;
      localStorage.setItem(key, this.checked ? '1' : '0');
      document.body.dataset['col' + col.charAt(0).toUpperCase() + col.slice(1)] = this.checked ? '1' : '0';
    });
  });
  ['tracks','albums','artists','genres','favorites'].forEach(function(view) {
    var el = document.getElementById('nav-'+view+'-toggle');
    if (!el) return;
    el.addEventListener('change', function() {
      localStorage.setItem('moidify_nav_'+view, this.checked ? '1' : '0');
      var navItem = qs('.nav-item[data-view="'+view+'"]');
      if (navItem) navItem.style.display = this.checked ? '' : 'none';
    });
  });
}

function renderShortcutsTab() {
  var container = document.getElementById('tab-shortcuts');
  var customKeys = safeJSON('moidify_custom_keys', {});
  var defaults = {
    'play-pause': 'Space',
    'next': 'N',
    'prev': 'P',
    'volume-up': 'ArrowUp',
    'volume-down': 'ArrowDown',
    'seek-forward': 'ArrowRight',
    'seek-back': 'ArrowLeft',
    'like': 'L',
    'repeat': 'R',
    'search': 'S',
    'queue': 'Q',
    'escape': 'Escape',
  };
  var html = '<div class="settings-section"><h3>Custom Keyboard Shortcuts</h3><p style="color:var(--text-muted);font-size:13px;margin-bottom:12px;">Click a shortcut to rebind. Press <kbd>Esc</kbd> to clear.</p>';
  for (var action in defaults) {
    var val = customKeys[action] || defaults[action];
    html += '<div class="shortcut-row" data-action="'+action+'"><span class="shortcut-label">'+action.replace('-',' ').replace(/\b\w/g,function(l){return l.toUpperCase();})+'</span>'+
      '<span class="shortcut-key" tabindex="0">'+esc(val)+'</span></div>';
  }
  html += '</div>';
  container.innerHTML = html;
  qsa('.shortcut-key', container).forEach(function(el) {
    el.addEventListener('keydown', function(e) {
      e.preventDefault();
      var key = e.key === ' ' ? 'Space' : e.key;
      if (key === 'Escape') { this.textContent = ''; return; }
      this.textContent = key;
      var action = this.closest('.shortcut-row').dataset.action;
      var keys = safeJSON('moidify_custom_keys', {});
      keys[action] = key;
      localStorage.setItem('moidify_custom_keys', JSON.stringify(keys));
      this.blur();
    });
    el.addEventListener('click', function() { this.focus(); this.textContent = '...'; });
  });
}

function renderAdvancedTab() {
  var container = document.getElementById('tab-advanced');
  var customCss = localStorage.getItem('moidify_custom_css') || '';
  container.innerHTML =
    '<div class="settings-section"><h3>Custom CSS</h3><p style="color:var(--text-muted);font-size:13px;margin-bottom:8px;">Paste custom CSS to override any styles. Changes apply immediately.</p>'+
    '<textarea id="custom-css-editor" class="css-editor" spellcheck="false">'+esc(customCss)+'</textarea></div>'+
    '<div class="settings-section"><h3>Reset to Defaults</h3><p style="color:var(--text-muted);font-size:13px;margin-bottom:8px;">Reset all settings to their default values. This cannot be undone.</p>'+
    '<button class="reset-defaults-btn" id="reset-defaults-btn">Reset All Settings</button></div>';
  var editor = document.getElementById('custom-css-editor');
  if (editor) {
    editor.addEventListener('input', function() {
      localStorage.setItem('moidify_custom_css', this.value);
      applyCustomCSS(this.value);
    });
  }
  var resetBtn = document.getElementById('reset-defaults-btn');
  if (resetBtn) {
    resetBtn.addEventListener('click', function() {
      if (!confirm('Reset all settings to defaults?')) return;
      localStorage.clear();
      location.reload();
    });
  }
}

function applyCustomCSS(css) {
  var existing = document.getElementById('custom-css-style');
  if (existing) existing.remove();
  if (!css || !css.trim()) return;
  var style = document.createElement('style');
  style.id = 'custom-css-style';
  style.textContent = css;
  document.head.appendChild(style);
}
