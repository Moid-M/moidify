import os
import time
import hashlib
import threading
from pathlib import Path

from mutagen import File
from mutagen.mp3 import MP3
from mutagen.flac import FLAC
from mutagen.oggvorbis import OggVorbis
from mutagen.mp4 import MP4
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler

from database import get_connection
from config import MUSIC_DIR, COVERS_DIR

AUDIO_EXTENSIONS = {'.mp3', '.flac', '.ogg', '.m4a', '.wav', '.wma', '.aac'}

# Scanner tracking (with lock for thread safety)
SCAN_STATUS = {"last_scan": None, "files_found": 0, "files_imported": 0, "errors": []}
SCAN_LOCK = threading.Lock()



def extract_metadata(file_path):
    try:
        audio = File(file_path)
        if audio is None:
            return None

        info = {
            'title': None,
            'artist': None,
            'album': None,
            'track_number': None,
            'genre': None,
            'year': None,
            'duration': None,
            'cover_data': None,
        }

        try:
            info['duration'] = int(audio.info.length)
        except Exception:
            pass

        if isinstance(audio, MP3):
            tags = audio.tags
            if tags:
                info['title'] = str(tags.get('TIT2', '') or '')
                info['artist'] = str(tags.get('TPE1', '') or '')
                info['album'] = str(tags.get('TALB', '') or '')
                info['track_number'] = str(tags.get('TRCK', '') or '')
                info['genre'] = str(tags.get('TCON', '') or '')
                info['year'] = str(tags.get('TDRC', '') or '')
                if 'APIC:' in tags:
                    info['cover_data'] = tags['APIC:'].data
                elif 'APIC' in tags:
                    info['cover_data'] = tags['APIC'].data

        elif isinstance(audio, FLAC):
            info['title'] = audio.get('title', [''])[0]
            info['artist'] = audio.get('artist', [''])[0]
            info['album'] = audio.get('album', [''])[0]
            info['track_number'] = audio.get('tracknumber', [''])[0]
            info['genre'] = audio.get('genre', [''])[0]
            info['year'] = audio.get('date', [''])[0]
            if audio.pictures:
                info['cover_data'] = audio.pictures[0].data

        elif isinstance(audio, OggVorbis):
            info['title'] = audio.get('title', [''])[0]
            info['artist'] = audio.get('artist', [''])[0]
            info['album'] = audio.get('album', [''])[0]
            info['track_number'] = audio.get('tracknumber', [''])[0]
            info['genre'] = audio.get('genre', [''])[0]
            info['year'] = audio.get('date', [''])[0]
            if hasattr(audio, 'pictures') and audio.pictures:
                info['cover_data'] = audio.pictures[0].data

        elif isinstance(audio, MP4):
            tags = audio.tags
            if tags:
                info['title'] = tags.get('\xa9nam', [''])[0]
                info['artist'] = tags.get('\xa9ART', [''])[0]
                info['album'] = tags.get('\xa9alb', [''])[0]
                info['track_number'] = tags.get('trkn', [(0, 0)])[0][0]
                info['genre'] = tags.get('\xa9gen', [''])[0]
                info['year'] = tags.get('\xa9day', [''])[0]
                if 'covr' in tags:
                    info['cover_data'] = tags['covr'][0]

        for key in ['title', 'artist', 'album', 'genre', 'year', 'track_number']:
            val = info[key]
            if isinstance(val, bytes):
                val = val.decode('utf-8', errors='replace')
            if val == '' or val is None:
                info[key] = None
            else:
                info[key] = str(val).strip()

        if not info['title']:
            info['title'] = Path(file_path).stem

        if info['track_number']:
            tn = str(info['track_number'])
            if '/' in tn:
                tn = tn.split('/')[0]
            try:
                info['track_number'] = int(tn)
            except ValueError:
                info['track_number'] = None

        if info['year']:
            y = str(info['year'])
            if len(y) == 4 and y.isdigit():
                info['year'] = int(y)
            else:
                info['year'] = None

        return info

    except Exception as e:
        return None


def get_file_hash(file_path):
    h = hashlib.sha256()
    stat = os.stat(file_path)
    size = stat.st_size
    with open(file_path, 'rb') as f:
        if size > 131072:
            h.update(f.read(65536))
            f.seek(-65536, 2)
            h.update(f.read(65536))
        else:
            h.update(f.read())
    h.update(str(size).encode())
    return h.hexdigest()


