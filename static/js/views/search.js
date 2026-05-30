async function renderSearchResults(query, navId) {
  var content = document.getElementById('content');
  content.innerHTML = '<div class="content-header"><div class="view-title">Search: "'+esc(query)+'"</div></div><div class="sr-full-loading">Searching...</div>';

  try {
    var q = query.toLowerCase();
    var qNorm = q.normalize ? q.normalize('NFKD').replace(/[\u0300-\u036f]/g, '') : q;

    var [tracks, albums, artists] = await Promise.all([
      apiJson('/api/tracks?search='+encodeURIComponent(query)),
      apiJson('/api/albums'),
      apiJson('/api/artists'),
    ]);
    if (state._navId !== navId) return;

    function matchField(val) {
      if (!val) return false;
      var v = val.toLowerCase();
      var vNorm = v.normalize ? v.normalize('NFKD').replace(/[\u0300-\u036f]/g, '') : v;
      return v.indexOf(q) !== -1 || vNorm.indexOf(qNorm) !== -1;
    }

    var matchedAlbums = albums.filter(function(a) { return matchField(a.album); }).slice(0, 12);
    var matchedArtists = artists.filter(function(a) { return matchField(a.artist); }).slice(0, 10);
    var matchedTracks = tracks.slice(0, 50);
    var total = matchedAlbums.length + matchedArtists.length + matchedTracks.length;

    var headerHtml = '<div class="content-header"><div class="view-title">Search: "'+esc(query)+'"</div><span style="font-size:13px;color:var(--text-muted);font-weight:400;margin-top:2px;">'+total+' result'+(total!==1?'s':'')+'</span></div>';

    if (!total) {
      content.innerHTML = headerHtml + '<div class="sr-full-empty"><div style="font-size:48px;opacity:0.15;margin-bottom:16px;">\uD83D\uDD0D</div><p style="font-size:18px;font-weight:600;">No results for "'+esc(query)+'"</p><p style="color:var(--text-muted);margin-top:4px;">Try a different search term</p></div>';
      return;
    }

    var html = '';

    if (matchedAlbums.length) {
      html += '<div class="sr-full-section"><h3 class="sr-full-title">Albums</h3><div class="album-grid">';
      matchedAlbums.forEach(function(a) {
        html += '<div class="album-card sr-album" data-album="'+esc(a.album)+'" data-artist="'+esc(a.artist||'')+'">'+
          '<img src="/api/cover/'+a.cover_track_id+'" alt="" loading="lazy">'+
          '<div class="album-name">'+esc(a.album)+'</div>'+
          '<div class="album-artist">'+esc(a.artist||'Unknown Artist')+'</div></div>';
      });
      html += '</div></div>';
    }

    if (matchedArtists.length) {
      html += '<div class="sr-full-section"><h3 class="sr-full-title">Artists</h3><div class="sr-full-artists">';
      matchedArtists.forEach(function(a) {
        html += '<div class="sr-full-artist" data-artist="'+esc(a.artist)+'">'+
          '<div class="sr-full-artist-icon">\uD83C\uDFA4</div>'+
          '<div class="sr-full-artist-info">'+
            '<div class="sr-full-artist-name">'+esc(a.artist)+'</div>'+
            '<div class="sr-full-artist-meta">'+a.track_count+' tracks'+(a.album_count ? ', '+a.album_count+' albums' : '')+'</div>'+
          '</div></div>';
      });
      html += '</div></div>';
    }

    if (matchedTracks.length) {
      html += '<div class="sr-full-section"><h3 class="sr-full-title">Tracks</h3><div class="track-list">'+trackHeaderHTML()+'</div></div>';
    }

    content.innerHTML = headerHtml + html;

    if (matchedTracks.length) {
      var trackList = content.querySelector('.track-list');
      matchedTracks.forEach(function(t, i) {
        trackList.appendChild(createTrackRow(t, i, matchedTracks));
      });
      state.currentTracks = matchedTracks; state.currentQueue = matchedTracks; state._favedFlag = false;
      setupTrackSorting(trackList, matchedTracks);
    }

    qsa('.sr-album', content).forEach(function(el) {
      el.addEventListener('click', function() {
        navigate('album', {album: this.dataset.album, artist: this.dataset.artist});
        document.getElementById('search').value = '';
      });
    });

    qsa('.sr-full-artist', content).forEach(function(el) {
      el.addEventListener('click', function() {
        navigate('artist-tracks', this.dataset.artist);
        document.getElementById('search').value = '';
      });
    });

  } catch(e) {
    content.innerHTML = '<div class="content-header"><div class="view-title">Search</div></div><div class="sr-full-empty"><p style="color:var(--danger);">Error: '+esc(e.message)+'</p></div>';
  }
}
