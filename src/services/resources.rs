//! Resource statistics from /proc — same output shape as Python
//! `_get_resource_stats()`. Python prefers psutil when installed; Rust has no
//! psutil, so this always uses the /proc fallback branch (same kernel
//! interfaces psutil reads underneath).

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

/// Python `_DISK_CACHE` for the music-dir walk (300 s TTL).
struct DiskCache {
    bytes: u64,
    by_format: Vec<(String, u64)>,
    stamp: Option<Instant>,
}
static DISK_CACHE: Mutex<DiskCache> = Mutex::new(DiskCache {
    bytes: 0,
    by_format: Vec::new(),
    stamp: None,
});

/// Python `AUDIO_EXTS` in admin.py (no `.opus`, unlike the scanner list).
const ADMIN_AUDIO_EXTS: [&str; 7] = ["mp3", "flac", "ogg", "m4a", "wav", "aac", "wma"];

/// Python `_walk_music_dir()` — total bytes + per-format bytes for audio
/// files under `music_dir` (recursive, like `os.walk`), cached for 300 s.
pub fn walk_music_dir(music_dir: &Path) -> (u64, Vec<(String, u64)>) {
    let mut cache = DISK_CACHE.lock().unwrap();
    if let Some(stamp) = cache.stamp {
        if stamp.elapsed().as_secs() < 300 {
            return (cache.bytes, cache.by_format.clone());
        }
    }
    let mut by_format: BTreeMap<String, u64> = BTreeMap::new();
    let total = walk_recursive(music_dir, &mut by_format);
    let by_format_vec: Vec<(String, u64)> = by_format.into_iter().collect();
    cache.bytes = total;
    cache.by_format = by_format_vec.clone();
    cache.stamp = Some(Instant::now());
    (total, by_format_vec)
}

fn walk_recursive(dir: &Path, by_format: &mut BTreeMap<String, u64>) -> u64 {
    let mut total = 0u64;
    let Ok(walk) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in walk.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += walk_recursive(&path, by_format);
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if !ADMIN_AUDIO_EXTS.contains(&ext.as_str()) {
            continue;
        }
        if let Ok(md) = entry.metadata() {
            total += md.len();
            *by_format.entry(ext.to_uppercase()).or_insert(0) += md.len();
        }
    }
    total
}

// ── /proc readers ────────────────────────────────────────────────────────────

/// Python `_read_proc_stat_cpu()` → (total, idle) jiffies since boot.
fn read_proc_stat_cpu() -> Option<(f64, f64)> {
    let text = std::fs::read_to_string("/proc/stat").ok()?;
    let line = text.lines().next()?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 5 {
        return None;
    }
    let vals: Vec<f64> = parts[1..]
        .iter()
        .filter_map(|v| v.parse::<f64>().ok())
        .collect();
    if vals.len() < 5 {
        return None;
    }
    let idle = vals[3] + vals[4];
    Some((vals.iter().sum(), idle))
}

/// Python `_read_proc_self_stat()` → (utime+stime, rss_pages, threads, starttime).
fn read_proc_self_stat() -> Option<(f64, u64, u64, f64)> {
    let text = std::fs::read_to_string("/proc/self/stat").ok()?;
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() < 24 {
        return None;
    }
    let utime: f64 = parts[13].parse().ok()?;
    let stime: f64 = parts[14].parse().ok()?;
    let rss: u64 = parts[23].parse().ok()?;
    let threads: u64 = parts[19].parse().ok()?;
    let starttime: f64 = parts[21].parse().ok()?;
    Some((utime + stime, rss, threads, starttime))
}

fn read_loadavg() -> Option<Vec<f64>> {
    let text = std::fs::read_to_string("/proc/loadavg").ok()?;
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    Some(
        parts[..3]
            .iter()
            .filter_map(|v| v.parse::<f64>().ok())
            .collect(),
    )
}

fn read_meminfo() -> BTreeMap<String, u64> {
    let mut info = BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string("/proc/meminfo") {
        for line in text.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if let Some(n) = v
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<u64>().ok())
                {
                    info.insert(k.trim().to_string(), n * 1024);
                }
            }
        }
    }
    info
}

fn read_uptime() -> Option<f64> {
    let text = std::fs::read_to_string("/proc/uptime").ok()?;
    text.split_whitespace().next()?.parse::<f64>().ok()
}

/// Cached CPU samples for percentage deltas (Python `_RES_CACHE`).
struct ResCache {
    stamp: Option<Instant>,
    sys_total: Option<f64>,
    sys_idle: Option<f64>,
    proc_utime: Option<f64>,
}
static RES_CACHE: Mutex<ResCache> = Mutex::new(ResCache {
    stamp: None,
    sys_total: None,
    sys_idle: None,
    proc_utime: None,
});

