function showToast(msg, type) {
  type = type || 'info';
  var c = document.getElementById('toast-container');
  // Limit visible toasts to 3
  var existing = c.querySelectorAll('.toast:not(.toast-out)');
  if (existing.length >= 3) {
    var oldest = existing[0];
    oldest.classList.add('toast-out');
    setTimeout(function() { if (oldest.parentNode) oldest.parentNode.removeChild(oldest); }, 300);
  }
  var t = document.createElement('div');
  t.className = 'toast toast-' + type;
  t.textContent = msg;
  t.addEventListener('click', function() {
    this.classList.add('toast-out');
    setTimeout(function() { if (t.parentNode) t.parentNode.removeChild(t); }, 300);
  });
  c.appendChild(t);
  var duration = type === 'error' ? 5000 : 3000;
  setTimeout(function() {
    t.classList.add('toast-out');
    setTimeout(function() { if (t.parentNode) t.parentNode.removeChild(t); }, 300);
  }, duration);
}
