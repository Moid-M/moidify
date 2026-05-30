function addShuffleButton(tracks, label) {
  var filter = qs('.track-list-filter');
  if (!filter) return;
  var btn = document.createElement('button');
  btn.className = 'shuffle-view-btn';
  btn.innerHTML = iconShuffle() + ' <span>' + (label || 'Shuffle') + '</span>';
  btn.addEventListener('click', function() {
    var shuffled = tracks.slice();
    for (var i = shuffled.length - 1; i > 0; i--) {
      var j = Math.floor(Math.random() * (i + 1));
      var tmp = shuffled[i]; shuffled[i] = shuffled[j]; shuffled[j] = tmp;
    }
    playFromQueue(shuffled, 0);
    showToast('Playing ' + shuffled.length + ' shuffled ' + (label || 'tracks').toLowerCase(), 'info');
  });
  filter.insertBefore(btn, filter.firstChild);
}

function navigate(view, data) {
  document.body.classList.remove('sidebar-open');
  var prevView = state.currentView;
  var prevData = state.currentData;
  if (view === 'search' && prevView !== 'search') {
    state._prevView = prevView;
    state._prevData = prevData;
  }
  state.currentView = view; state.currentData = data;
  state._navId = (state._navId || 0) + 1;
  var navId = state._navId;
  qsa('.nav-item').forEach(function(el) { el.classList.toggle('active', el.dataset.view === view); });
  qsa('.playlist-item').forEach(function(el) { el.classList.remove('active'); });
  switch (view) {
    case 'home': renderHome(navId); break;
    case 'albums': renderAlbums(navId); break;
    case 'artists': renderArtists(navId); break;
    case 'tracks': renderTracks(data || null, navId); break;
    case 'favorites': renderFavorites(navId); break;
    case 'album': renderAlbumDetail(data, navId); break;
    case 'playlist': renderPlaylistDetail(data, navId); break;
    case 'artist-tracks': renderArtistTracks(data, navId); break;
    case 'genres': renderGenres(navId); break;
    case 'genre-tracks': renderGenreTracks(data, navId); break;
    case 'search': renderSearchResults(data, navId); break;
    default: renderHome(navId);
  }
}
