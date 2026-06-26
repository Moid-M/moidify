var _langData = null;
var _lang = null;

var _cachedTranslations = null;
try {
  _cachedTranslations = JSON.parse(localStorage.getItem('moidify_i18n_cache'));
} catch(e) {}

function getLanguage() {
  return _lang || localStorage.getItem('moidify_lang') || navigator.language.slice(0, 2) || 'en';
}

function setLanguage(code) {
  _lang = code;
  localStorage.setItem('moidify_lang', code);
  return applyLanguage();
}

function loadTranslation(code) {
  return fetch('/static/lang/' + code + '.json')
    .then(function(r) {
      if (!r.ok) throw new Error('not found');
      return r.json();
    })
    .catch(function() {
      return fetch('/static/lang/en.json').then(function(r) {
        if (!r.ok) throw new Error('fallback not found');
        return r.json();
      });
    });
}

function applyLanguage() {
  _lang = getLanguage();
  var needsBoth = _lang !== 'en';
  var loadMain = loadTranslation(_lang);
  var loadFallback = needsBoth ? loadTranslation('en') : Promise.resolve(null);

  return Promise.all([loadMain, loadFallback]).then(function(results) {
    _langData = results[0];
    if (needsBoth && results[1]) {
      _langData._fallback = results[1];
    }
    try {
      localStorage.setItem('moidify_i18n_cache', JSON.stringify(_langData));
    } catch(e) {}
    document.documentElement.lang = _lang;
    translateDOM();
  });
}

function _(key, vars) {
  var data = _langData || _cachedTranslations;
  if (!data) return key;

  var val = data;
  var parts = key.split('.');
  for (var i = 0; i < parts.length; i++) {
    if (val == null) {
      if (data._fallback) {
        val = data._fallback;
        for (var j = 0; j < parts.length; j++) {
          if (val == null) return key;
          val = val[parts[j]];
          if (val == null) return key;
        }
      } else {
        return key;
      }
      break;
    }
    val = val[parts[i]];
  }
  if (typeof val !== 'string') {
    if (!data._fallback) return key;
    val = data._fallback;
    for (var j = 0; j < parts.length; j++) {
      if (val == null) return key;
      val = val[parts[j]];
    }
    if (typeof val !== 'string') return key;
  }
  if (vars) {
    for (var k in vars) {
      val = val.replace('{' + k + '}', vars[k]);
    }
  }
  return val;
}

function translateDOM(root) {
  root = root || document;
  qsa('[data-i18n]', root).forEach(function(el) {
    var key = el.dataset.i18n;
    var val = _(key);
    if (val !== key) {
      if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {
        el.placeholder = val;
      } else {
        el.textContent = val;
      }
    }
  });
  qsa('[data-i18n-title]', root).forEach(function(el) {
    var key = el.dataset.i18nTitle;
    var val = _(key);
    if (val !== key) el.title = val;
  });
}

var TRANSLATIONS = null;

// Load translations immediately — cached copy already available via localStorage
applyLanguage();