/// Python `_get_resource_stats()` → the same JSON structure.
pub fn get_resource_stats(music_dir: &Path) -> Value {
    // Linux defaults for os.sysconf('SC_CLK_TCK') / os.sysconf('SC_PAGE_SIZE').
    let clk_tck: f64 = 100.0;
    let page_size: u64 = 4096;

    let mut system = json!({
        "cpu_percent": Value::Null,
        "cpu_count": Value::Null,
        "load_avg": Value::Null,
        "mem_total_bytes": Value::Null,
        "mem_used_bytes": Value::Null,
        "mem_available_bytes": Value::Null,
        "uptime_seconds": Value::Null,
    });
    let mut process = json!({
        "rss_bytes": Value::Null,
        "cpu_percent": Value::Null,
        "threads": Value::Null,
        "uptime_seconds": Value::Null,
    });

    system["cpu_count"] = json!(std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1));
    if let Some(la) = read_loadavg() {
        system["load_avg"] = json!(la);
    }
    if let Some(up) = read_uptime() {
        system["uptime_seconds"] = json!(up);
    }
    let mem = read_meminfo();
    if let Some(t) = mem.get("MemTotal") {
        system["mem_total_bytes"] = json!(t);
    }
    if let Some(a) = mem.get("MemAvailable") {
        system["mem_available_bytes"] = json!(a);
        if let Some(t) = mem.get("MemTotal") {
            system["mem_used_bytes"] = json!(t.saturating_sub(*a));
        }
    } else if let (Some(t), Some(f)) = (mem.get("MemTotal"), mem.get("MemFree")) {
        system["mem_used_bytes"] = json!(t.saturating_sub(*f));
    }

    // process counters
    let (st_total, st_idle) = read_proc_stat_cpu().unwrap_or((0.0, 0.0));
    let proc_stat = read_proc_self_stat();
    if let Some((_, rss_pages, threads, start_time)) = proc_stat {
        process["rss_bytes"] = json!(rss_pages * page_size);
        process["threads"] = json!(threads);
        // Python fallback: system uptime − process start offset.
        if let Some(sys_up) = system["uptime_seconds"].as_f64() {
            let up = sys_up - start_time / clk_tck;
            if up > 0.0 {
                process["uptime_seconds"] = json!(up);
            }
        }
    }

    // CPU percentages via deltas (Python `_RES_CACHE`).
    {
        let mut cache = RES_CACHE.lock().unwrap();
        let proc_total = proc_stat.map(|(u, _, _, _)| u);
        if let (Some(prev_ts), Some(prev_sys), Some(prev_idle), Some(prev_proc), Some(pt)) = (
            cache.stamp,
            cache.sys_total,
            cache.sys_idle,
            cache.proc_utime,
            proc_total,
        ) {
            let dt = prev_ts.elapsed().as_secs_f64();
            if dt > 0.0 {
                let dsys = st_total - prev_sys;
                let didle = st_idle - prev_idle;
                let dproc = pt - prev_proc;
                if dsys > 0.0 {
                    let pct = ((dsys - didle) / dsys * 100.0).clamp(0.0, 100.0);
                    system["cpu_percent"] = json!((pct * 10.0).round() / 10.0);
                }
                let pct = ((dproc / clk_tck) / dt * 100.0).clamp(0.0, 100.0);
                process["cpu_percent"] = json!((pct * 10.0).round() / 10.0);
            }
        }
        cache.stamp = Some(Instant::now());
        cache.sys_total = Some(st_total);
        cache.sys_idle = Some(st_idle);
        cache.proc_utime = proc_total;
    }

    // disk (Python shutil.disk_usage → statvfs)
    let mut disk = json!({
        "music_dir": music_dir.to_string_lossy(),
        "total_bytes": Value::Null,
        "free_bytes": Value::Null,
        "used_bytes": Value::Null,
    });
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if let Ok(cp) = std::ffi::CString::new(music_dir.as_os_str().as_bytes()) {
            unsafe {
                let mut buf: libc::statvfs = std::mem::zeroed();
                if libc::statvfs(cp.as_ptr(), &mut buf) == 0 {
                    let total = buf.f_frsize as u64 * buf.f_blocks as u64;
                    let free = buf.f_frsize as u64 * buf.f_bavail as u64;
                    disk["total_bytes"] = json!(total);
                    disk["free_bytes"] = json!(free);
                    disk["used_bytes"] = json!(total.saturating_sub(free));
                }
            }
        }
    }

    json!({
        "system": system,
        "process": process,
        "disk": disk,
    })
}