def process_file(file_path):
    path = Path(file_path)
    if path.suffix.lower() not in AUDIO_EXTENSIONS:
        return

    conn = get_connection()

    existing = conn.execute(
        "SELECT id FROM tracks WHERE file_path = ?", (str(path),)
    ).fetchone()
    if existing:
        conn.close()
        return

    metadata = extract_metadata(str(path))
    if metadata is None:
        with SCAN_LOCK:
            SCAN_STATUS["errors"].append(f"Failed to parse: {path.name}")
        conn.close()
        return

    file_hash = get_file_hash(str(path))

    cursor = conn.execute(
        """INSERT INTO tracks
        (file_path, file_hash, title, artist, album, track_number, genre, year, duration, has_cover)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
        (
            str(path),
            file_hash,
            metadata['title'],
            metadata['artist'],
            metadata['album'],
            metadata['track_number'],
            metadata['genre'],
            metadata['year'],
            metadata['duration'],
            1 if metadata['cover_data'] else 0,
        ),
    )
    track_id = cursor.lastrowid

    if metadata['cover_data']:
        cover_data = metadata['cover_data']
        if isinstance(cover_data, bytes):
            if cover_data[:4] == b'\x89PNG':
                ext = '.png'
            elif cover_data[:2] == b'\xff\xd8':
                ext = '.jpg'
            elif cover_data[:4] == b'RIFF' and cover_data[8:12] == b'WEBP':
                ext = '.webp'
            else:
                ext = '.jpg'
            cover_path = COVERS_DIR / f"{track_id}{ext}"
            with open(cover_path, 'wb') as f:
                f.write(cover_data)

    conn.commit()
    conn.close()



def scan_existing():
    global SCAN_STATUS
    if not MUSIC_DIR.exists():
        with SCAN_LOCK:
            SCAN_STATUS["last_scan"] = time.strftime('%Y-%m-%d %H:%M:%S')
            SCAN_STATUS["errors"].append("Music directory does not exist")
        return
    with SCAN_LOCK:
        SCAN_STATUS["errors"] = []
        SCAN_STATUS["files_found"] = 0
        SCAN_STATUS["files_imported"] = 0
    for root, _, files in os.walk(str(MUSIC_DIR)):
        for f in files:
            if Path(f).suffix.lower() not in AUDIO_EXTENSIONS:
                continue
            with SCAN_LOCK:
                SCAN_STATUS["files_found"] += 1
                was = len(SCAN_STATUS["errors"])
            process_file(os.path.join(root, f))
            with SCAN_LOCK:
                if len(SCAN_STATUS["errors"]) == was:
                    SCAN_STATUS["files_imported"] += 1
    with SCAN_LOCK:
        SCAN_STATUS["last_scan"] = time.strftime('%Y-%m-%d %H:%M:%S')


class MusicFileHandler(FileSystemEventHandler):
    def on_created(self, event):
        if event.is_directory:
            return
        if Path(event.src_path).suffix.lower() not in AUDIO_EXTENSIONS:
            return
        time.sleep(2)
        with SCAN_LOCK:
            SCAN_STATUS["files_found"] += 1
            was = len(SCAN_STATUS["errors"])
        process_file(event.src_path)
        with SCAN_LOCK:
            if len(SCAN_STATUS["errors"]) == was:
                SCAN_STATUS["files_imported"] += 1
            SCAN_STATUS["last_scan"] = time.strftime('%Y-%m-%d %H:%M:%S')

    def on_moved(self, event):
        if event.is_directory:
            return
        if Path(event.dest_path).suffix.lower() not in AUDIO_EXTENSIONS:
            return
        with SCAN_LOCK:
            SCAN_STATUS["files_found"] += 1
            was = len(SCAN_STATUS["errors"])
        process_file(event.dest_path)
        with SCAN_LOCK:
            if len(SCAN_STATUS["errors"]) == was:
                SCAN_STATUS["files_imported"] += 1
            SCAN_STATUS["last_scan"] = time.strftime('%Y-%m-%d %H:%M:%S')



def get_scan_status():
    with SCAN_LOCK:
        return dict(SCAN_STATUS)


def start_watcher():
    observer = Observer()
    handler = MusicFileHandler()
    observer.schedule(handler, str(MUSIC_DIR), recursive=True)
    observer.start()
    return observer
