function safeJSON(key, fallback) {
  try { return JSON.parse(localStorage.getItem(key) || JSON.stringify(fallback)); }
  catch(e) { return fallback; }
}

var _mq = localStorage.getItem('moidify_stream_quality') || 'high';
var state = {
  token: localStorage.getItem('moidify_token'),
  user: null,
  currentView: 'albums',
  currentData: null,
  queue: [],
  currentIndex: -1,
  playlists: [],
  accentColor: localStorage.getItem('moidify_accent') || '#a855f7',
  eq: safeJSON('moidify_eq', null),
  eqPreset: localStorage.getItem('moidify_eq_preset') || 'Normal',
  crossfade: parseFloat(localStorage.getItem('moidify_crossfade') || '0'),
  animations: safeJSON('moidify_animations', {"cards":true,"rows":true,"transitions":true,"queueSlide":true,"vinylSpin":false,"glowPulse":true,"coverZoom":true,"eqAnim":true,"seekShimmer":true,"cdHole":true,"lyricsFade":true}),
  animSpeed: localStorage.getItem('moidify_anim_speed') || 'normal',
  vinylSpinSpeed: parseInt(localStorage.getItem('moidify_vinyl_speed') || '4'),
  repeatMode: localStorage.getItem('moidify_repeat') || 'off',
  shuffle: localStorage.getItem('moidify_shuffle') === 'true',
  lightMode: localStorage.getItem('moidify_light') === 'true',
  autoTheme: localStorage.getItem('moidify_auto_theme') !== 'false',
  sleepTimer: null,
  pinnedPlaylists: safeJSON('moidify_pinned', []),
  viewMode: localStorage.getItem('moidify_view') || 'grid',
  selectedTrackIds: [],
  shuffleOrder: [],
  shuffleIndex: 0,
  playHistory: [],
  smoothSeek: localStorage.getItem('moidify_smooth_seek') !== 'false',
  showTrackCovers: localStorage.getItem('moidify_show_track_covers') !== 'false',
  sortBy: null,
  sortDir: 'asc',
  currentTracks: [],
  currentQueue: [],
  _favedFlag: false,
  favedTracks: {},
  streamQuality: _mq || 'high',
  playbackSpeed: parseFloat(localStorage.getItem('moidify_speed') || '1'),
  gapless: localStorage.getItem('moidify_gapless') !== 'false',
  autoplay: localStorage.getItem('moidify_autoplay') === 'true',
  volumeNorm: localStorage.getItem('moidify_volume_norm') === 'true',
  pinnedAlbums: safeJSON('moidify_pinned_albums', []),
  pinnedArtists: safeJSON('moidify_pinned_artists', []),
  albumGrouping: localStorage.getItem('moidify_album_grouping') === '1',
  expandedAlbumGroups: safeJSON('moidify_expanded_groups', []),
  vizBars: parseInt(localStorage.getItem('moidify_viz_bars') || '32'),
  vizMirror: localStorage.getItem('moidify_viz_mirror') === 'true',
  vizStyle: localStorage.getItem('moidify_viz_style') || 'bars',
};

var audio = document.getElementById('audio');
var audioCtx = null;
var gainNode = null;
var eqNodes = [];
var isCrossfading = false;
var analyserNode = null;

var SLEEP_OPTIONS = [
  {label:'Off', value:0},
  {label:'End of Track', value:-1},
  {label:'End of Queue', value:-2},
  {label:'5 min', value:300},
  {label:'10 min', value:600},
  {label:'15 min', value:900},
  {label:'30 min', value:1800},
  {label:'45 min', value:2700},
  {label:'60 min', value:3600},
];

