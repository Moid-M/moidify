var sleepTimerInterval = null;

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
    } else if (value === -2) {
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
