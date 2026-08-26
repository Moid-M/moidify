import os
import time
import re
import hashlib
import logging
import threading
from pathlib import Path

from mutagen import File
from mutagen.mp3 import MP3
from mutagen.flac import FLAC
from mutagen.oggvorbis import OggVorbis
from mutagen.oggopus import OggOpus
from mutagen.mp4 import MP4, MP4Cover
from mutagen.id3 import ID3, TIT2, TPE1, TPE2, TALB, TCON, TDRC, TRCK, TPOS, USLT, APIC
from mutagen.flac import Picture as FlacPicture
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler

from database import get_connection
from config import MUSIC_DIR, COVERS_DIR

logger = logging.getLogger(__name__)

AUDIO_EXTENSIONS = {'.mp3', '.flac', '.ogg', '.m4a', '.wav', '.wma', '.aac', '.opus'}

SCAN_STATUS = {"last_scan": None, "files_found": 0, "files_imported": 0, "errors": []}
SCAN_LOCK = threading.Lock()
_SCAN_RUNNING = False
_SCAN_RUNNING_LOCK = threading.Lock()
_MAX_ERRORS = 100



def extract_metadata(file_path):
    try:
        audio = File(file_path)
        if audio is None:
            return None

        info = {
            'title': None,
            'artist': None,
            'album_artist': None,
            'album': None,
            'track_number': None,
            'disc_number': None,
            'genre': None,
            'year': None,
            'duration': None,
            'cover_data': None,
            'lyrics': None,
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
                info['album_artist'] = str(tags.get('TPE2', '') or '')
                info['album'] = str(tags.get('TALB', '') or '')
                info['track_number'] = str(tags.get('TRCK', '') or '')
                info['disc_number'] = str(tags.get('TPOS', '') or '')
                info['genre'] = str(tags.get('TCON', '') or '')
                info['year'] = str(tags.get('TDRC', '') or '')
                if 'APIC:' in tags or 'APIC' in tags:
                    apic = tags.getall('APIC')
                    if apic and apic[0].data:
                        info['cover_data'] = apic[0].data
                for uslt in tags.getall('USLT'):
                    if uslt and uslt.text:
                        info['lyrics'] = str(uslt.text)
                        break

        elif isinstance(audio, FLAC):
            info['title'] = audio.get('title', [''])[0]
            info['artist'] = audio.get('artist', [''])[0]
            info['album_artist'] = audio.get('albumartist', [''])[0]
            if not info['album_artist']:
                info['album_artist'] = audio.get('album_artist', [''])[0]
            info['album'] = audio.get('album', [''])[0]
            info['track_number'] = audio.get('tracknumber', [''])[0]
            info['disc_number'] = audio.get('discnumber', [''])[0]
            if not info['disc_number']:
                info['disc_number'] = audio.get('disc', [''])[0]
            info['genre'] = audio.get('genre', [''])[0]
            info['year'] = audio.get('date', [''])[0]
            info['lyrics'] = audio.get('lyrics', [''])[0]
            if not info['lyrics']:
                info['lyrics'] = audio.get('unsyncedlyrics', [''])[0]
            if audio.pictures:
                info['cover_data'] = audio.pictures[0].data

        elif isinstance(audio, (OggVorbis, OggOpus)):
            info['title'] = audio.get('title', [''])[0]
            info['artist'] = audio.get('artist', [''])[0]
            info['album_artist'] = audio.get('albumartist', [''])[0]
            if not info['album_artist']:
                info['album_artist'] = audio.get('album_artist', [''])[0]
            info['album'] = audio.get('album', [''])[0]
            info['track_number'] = audio.get('tracknumber', [''])[0]
            info['disc_number'] = audio.get('discnumber', [''])[0]
            info['genre'] = audio.get('genre', [''])[0]
            info['year'] = audio.get('date', [''])[0]
            info['lyrics'] = audio.get('lyrics', [''])[0]
            if hasattr(audio, 'pictures') and audio.pictures:
                info['cover_data'] = audio.pictures[0].data

        elif isinstance(audio, MP4):
            tags = audio.tags
            if tags:
                info['title'] = tags.get('\xa9nam', [''])[0]
                info['artist'] = tags.get('\xa9ART', [''])[0]
                info['album_artist'] = tags.get('aART', [''])[0]
                info['album'] = tags.get('\xa9alb', [''])[0]
                info['track_number'] = tags.get('trkn', [(0, 0)])[0][0]
                info['disc_number'] = tags.get('disk', [(0, 0)])[0][0]
                info['genre'] = tags.get('\xa9gen', [''])[0]
                info['year'] = tags.get('\xa9day', [''])[0]
                if 'covr' in tags:
                    info['cover_data'] = tags['covr'][0]
                info['lyrics'] = tags.get('\xa9lyr', [''])[0]

        for key in ['title', 'artist', 'album_artist', 'album', 'genre', 'year', 'track_number', 'disc_number']:
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

        if info['disc_number']:
            dn = str(info['disc_number'])
            if '/' in dn:
                dn = dn.split('/')[0]
            try:
                info['disc_number'] = int(dn)
            except ValueError:
                info['disc_number'] = None

        if info['year']:
            m = re.search(r'\b(\d{4})\b', str(info['year']))
            if m:
                info['year'] = int(m.group(1))
            else:
                info['year'] = None

        if not info['lyrics']:
            lrc_path = Path(file_path).with_suffix('.lrc')
            if lrc_path.is_file():
                try:
                    info['lyrics'] = lrc_path.read_text(encoding='utf-8', errors='replace')
                except Exception as e:
                    logger.warning("Failed to read companion .lrc file %s: %s", lrc_path, e)

        return info

    except Exception as e:
        logger.warning("Failed to extract metadata from %s: %s", file_path, e)
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


def _save_cover(cover_data):
    if not isinstance(cover_data, bytes):
        cover_data = bytes(cover_data)
    if cover_data[:4] == b'\x89PNG':
        ext = '.png'
    elif cover_data[:2] == b'\xff\xd8':
        ext = '.jpg'
    elif cover_data[:4] == b'RIFF' and cover_data[8:12] == b'WEBP':
        ext = '.webp'
    else:
        ext = '.jpg'
    cover_hash = hashlib.sha256(cover_data).hexdigest()[:16]
    cover_path = COVERS_DIR / f"{cover_hash}{ext}"
    if not cover_path.exists():
        with open(cover_path, 'wb') as f:
            f.write(cover_data)
    return cover_hash


def _img_mime(data):
    if data[:4] == b'\x89PNG':
        return 'image/png'
    if data[:2] == b'\xff\xd8':
        return 'image/jpeg'
    if data[:4] == b'RIFF' and data[8:12] == b'WEBP':
        return 'image/webp'
    return 'image/jpeg'


def _img_ext(data):
    m = _img_mime(data)
    return '.png' if m == 'image/png' else ('.webp' if m == 'image/webp' else '.jpg')


def write_metadata_to_file(file_path, fields):
    """Write a subset of metadata fields into an audio file's tags.

    fields keys: title, artist, album_artist, album, genre, year,
                 track_number, disc_number, lyrics, cover_data (bytes).
    Returns True if tags were written, False if the format has no tag support
    (e.g. WAV). Raises on unexpected tag-write errors (caller should catch).
    """
    audio = File(file_path)
    if audio is None:
        return False

    def _num(v):
        try:
            return int(v)
        except (TypeError, ValueError):
            return None

    if isinstance(audio, MP3):
        try:
            tags = audio.tags or ID3()
        except Exception:
            tags = ID3()
        if 'lyrics' in fields:
            tags.delall('USLT')
            if fields['lyrics']:
                tags.add(USLT(encoding=3, lang='eng', desc='', text=fields['lyrics']))
        if 'title' in fields: tags['TIT2'] = TIT2(encoding=3, text=fields['title'] or '')
        if 'artist' in fields: tags['TPE1'] = TPE1(encoding=3, text=fields['artist'] or '')
        if 'album_artist' in fields: tags['TPE2'] = TPE2(encoding=3, text=fields['album_artist'] or '')
        if 'album' in fields: tags['TALB'] = TALB(encoding=3, text=fields['album'] or '')
        if 'genre' in fields: tags['TCON'] = TCON(encoding=3, text=fields['genre'] or '')
        if 'year' in fields: tags['TDRC'] = TDRC(encoding=3, text=str(fields['year'] or ''))
        if 'track_number' in fields:
            tn = _num(fields['track_number'])
            tags['TRCK'] = TRCK(encoding=3, text=str(tn) if tn is not None else '')
        if 'disc_number' in fields:
            dn = _num(fields['disc_number'])
            tags['TPOS'] = TPOS(encoding=3, text=str(dn) if dn is not None else '')
        if 'cover_data' in fields and fields['cover_data']:
            tags['APIC'] = APIC(encoding=3, mime=_img_mime(fields['cover_data']),
                                type=3, desc='Cover', data=fields['cover_data'])
        audio.tags = tags
        audio.save()
        return True

    if isinstance(audio, (FLAC, OggVorbis, OggOpus)):
        if 'lyrics' in fields:
            if fields['lyrics']:
                audio['lyrics'] = [fields['lyrics']]
            else:
                audio.pop('lyrics', None)
        if 'title' in fields: audio['title'] = [fields['title'] or '']
        if 'artist' in fields: audio['artist'] = [fields['artist'] or '']
        if 'album_artist' in fields: audio['albumartist'] = [fields['album_artist'] or '']
        if 'album' in fields: audio['album'] = [fields['album'] or '']
        if 'genre' in fields: audio['genre'] = [fields['genre'] or '']
        if 'year' in fields: audio['date'] = [str(fields['year'] or '')]
        if 'track_number' in fields:
            tn = _num(fields['track_number'])
            audio['tracknumber'] = [str(tn)] if tn is not None else ['']
        if 'disc_number' in fields:
            dn = _num(fields['disc_number'])
            audio['discnumber'] = [str(dn)] if dn is not None else ['']
        if 'cover_data' in fields and fields['cover_data']:
            try:
                pic = FlacPicture()
                pic.data = fields['cover_data']
                pic.type = 3
                pic.mime = _img_mime(fields['cover_data'])
                pic.desc = 'Cover'
                audio.clear_pictures()
                audio.add_picture(pic)
            except Exception:
                pass
        audio.save()
        return True

    if isinstance(audio, MP4):
        tags = audio.tags or {}
        if 'lyrics' in fields: tags['\xa9lyr'] = fields['lyrics'] or ''
        if 'title' in fields: tags['\xa9nam'] = fields['title'] or ''
        if 'artist' in fields: tags['\xa9ART'] = fields['artist'] or ''
        if 'album_artist' in fields: tags['aART'] = fields['album_artist'] or ''
        if 'album' in fields: tags['\xa9alb'] = fields['album'] or ''
        if 'genre' in fields: tags['\xa9gen'] = fields['genre'] or ''
        if 'year' in fields: tags['\xa9day'] = str(fields['year'] or '')
        if 'track_number' in fields:
            tn = _num(fields['track_number'])
            tags['trkn'] = [(tn, 0)] if tn is not None else [(0, 0)]
        if 'disc_number' in fields:
            dn = _num(fields['disc_number'])
            tags['disk'] = [(dn, 0)] if dn is not None else [(0, 0)]
        if 'cover_data' in fields and fields['cover_data']:
            flag = MP4Cover.FORMAT_PNG if _img_mime(fields['cover_data']) == 'image/png' else MP4Cover.FORMAT_JPEG
            tags['covr'] = [MP4Cover(fields['cover_data'], flag)]
        audio.tags = tags
        audio.save()
        return True

    return False


def process_file(file_path, conn=None, force=False):
    path = Path(file_path)
    if path.suffix.lower() not in AUDIO_EXTENSIONS:
        return

    close_conn = False
    if conn is None:
        conn = get_connection()
        close_conn = True

    try:
        existing = conn.execute(
            "SELECT id FROM tracks WHERE file_path = ?", (str(path),)
        ).fetchone()
        if existing and not force:
            return

        metadata = extract_metadata(str(path))
        if metadata is None:
            metadata = {
                'title': path.stem,
                'artist': 'Unknown',
                'album_artist': None,
                'album': 'Unknown',
                'track_number': None,
                'disc_number': None,
                'genre': None,
                'year': None,
                'duration': 0,
                'cover_data': None,
                'lyrics': None,
            }

        try:
            file_hash = get_file_hash(str(path))
        except Exception:
            with SCAN_LOCK:
                SCAN_STATUS["errors"].append(f"Failed to hash: {path.name}")
                if len(SCAN_STATUS["errors"]) > _MAX_ERRORS:
                    SCAN_STATUS["errors"] = SCAN_STATUS["errors"][-_MAX_ERRORS:]
            return

        cover_hash = None
        if metadata['cover_data']:
            cover_hash = _save_cover(metadata['cover_data'])

        if existing and force:
            conn.execute(
                """UPDATE tracks SET
                file_hash=?, title=?, artist=?, album_artist=?, album=?, track_number=?,
                disc_number=?, genre=?, year=?, duration=?, has_cover=?, lyrics=?, cover_hash=?
                WHERE file_path=?""",
                (
                    file_hash,
                    metadata['title'],
                    metadata['artist'],
                    metadata['album_artist'],
                    metadata['album'],
                    metadata['track_number'],
                    metadata['disc_number'],
                    metadata['genre'],
                    metadata['year'],
                    metadata['duration'],
                    1 if metadata['cover_data'] else 0,
                    metadata['lyrics'],
                    cover_hash,
                    str(path),
                ),
            )
        else:
            cursor = conn.execute(
                """INSERT OR IGNORE INTO tracks
                (file_path, file_hash, title, artist, album_artist, album, track_number, disc_number, genre, year, duration, has_cover, lyrics, cover_hash)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
                (
                    str(path),
                    file_hash,
                    metadata['title'],
                    metadata['artist'],
                    metadata['album_artist'],
                    metadata['album'],
                    metadata['track_number'],
                    metadata['disc_number'],
                    metadata['genre'],
                    metadata['year'],
                    metadata['duration'],
                    1 if metadata['cover_data'] else 0,
                    metadata['lyrics'],
                    cover_hash,
                ),
            )
            if cursor.rowcount == 0:
                return
    except Exception:
        with SCAN_LOCK:
            SCAN_STATUS["errors"].append(f"Error processing: {path.name}")
            if len(SCAN_STATUS["errors"]) > _MAX_ERRORS:
                SCAN_STATUS["errors"] = SCAN_STATUS["errors"][-_MAX_ERRORS:]
    finally:
        if close_conn:
            conn.commit()
            conn.close()



def scan_existing(clean=False):
    global SCAN_STATUS, _SCAN_RUNNING
    with _SCAN_RUNNING_LOCK:
        if _SCAN_RUNNING:
            return
        _SCAN_RUNNING = True
    try:
        if not MUSIC_DIR.exists():
            with SCAN_LOCK:
                SCAN_STATUS["last_scan"] = time.strftime('%Y-%m-%d %H:%M:%S')
                SCAN_STATUS["errors"].append("Music directory does not exist")
            return
        with SCAN_LOCK:
            SCAN_STATUS["errors"] = []
            SCAN_STATUS["files_found"] = 0
            SCAN_STATUS["files_imported"] = 0
        conn = get_connection()
        count = 0

        if clean:
            conn.execute("DELETE FROM playlist_tracks")
            conn.execute("DELETE FROM favorites")
            conn.execute("DELETE FROM tracks")
            if COVERS_DIR.exists():
                for cf in COVERS_DIR.iterdir():
                    try:
                        cf.unlink()
                    except Exception:
                        pass
            conn.commit()
            SCAN_STATUS["files_imported"] = 0

        try:
            for root, _, files in os.walk(str(MUSIC_DIR)):
                for f in files:
                    if Path(f).suffix.lower() not in AUDIO_EXTENSIONS:
                        continue
                    with SCAN_LOCK:
                        SCAN_STATUS["files_found"] += 1
                    was = len(SCAN_STATUS["errors"])
                    process_file(os.path.join(root, f), conn=conn, force=clean)
                    with SCAN_LOCK:
                        if len(SCAN_STATUS["errors"]) == was:
                            SCAN_STATUS["files_imported"] += 1
                    count += 1
                    if count % 50 == 0:
                        conn.commit()
            conn.commit()
        except Exception as e:
            with SCAN_LOCK:
                SCAN_STATUS["errors"].append(f"Scan error: {e}")
        finally:
            conn.close()
        with SCAN_LOCK:
            SCAN_STATUS["last_scan"] = time.strftime('%Y-%m-%d %H:%M:%S')
    finally:
        with _SCAN_RUNNING_LOCK:
            _SCAN_RUNNING = False


class MusicFileHandler(FileSystemEventHandler):
    def _is_scanning(self):
        with _SCAN_RUNNING_LOCK:
            return _SCAN_RUNNING

    def on_created(self, event):
        if self._is_scanning():
            return
        if event.is_directory:
            return
        ext = Path(event.src_path).suffix.lower()
        if ext == '.lrc':
            audio_path = Path(event.src_path).with_suffix('')
            for ae in AUDIO_EXTENSIONS:
                ap = audio_path.with_suffix(ae)
                if ap.is_file():
                    time.sleep(1)
                    process_file(str(ap))
                    break
            return
        if ext not in AUDIO_EXTENSIONS:
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

    def on_deleted(self, event):
        if self._is_scanning():
            return
        if event.is_directory:
            return
        if Path(event.src_path).suffix.lower() not in AUDIO_EXTENSIONS:
            return
        conn = get_connection()
        conn.execute("DELETE FROM tracks WHERE file_path = ?", (str(event.src_path),))
        conn.commit()
        conn.close()

    def on_moved(self, event):
        if self._is_scanning():
            return
        if event.is_directory:
            return
        src_ext = Path(event.src_path).suffix.lower()
        dst_ext = Path(event.dest_path).suffix.lower()
        src_is_audio = src_ext in AUDIO_EXTENSIONS
        dst_is_audio = dst_ext in AUDIO_EXTENSIONS
        if src_ext == '.lrc' or dst_ext == '.lrc':
            audio_path = Path(event.dest_path if dst_ext == '.lrc' else event.src_path).with_suffix('')
            for ae in AUDIO_EXTENSIONS:
                ap = audio_path.with_suffix(ae)
                if ap.is_file():
                    process_file(str(ap))
                    break
            return
        if not dst_is_audio and not src_is_audio:
            return
        if src_is_audio:
            conn = get_connection()
            conn.execute("DELETE FROM tracks WHERE file_path = ?", (str(event.src_path),))
            conn.commit()
            conn.close()
        if dst_is_audio:
            with SCAN_LOCK:
                SCAN_STATUS["files_found"] += 1
                was = len(SCAN_STATUS["errors"])
            process_file(event.dest_path)
            with SCAN_LOCK:
                if len(SCAN_STATUS["errors"]) == was:
                    SCAN_STATUS["files_imported"] += 1
                SCAN_STATUS["last_scan"] = time.strftime('%Y-%m-%d %H:%M:%S')



def get_scan_status():
    with _SCAN_RUNNING_LOCK:
        running = _SCAN_RUNNING
    with SCAN_LOCK:
        return {
            "running": running,
            "last_scan": SCAN_STATUS["last_scan"],
            "files_found": SCAN_STATUS["files_found"],
            "files_imported": SCAN_STATUS["files_imported"],
            "errors": list(SCAN_STATUS["errors"]),
        }


def start_watcher():
    observer = Observer()
    handler = MusicFileHandler()
    observer.schedule(handler, str(MUSIC_DIR), recursive=True)
    observer.start()
    return observer