var EQ_PRESETS = {
  'Normal':        [0,0,0,0,0,0,0,0,0,0],
  'Bass Boost':    [6,5,4,2,0,-1,-2,-3,-4,-5],
  'Treble Boost':  [-4,-3,-2,-1,0,1,2,4,6,8],
  'Classical':     [4,3,2,1,0,0,1,2,3,4],
  'Dance':         [6,4,2,1,0,-1,-1,-2,-3,-4],
  'Jazz':          [4,3,1,0,0,1,2,3,4,5],
  'Pop':           [-2,-1,0,2,4,4,2,0,-1,-2],
  'Rock':          [5,4,2,0,-1,-2,0,2,4,5],
  'Vocal':         [-2,-2,-1,0,2,4,5,4,2,0],
};

var PALETTE = {
  '#a855f7': '#c084fc', '#7c3aed': '#a78bfa', '#d946ef': '#f0abfc', '#ec4899': '#f472b6',
  '#be185d': '#ec4899', '#f43f5e': '#fb7185', '#ef4444': '#f87171', '#f97316': '#fb923c',
  '#eab308': '#facc15', '#1db954': '#22c55e', '#06b6d4': '#22d3ee', '#14b8a6': '#2dd4bf',
  '#3b82f6': '#60a5fa', '#8b5cf6': '#a78bfa',
};

function formatTime(sec) {
  if (!sec || isNaN(sec)) return '0:00';
  var m = Math.floor(sec / 60);
  var s = Math.floor(sec % 60);
  return m + ':' + (s < 10 ? '0' : '') + s;
}

function formatTimeLong(sec) {
  if (!sec || isNaN(sec)) return '0:00';
  var h = Math.floor(sec / 3600);
  var m = Math.floor((sec % 3600) / 60);
  var s = Math.floor(sec % 60);
  if (h > 0) return h + 'h ' + m + 'm ' + s + 's';
  return m + 'm ' + s + 's';
}

function qs(sel, parent) { return (parent || document).querySelector(sel); }
function qsa(sel, parent) { return Array.from((parent || document).querySelectorAll(sel)); }

function skeletonGrid(count) {
  var html = '<div class="skeleton-grid">';
  for (var i = 0; i < (count || 12); i++) {
    html += '<div class="skeleton-card"><div class="skeleton-img"></div><div class="skeleton-line"></div><div class="skeleton-line"></div></div>';
  }
  return html + '</div>';
}

function skeletonTrackRows(count) {
  var html = '';
  for (var i = 0; i < (count || 10); i++) {
    html += '<div class="skeleton-track"><div class="skeleton-num"></div><div class="skeleton-img-small"></div><div class="skeleton-line"></div><div class="skeleton-line"></div><div class="skeleton-line"></div></div>';
  }
  return html;
}

function esc(s) {
  if (s == null) return '';
  var d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

function applyAccent(color) {
  state.accentColor = color;
  document.documentElement.style.setProperty('--accent', color);
  var hover = PALETTE[color] || lightenColor(color, 30);
  document.documentElement.style.setProperty('--accent-hover', hover);
  var dim = hexToRgba(color, 0.15);
  document.documentElement.style.setProperty('--accent-dim', dim);
  localStorage.setItem('moidify_accent', color);
}

function applyTheme() {
  var light = state.lightMode;
  if (state.autoTheme && window.matchMedia) {
    light = window.matchMedia('(prefers-color-scheme: light)').matches;
  }
  document.body.classList.toggle('light-mode', light);
  localStorage.setItem('moidify_light', state.lightMode);
  localStorage.setItem('moidify_auto_theme', state.autoTheme);
}

function lightenColor(hex, percent) {
  var num = parseInt(hex.replace('#', ''), 16);
  var amt = Math.round(2.55 * percent);
  var R = Math.min(255, (num >> 16) + amt);
  var G = Math.min(255, ((num >> 8) & 0x00FF) + amt);
  var B = Math.min(255, (num & 0x0000FF) + amt);
  return '#' + (0x1000000 + R * 0x10000 + G * 0x100 + B).toString(16).slice(1);
}

function hexToRgba(hex, alpha) {
  var num = parseInt(hex.replace('#', ''), 16);
  return 'rgba(' + (num >> 16) + ', ' + ((num >> 8) & 0xFF) + ', ' + (num & 0xFF) + ', ' + alpha + ')';
}
