var searchTimeout = null;

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

function _customKey(action) {
  var keys = safeJSON('moidify_custom_keys', {});
  var defaults = {
    'play-pause': ' ',
    'next': 'n',
    'prev': 'p',
    'volume-up': 'ArrowUp',
    'volume-down': 'ArrowDown',
    'seek-forward': 'ArrowRight',
    'seek-back': 'ArrowLeft',
    'like': 'l',
    'repeat': 'r',
    'search': 's',
    'queue': 'q',
    'escape': 'Escape',
  };
  return (keys[action] || defaults[action]).toLowerCase();
}

function setupKeyboard() {
  document.addEventListener('keydown', function(e) {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
    var key = e.key;
    var actions = {
      'play-pause': function() { e.preventDefault(); togglePlay(); },
      'next': function() { nextTrack(); },
      'prev': function() { prevTrack(); },
      'volume-up': function() { e.preventDefault(); audio.volume=Math.min(1,audio.volume+0.1); document.getElementById('volume').value=audio.volume; },
      'volume-down': function() { e.preventDefault(); audio.volume=Math.max(0,audio.volume-0.1); document.getElementById('volume').value=audio.volume; },
      'seek-forward': function() { e.preventDefault(); seekRelative(10); },
      'seek-back': function() { e.preventDefault(); seekRelative(-10); },
      'like': function() { if (state.queue[state.currentIndex]) toggleFavorite(state.queue[state.currentIndex].id); },
      'repeat': function() { cycleRepeat(); },
      'search': function() { e.preventDefault(); document.getElementById('search').focus(); },
      'queue': function() { toggleQueuePanel(); },
      'escape': function() { closeModal(); hideContextMenu(); closeLyricsPanel(); closeEQPanel(); },
    };
    var keys = safeJSON('moidify_custom_keys', {});
    var defaults = {
      'play-pause': ' ',
      'next': 'n',
      'prev': 'p',
      'volume-up': 'ArrowUp',
      'volume-down': 'ArrowDown',
      'seek-forward': 'ArrowRight',
      'seek-back': 'ArrowLeft',
      'like': 'l',
      'repeat': 'r',
      'search': 's',
      'queue': 'q',
      'escape': 'Escape',
    };
    for (var action in defaults) {
      var boundKey = (keys[action] || defaults[action]);
      if (key === boundKey || (key.length === 1 && key.toLowerCase() === boundKey.toLowerCase())) {
        if (actions[action]) actions[action]();
        return;
      }
    }
  });
}
