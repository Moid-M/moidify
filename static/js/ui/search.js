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
