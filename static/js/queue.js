function addTrackToQueueNext(track) {
  if (state.queue.length===0 || state.currentIndex<0) { state.queue=[track]; state.currentIndex=0; playFromQueue(state.queue,0); return; }
  state.queue.splice(state.currentIndex+1,0,track); renderQueuePanel();
}

function addTrackToQueueEnd(track) {
  if (state.queue.length===0) { state.queue=[track]; state.currentIndex=-1; }
  else state.queue.push(track);
  renderQueuePanel();
}

function addMultipleToQueue(trackIds) {
  if (!trackIds || !trackIds.length) return;
  trackIds.forEach(function(id) {
    var row = qs('[data-track-id="'+id+'"]');
    if (!row) return;
    var dur = parseFloat(row.dataset.trackDuration) || 0;
    addTrackToQueueEnd({
      id: id,
      title: qs('.track-title', row).textContent,
      artist: qs('.track-artist', row).textContent,
      album: qs('.track-album', row).textContent,
      duration: dur,
    });
  });
}

function removeFromQueue(index) {
  if (index<0||index>=state.queue.length) return;
  if (index===state.currentIndex) { nextTrack(); renderQueuePanel(); return; }
  state.queue.splice(index,1);
  if (index<state.currentIndex) state.currentIndex--;
  renderQueuePanel();
}

function moveInQueue(fromIndex, toIndex) {
  if (fromIndex<0||fromIndex>=state.queue.length||toIndex<0||toIndex>=state.queue.length||fromIndex===toIndex) return;
  var item = state.queue.splice(fromIndex,1)[0];
  state.queue.splice(toIndex,0,item);
  if (fromIndex===state.currentIndex) state.currentIndex=toIndex;
  else if (fromIndex<state.currentIndex && toIndex>=state.currentIndex) state.currentIndex--;
  else if (fromIndex>state.currentIndex && toIndex<=state.currentIndex) state.currentIndex++;
  // Rebuild shuffle order after manual reorder
  if (state.shuffle && state.shuffleOrder.length) {
    state.shuffleOrder = generateShuffleOrder(state.queue.length);
    state.shuffleIndex = 0;
    for (var si = 0; si < state.shuffleOrder.length; si++) {
      if (state.shuffleOrder[si] === state.currentIndex) { state.shuffleIndex = si; break; }
    }
  }
  renderQueuePanel();
}

function clearQueue() {
  state.queue=[]; state.currentIndex=-1;
  audio.pause(); audio.src=''; qs('#play-btn').innerHTML=iconPlay();
  renderQueuePanel(); clearAnimations();
  document.getElementById('player-title').textContent='';
  document.getElementById('player-artist').textContent='';
  document.getElementById('player-cover').src='/static/logo.png';
  var bd=document.getElementById('backdrop'); bd.classList.remove('visible'); bd.style.background='';
}

function autoShowQueue() {
  var panel = document.getElementById('queue-panel');
  if (panel.style.display === 'none' || panel.style.display === '') {
    if (!window._queueAutoShown) {
      window._queueAutoShown = true;
      toggleQueuePanel();
    }
  }
  updateQueueBadge();
}

function updateQueueBadge() {
  var badge = document.getElementById('queue-badge');
  var count = Math.max(0, state.queue.length - state.currentIndex - 1);
  if (count > 0) {
    badge.textContent = count > 9 ? '9+' : count;
    badge.style.display = 'flex';
  } else {
    badge.style.display = 'none';
  }
}

