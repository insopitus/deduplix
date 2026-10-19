// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use rayon::prelude::*;
use std::{
    collections::HashMap,
    fs,
    hash::Hasher,
    io::{BufReader, Read},
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tauri::Emitter;
use twox_hash::XxHash64;
use wax::Pattern;

const SEED: u64 = 1233135;
/// Bytes compared by the cheap pre-filter.
const SAMPLE_SIZE: usize = 1024;
/// Read buffer used while hashing a whole file.
const BATCH_SIZE: usize = 64 * 1024;
/// `FILE_ATTRIBUTE_REPARSE_POINT`: symlinks, junctions and mount points.
/// They can leave the scan root or form a traversal cycle.
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

/// Tracks the currently running analysis so `cancel_analysis` can flip its
/// cancellation flag. The id prevents a stale request from cancelling a
/// newer analysis.
#[derive(Default)]
struct AnalysisState(Mutex<Option<(u64, Arc<AtomicBool>)>>);

/// Clears the registry when an analysis ends, but only if a newer analysis
/// has not replaced it in the meantime.
struct AnalysisGuard<'a> {
    state: &'a AnalysisState,
    id: u64,
}

impl Drop for AnalysisGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut current) = self.state.0.lock() {
            if matches!(current.as_ref(), Some((current_id, _)) if *current_id == self.id) {
                *current = None;
            }
        }
    }
}

