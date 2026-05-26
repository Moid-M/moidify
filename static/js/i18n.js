var _langData = null;
var _lang = null;

function getLanguage() {
  return _lang || localStorage.getItem('moidify_lang') || navigator.language.slice(0, 2) || 'en';
}

function setLanguage(code) {
  _lang = code;
  localStorage.setItem('moidify_lang', code);
  applyLanguage();
}

function applyLanguage() {
  _lang = getLanguage();
  _langData = TRANSLATIONS[_lang] || TRANSLATIONS.en;
  document.documentElement.lang = _lang;
  translateDOM();
}

function _(key, vars) {
  if (!_langData) {
    _lang = getLanguage();
    _langData = TRANSLATIONS[_lang] || TRANSLATIONS.en;
  }
  var val = _langData;
  var parts = key.split('.');
  for (var i = 0; i < parts.length; i++) {
    if (val == null) return key;
    val = val[parts[i]];
    if (val == null) return key;
  }
  if (typeof val !== 'string') return key;
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

var TRANSLATIONS = {
  en: {
    nav: {
      tracks: 'All Tracks',
      albums: 'Albums',
      artists: 'Artists',
      genres: 'Genres',
      favorites: 'Liked Songs',
      playlists: 'Your Playlists',
    },
    search: {
      placeholder: 'Search tracks, artists, albums...',
      results: 'Search: "{q}"',
      noResults: 'No results for "{q}"',
      noResultsHint: 'Try a different search term',
      resultsCount: '{n} result',
      resultsCount_plural: '{n} results',
      albums: 'Albums',
      artists: 'Artists',
      tracks: 'Tracks',
      loading: 'Searching...',
    },
    player: {
      play: 'Play',
      pause: 'Pause',
      previous: 'Previous',
      next: 'Next',
      shuffle: 'Shuffle',
      repeat_off: 'Repeat: Off',
      repeat_one: 'Repeat: One',
      repeat_all: 'Repeat: All',
      mute: 'Mute',
      lyrics: 'Lyrics',
      queue: 'Queue',
      eq: 'Equalizer',
      like: 'Like',
      rewind: '-10 seconds',
      forward: '+10 seconds',
      noTrack: 'No track playing',
    },
    playlist: {
      empty: 'This playlist is empty.',
      new: 'New Playlist',
      namePlaceholder: 'Playlist name',
      createAdd: 'Create & Add',
      share: 'Share playlist',
      copyLink: 'Copy link',
      revoke: 'Revoke',
      sharedWith: 'Shared by {user}',
      delete: 'Delete this playlist?',
      addTo: 'Add to Playlist',
      pin: 'Pin',
    },
    settings: {
      title: 'Settings',
      theme: 'Theme',
      animations: 'Animations',
      playback: 'Playback',
      about: 'About',
      language: 'Language',
      themeColor: 'Theme Color',
      appearance: 'Appearance',
      lightMode: 'Light Mode',
      autoTheme: 'Follow system theme',
      eq: 'Equalizer',
      streamQuality: 'Stream Quality',
      qualityOriginal: 'Original',
      qualityMedium: 'Medium (256k)',
      qualityLow: 'Low (128k)',
      qualityVoice: 'Voice (64k Opus)',
      qualityHint: 'Requires ffmpeg on the server for transcoding',
      crossfade: 'Crossfade',
      sleepTimer: 'Sleep Timer',
      timerActive: 'Timer is active ({label})',
      noTimer: 'No timer active',
      setTimer: 'Set Timer',
      changeTimer: 'Change Timer',
      cancel: 'Cancel',
      version: 'Version',
      database: 'Database',
      musicFolder: 'Music folder',
      admin: 'Admin',
      keyboardShortcuts: 'Keyboard Shortcuts',
    },
    auth: {
      login: 'Log In',
      register: 'Register',
      logout: 'Log Out',
      loginTitle: 'Log In',
      registerTitle: 'Register',
      username: 'Username',
      password: 'Password',
      confirmPassword: 'Confirm Password',
      email: 'Email (optional)',
      passwordHint: 'At least 3 characters',
      loginRequired: 'Login required',
    },
    tracks: {
      title: 'Title',
      artist: 'Artist',
      album: 'Album',
      duration: 'Duration',
      noTracks: 'No tracks yet.',
      noResults: 'No results found.',
      playAll: 'Play All',
      addQueue: 'Add to Queue',
      addPlaylist: 'Add to Playlist',
      download: 'Download',
      clear: 'Clear',
      selected: '{n} selected',
      sortAsc: 'A–Z',
      sortDesc: 'Z–A',
    },
    context: {
      play: 'Play',
      playNext: 'Play Next',
      addQueue: 'Add to Queue',
      like: 'Like',
      unlike: 'Remove from Liked Songs',
      goAlbum: 'Go to Album',
      goArtist: 'Go to Artist',
      download: 'Download Song',
      downloadAlbum: 'Download Album',
      removeQueue: 'Remove from Queue',
      trackInfo: 'Track Info',
      albumShuffle: 'Shuffle Album',
      artistShuffle: 'Shuffle Artist',
    },
    genres: {
      title: 'Genres',
      noGenres: 'No genre tags found in your music.',
      tracks: '{n} tracks',
      albums: '{n} albums',
    },
    favorites: {
      title: 'Liked Songs',
      empty: 'No liked songs yet. Click the heart on any track.',
      login: 'Log in to see your liked songs.',
    },
    albums: {
      title: 'Albums',
      noAlbums: 'Drop music into the music/ folder.',
      tracks: '{n} tracks',
    },
    artists: {
      title: 'Artists',
      noArtists: 'No artists found.',
      tracksAlbums: '{n} tracks, {m} albums',
    },
    common: {
      loading: 'Loading...',
      error: 'Error: {msg}',
      back: 'Back',
      cancel: 'Cancel',
      save: 'Save',
      delete: 'Delete',
      confirm: 'Confirm',
      search: 'Search',
      close: 'Close',
      track: 'Track',
      artist: 'Artist',
      album: 'Album',
      year: 'Year',
      number: '#',
    },
    trackInfo: {
      title: 'Track Info',
      filename: 'File',
      format: 'Format',
      bitrate: 'Bitrate',
      sampleRate: 'Sample Rate',
      channels: 'Channels',
      duration: 'Duration',
      playCount: 'Play Count',
      lastPlayed: 'Last Played',
      added: 'Added',
      filePath: 'File Path',
    },
  },

  de: {
    nav: {
      tracks: 'Alle Titel',
      albums: 'Alben',
      artists: 'Interpret:innen',
      genres: 'Genres',
      favorites: 'Lieblingslieder',
      playlists: 'Playlists',
    },
    search: {
      placeholder: 'Suche Titel, Interpret:innen, Alben...',
      results: 'Suche: "{q}"',
      noResults: 'Keine Ergebnisse für "{q}"',
      noResultsHint: 'Versuche einen anderen Suchbegriff',
      resultsCount: '{n} Ergebnis',
      resultsCount_plural: '{n} Ergebnisse',
      albums: 'Alben',
      artists: 'Interpret:innen',
      tracks: 'Titel',
      loading: 'Suche...',
    },
    player: {
      play: 'Abspielen',
      pause: 'Pause',
      previous: 'Vorheriger',
      next: 'Nächster',
      shuffle: 'Zufallswiedergabe',
      repeat_off: 'Wiederholung: Aus',
      repeat_one: 'Wiederholung: Eins',
      repeat_all: 'Wiederholung: Alle',
      mute: 'Stumm',
      lyrics: 'Liedtext',
      queue: 'Warteschlange',
      eq: 'Equalizer',
      like: 'Gefällt mir',
      rewind: '-10 Sekunden',
      forward: '+10 Sekunden',
      noTrack: 'Kein Titel läuft',
    },
    playlist: {
      empty: 'Diese Playlist ist leer.',
      new: 'Neue Playlist',
      namePlaceholder: 'Name der Playlist',
      createAdd: 'Erstellen & Hinzufügen',
      share: 'Playlist teilen',
      copyLink: 'Link kopieren',
      revoke: 'Widerrufen',
      sharedWith: 'Geteilt von {user}',
      delete: 'Diese Playlist löschen?',
      addTo: 'Zu Playlist hinzufügen',
      pin: 'Anheften',
    },
    settings: {
      title: 'Einstellungen',
      theme: 'Design',
      animations: 'Animationen',
      playback: 'Wiedergabe',
      about: 'Info',
      language: 'Sprache',
      themeColor: 'Akzentfarbe',
      appearance: 'Erscheinungsbild',
      lightMode: 'Helles Design',
      autoTheme: 'Systemdesign folgen',
      eq: 'Equalizer',
      streamQuality: 'Stream-Qualität',
      qualityOriginal: 'Original',
      qualityMedium: 'Mittel (256k)',
      qualityLow: 'Niedrig (128k)',
      qualityVoice: 'Sprache (64k Opus)',
      qualityHint: 'Erfordert ffmpeg auf dem Server',
      crossfade: 'Überblendung',
      sleepTimer: 'Schlaf-Timer',
      timerActive: 'Timer aktiv ({label})',
      noTimer: 'Kein Timer aktiv',
      setTimer: 'Timer setzen',
      changeTimer: 'Timer ändern',
      cancel: 'Abbrechen',
      version: 'Version',
      database: 'Datenbank',
      musicFolder: 'Musikordner',
      admin: 'Admin',
      keyboardShortcuts: 'Tastaturkürzel',
    },
    auth: {
      login: 'Anmelden',
      register: 'Registrieren',
      logout: 'Abmelden',
      loginTitle: 'Anmelden',
      registerTitle: 'Registrieren',
      username: 'Benutzername',
      password: 'Passwort',
      confirmPassword: 'Passwort bestätigen',
      email: 'E-Mail (optional)',
      passwordHint: 'Mindestens 3 Zeichen',
      loginRequired: 'Anmeldung erforderlich',
    },
    tracks: {
      title: 'Titel',
      artist: 'Interpret:in',
      album: 'Album',
      duration: 'Dauer',
      noTracks: 'Noch keine Titel.',
      noResults: 'Keine Ergebnisse gefunden.',
      playAll: 'Alle abspielen',
      addQueue: 'Zur Warteschlange',
      addPlaylist: 'Zur Playlist',
      download: 'Herunterladen',
      clear: 'Leeren',
      selected: '{n} ausgewählt',
    },
    context: {
      play: 'Abspielen',
      playNext: 'Als Nächstes',
      addQueue: 'Zur Warteschlange',
      like: 'Gefällt mir',
      unlike: 'Nicht mehr gefallen',
      goAlbum: 'Zum Album',
      goArtist: 'Zu Interpret:in',
      download: 'Titel herunterladen',
      downloadAlbum: 'Album herunterladen',
      removeQueue: 'Aus Warteschlange',
      trackInfo: 'Titelinfo',
      albumShuffle: 'Album mischen',
      artistShuffle: 'Interpret:in mischen',
    },
    genres: {
      title: 'Genres',
      noGenres: 'Keine Genres in deiner Musik gefunden.',
      tracks: '{n} Titel',
      albums: '{n} Alben',
    },
    favorites: {
      title: 'Lieblingslieder',
      empty: 'Noch keine Lieblingslieder. Klicke auf das Herz bei einem Titel.',
      login: 'Melde dich an, um deine Lieblingslieder zu sehen.',
    },
    albums: {
      title: 'Alben',
      noAlbums: 'Lege Musik in den music/ Ordner.',
      tracks: '{n} Titel',
    },
    artists: {
      title: 'Interpret:innen',
      noArtists: 'Keine Interpret:innen gefunden.',
      tracksAlbums: '{n} Titel, {m} Alben',
    },
    common: {
      loading: 'Laden...',
      error: 'Fehler: {msg}',
      back: 'Zurück',
      cancel: 'Abbrechen',
      save: 'Speichern',
      delete: 'Löschen',
      confirm: 'Bestätigen',
      search: 'Suchen',
      close: 'Schließen',
      track: 'Titel',
      artist: 'Interpret:in',
      album: 'Album',
      year: 'Jahr',
      number: '#',
    },
    trackInfo: {
      title: 'Titelinfo',
      filename: 'Datei',
      format: 'Format',
      bitrate: 'Bitrate',
      sampleRate: 'Abtastrate',
      channels: 'Kanäle',
      duration: 'Dauer',
      playCount: 'Wiedergaben',
      lastPlayed: 'Zuletzt gespielt',
      added: 'Hinzugefügt',
      filePath: 'Dateipfad',
    },
  },
};