function renderQueuePanel() {
  var list = document.getElementById('queue-list');
  var count = document.getElementById('queue-count');
  var cover = document.getElementById('queue-cover');
  var qTitle = document.getElementById('queue-title');
  var qArtist = document.getElementById('queue-artist');
  if (!list) return;

  if (state.currentIndex>=0 && state.queue[state.currentIndex]) {
    var cur = state.queue[state.currentIndex];
    cover.src = '/api/cover/'+cur.id;
    qTitle.textContent = cur.title||'';
    qArtist.textContent = (cur.artist||'')+(cur.album?'  '+cur.album:'');
  } else {
    cover.src = '/static/logo.png';
    qTitle.textContent = 'Nothing playing'; qArtist.textContent = '';
  }

  var upcoming = state.queue.slice(state.currentIndex+1);
  count.textContent = upcoming.length+' tracks';
  updateQueueBadge();
  var jumpBtn = document.getElementById('queue-jump-btn');
  if (jumpBtn) {
    if (upcoming.length > 0) {
      jumpBtn.style.display = '';
      jumpBtn.onclick = function() {
        var el = qs('.qitem[data-qi="'+(state.currentIndex+1)+'"]');
        if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' });
      };
    } else {
      jumpBtn.style.display = 'none';
    }
  }

  if (upcoming.length===0) { list.innerHTML = '<div style="color:var(--text-muted);font-size:13px;padding:20px 0;text-align:center;">Queue is empty</div>'; return; }

  // When shuffle is on, show upcoming tracks from pre-generated shuffle order
  if (state.shuffle && state.shuffleOrder.length) {
    list.innerHTML = '';
    var qIcon = iconShuffle();
    for (var si = state.shuffleIndex + 1; si < state.shuffleOrder.length; si++) {
      var track = state.queue[state.shuffleOrder[si]];
      if (!track) continue;
      var item = document.createElement('div');
      item.className = 'qitem'; item.dataset.qi = state.shuffleOrder[si];
      item.innerHTML = '<span class="qitem-drag" style="opacity:0.4;">'+qIcon+'</span>'+
        '<div class="qitem-info"><div class="qitem-title">'+esc(track.title||'')+'</div><div class="qitem-artist">'+esc(track.artist||'')+'</div></div>'+
        '<button class="qitem-remove" data-qi="'+state.shuffleOrder[si]+'" title="Remove">'+iconClose()+'</button>';
      (function(idx) {
        qs('.qitem-remove',item).addEventListener('click',function(e){e.stopPropagation();removeFromQueue(idx);});
        item.addEventListener('click',function(e){if(e.target.closest('.qitem-remove'))return;playFromQueue(state.queue,idx);});
      })(state.shuffleOrder[si]);
      list.appendChild(item);
    }
    return;
  }

  list.innerHTML = '';
  upcoming.forEach(function(track, i) {
    var actualIndex = state.currentIndex+1+i;
    var item = document.createElement('div');
    item.className = 'qitem'; item.draggable = true; item.dataset.qi = actualIndex;
    item.innerHTML = '<span class="qitem-drag">'+iconDrag()+'</span>'+
      '<div class="qitem-info"><div class="qitem-title">'+esc(track.title||'')+'</div><div class="qitem-artist">'+esc(track.artist||'')+'</div></div>'+
      '<button class="qitem-remove" data-qi="'+actualIndex+'" title="Remove">'+iconClose()+'</button>';
    qs('.qitem-remove',item).addEventListener('click',function(e){e.stopPropagation();removeFromQueue(parseInt(this.dataset.qi));});
    item.addEventListener('click',function(e){if(e.target.closest('.qitem-remove')||e.target.closest('.qitem-drag'))return;playFromQueue(state.queue,actualIndex);});
    item.addEventListener('dragstart',function(e){e.dataTransfer.setData('text/plain',this.dataset.qi);this.classList.add('dragging');});
    item.addEventListener('dragend',function(){this.classList.remove('dragging');qsa('.qitem').forEach(function(el){el.classList.remove('drag-over');});});
    item.addEventListener('dragover',function(e){e.preventDefault();qsa('.qitem').forEach(function(el){el.classList.remove('drag-over');});this.classList.add('drag-over');});
    item.addEventListener('dragleave',function(){this.classList.remove('drag-over');});
    item.addEventListener('drop',function(e){e.preventDefault();qsa('.qitem').forEach(function(el){el.classList.remove('drag-over');});var from=parseInt(e.dataTransfer.getData('text/plain'));var to=parseInt(this.dataset.qi);if(from!==to)moveInQueue(from,to);});
    list.appendChild(item);
  });
}