#[tauri::command(async)]
fn start_analysis(
    id: u64,
    path: &str,
    config: Config,
    window: tauri::Window,
    state: tauri::State<'_, AnalysisState>,
) -> Result<Report, String> {
    let token = Arc::new(AtomicBool::new(false));
    {
        let mut current = state.0.lock().unwrap();
        // A new analysis supersedes an older one that is still winding down.
        if let Some((_, previous)) = current.take() {
            previous.store(true, Ordering::Relaxed);
        }
        *current = Some((id, token.clone()));
    }
    let _guard = AnalysisGuard { state: &*state, id };
    let is_cancelled = || token.load(Ordering::Relaxed);

    let root_path = Path::new(path);
    let include = compile_patterns(&config.include, "include")?;
    let exclude = compile_patterns(&config.exclude, "exclude")?;

    let size_range = config.size_extend.0..config.size_extend.1;
    let skipped = Mutex::new(Vec::new());

    // Stage 1: walk the tree and group files by size.
    let mut size_map = HashMap::new();
    let instant = Instant::now();
    read_entries(
        root_path,
        root_path,
        &mut size_map,
        &include,
        &exclude,
        token.as_ref(),
    );
    if is_cancelled() {
        return Ok(Report::cancelled());
    }
    let size_map: HashMap<_, _> = size_map
        .into_iter()
        .filter(|(size, list)| *size != 0 && size_range.contains(size) && list.len() > 1)
        .collect();
    let elapsed = instant.elapsed().as_secs_f32();
    println!("Ingestion took {} seconds", elapsed);
    let instant = Instant::now();
    let _ = window.emit("ingestion-done", ());

    // Stage 2: hash the first 1 KB of every same-size candidate.
    // For files no larger than the sample this is already a full-file hash.
    let quick_map_by_size: HashMap<u64, HashMap<u64, Vec<PathBuf>>> = size_map
        .into_par_iter()
        .map(|(size, list)| {
            let sample_size = SAMPLE_SIZE.min(size as usize);
            let mut quick_hash_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
            let mut buf = vec![0u8; sample_size];
            for path in list {
                if token.load(Ordering::Relaxed) {
                    break;
                }
                match hash_sample(&path, &mut buf) {
                    Some(hash) => quick_hash_map.entry(hash).or_default().push(path),
                    None => skipped.lock().unwrap().push(path),
                }
            }
            let quick_hash_map = quick_hash_map
                .into_iter()
                .filter(|(_, list)| list.len() > 1)
                .collect();
            (size, quick_hash_map)
        })
        .collect();
    if is_cancelled() {
        return Ok(Report::cancelled());
    }
    let elapsed = instant.elapsed().as_secs_f32();
    println!("Rough hashing took {} seconds", elapsed);
    let _ = window.emit("quick-hash-done", ());

    // Stage 3: hash full file contents.
    // Quick mode stops after the sampled hash, so its groups are only
    // candidates and must be verified explicitly before files are removed.
    // Small files already have a complete hash from stage 2, so reuse it.
    // All remaining files are hashed in one flat parallel batch; this keeps
    // every core busy even when there are many groups with only two files.
    let map: HashMap<u64, HashMap<u64, Vec<PathBuf>>> = if config.quick_mode {
        println!("Quick mode: skipping full hashing");
        quick_map_by_size
    } else {
        let instant = Instant::now();
        let mut map: HashMap<u64, HashMap<u64, Vec<PathBuf>>> = HashMap::new();
        let mut large_files: Vec<(u64, PathBuf)> = Vec::new();
        for (size, quick_map) in quick_map_by_size {
            if size <= SAMPLE_SIZE as u64 {
                let by_size = map.entry(size).or_default();
                for (hash, list) in quick_map {
                    by_size.insert(hash, list);
                }
            } else {
                for list in quick_map.into_values() {
                    for path in list {
                        large_files.push((size, path));
                    }
                }
            }
        }
        if is_cancelled() {
            return Ok(Report::cancelled());
        }

        let hashed: Vec<(u64, u64, PathBuf)> = large_files
            .into_par_iter()
            .map_init(
                || vec![0u8; BATCH_SIZE],
                |buf, (size, path)| match hash_file(&path, buf, token.as_ref()) {
                    HashOutcome::Hashed(hash) => Some((size, hash, path)),
                    HashOutcome::Skipped => {
                        skipped.lock().unwrap().push(path);
                        None
                    }
                    HashOutcome::Cancelled => None,
                },
            )
            .flatten()
            .collect();
        if is_cancelled() {
            return Ok(Report::cancelled());
        }

        for (size, hash, path) in hashed {
            map.entry(size)
                .or_default()
                .entry(hash)
                .or_default()
                .push(path);
        }

        let elapsed = instant.elapsed().as_secs_f32();
        println!("Long hashing took {} seconds", elapsed);
        let _ = window.emit("full-hash-done", ());

        map.into_iter()
            .map(|(size, hash_map)| {
                let hash_map = hash_map
                    .into_iter()
                    .filter(|(_, list)| list.len() > 1)
                    .collect();
                (size, hash_map)
            })
            .collect()
    };

    let mut pairs = vec![];
    for (size, hash_map) in map {
        for (hash, list) in hash_map {
            pairs.push((
                hash,
                size,
                list.into_iter()
                    .map(|p| p.strip_prefix(root_path).unwrap_or(&p).into())
                    .collect(),
            ))
        }
    }
    // sort by size (descending)
    pairs.sort_by(|a, b| b.1.cmp(&a.1));
    let pairs = pairs
        .into_iter()
        .map(|(hash, size, list)| Record {
            hash: format!("{:016X}", hash),
            size: to_human_readable_size(size),
            size_bytes: size,
            files: list,
        })
        .collect();

    let mut skipped: Vec<String> = skipped
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .map(|path| {
            path.strip_prefix(root_path)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    skipped.sort();
    skipped.dedup();

    Ok(Report {
        pairs,
        skipped,
        cancelled: false,
    })
}

/// Compile a comma-separated list of globs, trimming each item so that
/// `"a, b"` does not produce a pattern with a leading space.
fn compile_patterns<'a>(input: &'a str, kind: &str) -> Result<Option<wax::Any<'a>>, String> {
    let patterns: Vec<&str> = input
        .split(',')
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .collect();
    if patterns.is_empty() {
        return Ok(None);
    }
    wax::any(patterns)
        .map(Some)
        .map_err(|error| format!("Invalid {kind} pattern: {error}"))
}

/// Match a pattern against both the absolute path and the path relative to
/// the scan root, so `*.txt` and `sub/*.txt` behave the way users expect
/// without forcing a leading `**/`.
fn matches_pattern(patterns: &wax::Any, root: &Path, path: &Path) -> bool {
    patterns.is_match(path)
        || path
            .strip_prefix(root)
            .map(|relative| patterns.is_match(relative))
            .unwrap_or(false)
}

fn is_included(include: &Option<wax::Any>, root: &Path, path: &Path) -> bool {
    match include {
        Some(patterns) => matches_pattern(patterns, root, path),
        None => true,
    }
}

fn is_excluded(exclude: &Option<wax::Any>, root: &Path, path: &Path) -> bool {
    match exclude {
        Some(patterns) => matches_pattern(patterns, root, path),
        None => false,
    }
}

/// Read the first `buf.len()` bytes of a file. `read_exact` is important:
/// a single `read` call is allowed to return fewer bytes than requested,
/// which would make identical files produce different sample hashes.
fn hash_sample(path: &Path, buf: &mut [u8]) -> Option<u64> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    reader.read_exact(buf).ok()?;
    Some(XxHash64::oneshot(SEED, buf))
}

enum HashOutcome {
    Hashed(u64),
    Skipped,
    Cancelled,
}