function toggleQueuePanel() {
  var panel = document.getElementById('queue-panel');
  var app = document.getElementById('app');
  var btn = document.getElementById('queue-toggle-btn');
  if (panel.style.display!=='none') {
    panel.style.display='none'; app.classList.remove('has-queue'); btn.classList.remove('active');
  } else {
    panel.style.display='flex'; app.classList.add('has-queue'); btn.classList.add('active');
    renderQueuePanel();
  }
}

function generateShuffleOrder(n) {
  var order = [];
  for (var i = 0; i < n; i++) order.push(i);
  for (var i2 = order.length - 1; i2 > 0; i2--) {
    var j = Math.floor(Math.random() * (i2 + 1));
    var t = order[i2]; order[i2] = order[j]; order[j] = t;
  }
  return order;
}

function toggleShuffle() {
  state.shuffle = !state.shuffle;
  localStorage.setItem('moidify_shuffle', state.shuffle);
  if (state.shuffle && state.queue.length) {
    state.shuffleOrder = generateShuffleOrder(state.queue.length);
    state.shuffleIndex = 0;
    for (var si = 0; si < state.shuffleOrder.length; si++) {
      if (state.shuffleOrder[si] === state.currentIndex) { state.shuffleIndex = si; break; }
    }
  } else {
    state.shuffleOrder = [];
    state.shuffleIndex = 0;
  }
  renderRepeatShuffleButtons();
  renderQueuePanel();
}

function cycleRepeat() {
  var modes = ['off', 'all', 'one'];
  var idx = modes.indexOf(state.repeatMode);
  if (idx < 0) idx = 0;
  state.repeatMode = modes[(idx + 1) % modes.length];
  localStorage.setItem('moidify_repeat', state.repeatMode);
  renderRepeatShuffleButtons();
}

function renderRepeatShuffleButtons() {
  var shuffleBtn = document.getElementById('shuffle-btn');
  var repeatBtn = document.getElementById('repeat-btn');
  if (shuffleBtn) {
    shuffleBtn.classList.toggle('active', state.shuffle);
    shuffleBtn.title = state.shuffle ? 'Disable Shuffle' : 'Enable Shuffle';
  }
  if (repeatBtn) {
    var isOn = state.repeatMode !== 'off';
    repeatBtn.classList.toggle('active', isOn);
    repeatBtn.dataset.mode = state.repeatMode;
    var icons = { off: iconRepeat(), all: iconRepeat(), one: iconRepeat(), once: iconRepeat() };
    repeatBtn.innerHTML = icons[state.repeatMode] || iconRepeat();
    var titles = { off:'Repeat: Off', all:'Repeat: All', one:'Repeat: One' };
    repeatBtn.title = titles[state.repeatMode] || 'Repeat: Off';
  }
}

function setupQueueDropZone() {
  var panel = document.getElementById('queue-panel');
  if (!panel) return;
  panel.addEventListener('dragover', function(e) { e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; panel.classList.add('drag-over'); });
  panel.addEventListener('dragleave', function() { panel.classList.remove('drag-over'); });
  panel.addEventListener('drop', function(e) {
    e.preventDefault();
    panel.classList.remove('drag-over');
    var trackId = e.dataTransfer.getData('text/plain');
    if (!trackId) return;
    // Look for a track row with this ID in the DOM
    var row = document.querySelector('[data-track-id="'+trackId+'"]');
    if (row) {
      addTrackToQueueEnd({
        id: parseInt(trackId),
        title: (row.querySelector('.track-title') || {}).textContent || 'Unknown',
        artist: (row.querySelector('.track-artist') || {}).textContent || 'Unknown',
        album: (row.querySelector('.track-album') || {}).textContent || '',
        duration: parseFloat(row.dataset.trackDuration) || 0,
      });
      showToast('Added to queue', 'success');
    } else {
      // If no DOM row found, try fetching from API
      fetch('/api/tracks/' + trackId).then(function(r) { return r.json(); }).then(function(t) {
        addTrackToQueueEnd(t);
        showToast('Added to queue', 'success');
      }).catch(function() {});
    }
  });
}