fn hash_file(path: &Path, buf: &mut [u8], token: &AtomicBool) -> HashOutcome {
    if token.load(Ordering::Relaxed) {
        return HashOutcome::Cancelled;
    }
    let Ok(file) = fs::File::open(path) else {
        return HashOutcome::Skipped;
    };
    let mut reader = BufReader::new(file);
    let mut hasher = XxHash64::with_seed(SEED);
    loop {
        if token.load(Ordering::Relaxed) {
            return HashOutcome::Cancelled;
        }
        match reader.read(buf) {
            Ok(0) => return HashOutcome::Hashed(hasher.finish()),
            Ok(n) => hasher.write(&buf[..n]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            // Do not return a hash of a partially-read file: it could be
            // mistaken for the content of another truncated file.
            Err(_) => return HashOutcome::Skipped,
        }
    }
}

#[tauri::command]
fn cancel_analysis(state: tauri::State<'_, AnalysisState>, id: u64) {
    if let Ok(current) = state.0.lock() {
        if let Some((current_id, token)) = current.as_ref() {
            if *current_id == id {
                token.store(true, Ordering::Relaxed);
            }
        }
    }
}

#[derive(Serialize)]
struct GroupVerification {
    /// Common full-file hash when every file matches, otherwise `None`.
    hash: Option<String>,
    all_same: bool,
    files: Vec<FileVerification>,
}

#[derive(Serialize)]
struct FileVerification {
    path: String,
    hash: Option<String>,
    error: Option<String>,
}

/// Hash the full contents of every file in one candidate group. Used to check
/// groups produced by quick mode, where only a 1 KB sample was compared.
#[tauri::command(async)]
fn full_hash_group(root_path: &str, rel_paths: Vec<String>) -> Result<GroupVerification, String> {
    let root = Path::new(root_path);
    let token = AtomicBool::new(false);
    let files: Vec<FileVerification> = rel_paths
        .par_iter()
        .map_init(
            || vec![0u8; BATCH_SIZE],
            |buf, rel_path| {
                let path = root.join(rel_path);
                match hash_file(&path, buf, &token) {
                    HashOutcome::Hashed(hash) => FileVerification {
                        path: rel_path.clone(),
                        hash: Some(format!("{:016X}", hash)),
                        error: None,
                    },
                    HashOutcome::Skipped => FileVerification {
                        path: rel_path.clone(),
                        hash: None,
                        error: Some("Could not read the file".to_string()),
                    },
                    HashOutcome::Cancelled => FileVerification {
                        path: rel_path.clone(),
                        hash: None,
                        error: Some("Hashing was interrupted".to_string()),
                    },
                }
            },
        )
        .collect();

    let all_same = !files.is_empty()
        && files[0].hash.is_some()
        && files.iter().all(|file| file.hash == files[0].hash);
    let hash = all_same.then(|| files[0].hash.clone().unwrap_or_default());

    Ok(GroupVerification {
        hash,
        all_same,
        files,
    })
}

#[tauri::command]
fn remove_file(root_path: &str, rel_path: &str) -> Result<(), String> {
    let path = PathBuf::from(root_path).join(rel_path);
    fs::remove_file(path).map_err(|e| e.to_string())
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AnalysisState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            start_analysis,
            cancel_analysis,
            full_hash_group,
            remove_file,
            reveal_file_in_explorer
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Recursively read files.
///
/// `include` is only applied to files; directories must always be traversed
/// unless they match `exclude`. Applying `include` to directories would make
/// patterns such as `**/*.mp4` stop the walk at the root.
fn read_entries(
    root: &Path,
    path: &Path,
    map: &mut HashMap<u64, Vec<PathBuf>>,
    include: &Option<wax::Any>,
    exclude: &Option<wax::Any>,
    token: &AtomicBool,
) {
    if token.load(Ordering::Relaxed) {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        if token.load(Ordering::Relaxed) {
            return;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        // Never follow links.
        if file_type.is_symlink() {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        // Junctions and mount points are reparse points but are not always
        // reported as symlinks by `FileType::is_symlink`.
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            continue;
        }

        let entry_path = entry.path();
        if metadata.is_dir() {
            if !is_excluded(exclude, root, &entry_path) {
                read_entries(root, &entry_path, map, include, exclude, token);
            }
        } else if metadata.is_file() {
            if !is_included(include, root, &entry_path) {
                continue;
            }
            if is_excluded(exclude, root, &entry_path) {
                continue;
            }
            map.entry(metadata.file_size())
                .and_modify(|value: &mut Vec<PathBuf>| {
                    value.push(entry_path.clone());
                })
                .or_insert_with(|| vec![entry_path]);
        }
    }
}

use std::process::Command;

#[tauri::command]
fn reveal_file_in_explorer(root_path: &str, rel_path: &str) -> Result<(), String> {
    let path = Path::new(&root_path).join(rel_path);

    if !path.exists() {
        return Err("File doesn't exist.".into());
    }

    // Windows system command
    #[cfg(windows)]
    Command::new("explorer")
        .args(&["/select,", &path.to_string_lossy()])
        .spawn()
        .map_err(|e| format!("Failed to open explorer: {}", e))?;

    Ok(())
}

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct Report {
    pairs: Vec<Record>,
    skipped: Vec<String>,
    cancelled: bool,
}

impl Report {
    fn cancelled() -> Self {
        Report {
            pairs: Vec::new(),
            skipped: Vec::new(),
            cancelled: true,
        }
    }
}

#[derive(Serialize)]
struct Record {
    hash: String,
    size: String,
    size_bytes: u64,
    files: Vec<PathBuf>,
}

#[derive(Deserialize)]
struct Config {
    pub include: String,
    pub exclude: String,
    pub size_extend: (u64, u64),
    /// Stop after the sampled-hash stage instead of hashing full contents.
    #[serde(default)]
    pub quick_mode: bool,
}

fn to_human_readable_size(size: u64) -> String {
    let size = size as f32;
    const N: f32 = 1024.0;
    if size > N {
        let k = size / N;
        if k > N {
            let m = k / N;
            if m > N {
                let g = m / N;
                if g > N {
                    let t = g / N;
                    format!("{}TB", format_significant(t, 3))
                } else {
                    format!("{}GB", format_significant(g, 3))
                }
            } else {
                format!("{}MB", format_significant(m, 3))
            }
        } else {
            format!("{}KB", format_significant(k, 3))
        }
    } else {
        format!("{}B", size)
    }
}

fn format_significant(value: f32, digits: usize) -> String {
    if value == 0.0 {
        return "0".to_string();
    }

    let log10 = value.abs().log10().floor() as i32;
    let decimals = (digits as i32 - 1 - log10).clamp(0, 15) as usize;

    format!("{0:.1$}", value, decimals)
}
