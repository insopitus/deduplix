use nohash_hasher::IntMap;
use rayon::prelude::*;
use std::{
    fs,
    hash::Hasher,
    io::{BufReader, Read},
    ops::Range,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
    time::Instant,
};
use twox_hash::XxHash64;
use wax::Pattern;

const SEED: u64 = 1233135;
/// Bytes compared by the cheap pre-filter.
const SAMPLE_SIZE: usize = 4 * 1024;
/// Read buffer used while hashing a whole file.
const BATCH_SIZE: usize = 256 * 1024;
/// `FILE_ATTRIBUTE_REPARSE_POINT`: symlinks, junctions and mount points.
/// They can leave the scan root or form a traversal cycle.
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

#[derive(Clone, Debug)]
pub struct Config {
    pub include: String,
    pub exclude: String,
    pub size_extend: (u64, u64),
    pub quick_mode: bool,
}

#[derive(Clone, Debug)]
pub enum AnalysisEvent {
    IngestionDone,
    QuickProgress { done: usize, total: usize },
    QuickDone,
    FullProgress { done: usize, total: usize },
    FullDone,
}

#[derive(Clone, Debug)]
pub struct Record {
    pub hash: String,
    pub size: String,
    pub size_bytes: u64,
    pub files: Vec<String>,
    /// The hash only covers a sample of each file, not its full contents.
    pub sampled: bool,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub pairs: Vec<Record>,
    pub skipped: Vec<String>,
    pub cancelled: bool,
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

/// Analyze `root_path`, reporting each stage through `emit`.
///
/// The emitter is called from Rayon worker threads for progress updates, so it
/// must be thread safe.
pub fn analyze(
    root_path: &Path,
    config: &Config,
    token: &AtomicBool,
    emit: impl Fn(AnalysisEvent) + Sync,
) -> Result<Report, String> {
    let include = compile_patterns(&config.include, "include")?;
    let exclude = compile_patterns(&config.exclude, "exclude")?;

    let size_range = config.size_extend.0..config.size_extend.1;
    let skipped = Mutex::new(Vec::new());

    // Stage 1: walk the tree and group files by size.
    let mut size_map: IntMap<u64, Vec<PathBuf>> = IntMap::default();
    let instant = Instant::now();
    read_entries(
        root_path,
        root_path,
        &mut size_map,
        &include,
        &exclude,
        &size_range,
        token,
    );
    if token.load(Ordering::Relaxed) {
        return Ok(Report::cancelled());
    }
    let size_map: IntMap<u64, Vec<PathBuf>> = size_map
        .into_iter()
        .filter(|(_, list)| list.len() > 1)
        .collect();
    println!("Ingestion took {} seconds", instant.elapsed().as_secs_f32());
    emit(AnalysisEvent::IngestionDone);
    let quick_instant = Instant::now();

    // Stage 2: hash the first 4 KB of every same-size candidate.
    // For files no larger than the sample this is already a full-file hash.
    // Parallelize per file rather than per size: a single large size bucket
    // must not leave the other cores idle.
    let quick_total: usize = size_map.values().map(Vec::len).sum();
    emit(AnalysisEvent::QuickProgress {
        done: 0,
        total: quick_total,
    });
    let quick_done = AtomicUsize::new(0);
    let quick_report_every = (quick_total / 100).max(1);
    let quick_hashed: Vec<(u64, u64, PathBuf)> = size_map
        .into_par_iter()
        .flat_map_iter(|(size, list)| list.into_iter().map(move |path| (size, path)))
        .map_init(
            || vec![0u8; SAMPLE_SIZE],
            |buf, (size, path)| {
                if token.load(Ordering::Relaxed) {
                    return None;
                }
                let sample_size = SAMPLE_SIZE.min(size as usize);
                let hashed = match hash_sample(&path, &mut buf[..sample_size]) {
                    Some(hash) => Some((size, hash, path)),
                    None => {
                        skipped.lock().unwrap().push(path);
                        None
                    }
                };
                let processed = quick_done.fetch_add(1, Ordering::Relaxed) + 1;
                if processed == quick_total || processed % quick_report_every == 0 {
                    emit(AnalysisEvent::QuickProgress {
                        done: processed,
                        total: quick_total,
                    });
                }
                hashed
            },
        )
        .flatten()
        .collect();
    if token.load(Ordering::Relaxed) {
        return Ok(Report::cancelled());
    }

    let mut quick_map_by_size: IntMap<u64, IntMap<u64, Vec<PathBuf>>> = IntMap::default();
    for (size, hash, path) in quick_hashed {
        quick_map_by_size
            .entry(size)
            .or_default()
            .entry(hash)
            .or_default()
            .push(path);
    }
    let quick_map_by_size: IntMap<u64, IntMap<u64, Vec<PathBuf>>> = quick_map_by_size
        .into_iter()
        .filter_map(|(size, hash_map)| {
            let hash_map: IntMap<u64, Vec<PathBuf>> = hash_map
                .into_iter()
                .filter(|(_, list)| list.len() > 1)
                .collect();
            (!hash_map.is_empty()).then_some((size, hash_map))
        })
        .collect();
    println!(
        "Rough hashing took {} seconds",
        quick_instant.elapsed().as_secs_f32()
    );
    emit(AnalysisEvent::QuickDone);

    // Stage 3: hash full file contents.
    // Quick mode stops after the sampled hash, so its groups are only
    // candidates and must be verified explicitly before files are removed.
    // Small files already have a complete hash from stage 2, so reuse it.
    // All remaining files are hashed in one flat parallel batch; this keeps
    // every core busy even when there are many groups with only two files.
    let map: IntMap<u64, IntMap<u64, Vec<PathBuf>>> = if config.quick_mode {
        println!("Quick mode: skipping full hashing");
        quick_map_by_size
    } else {
        let full_instant = Instant::now();
        let mut map: IntMap<u64, IntMap<u64, Vec<PathBuf>>> = IntMap::default();
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
        if token.load(Ordering::Relaxed) {
            return Ok(Report::cancelled());
        }

        let full_total = large_files.len();
        emit(AnalysisEvent::FullProgress {
            done: 0,
            total: full_total,
        });
        let full_done = AtomicUsize::new(0);
        let full_report_every = (full_total / 100).max(1);

        let hashed: Vec<(u64, u64, PathBuf)> = large_files
            .into_par_iter()
            .map_init(
                || vec![0u8; BATCH_SIZE],
                |buf, (size, path)| {
                    let outcome = hash_file(&path, buf, token);
                    let processed = full_done.fetch_add(1, Ordering::Relaxed) + 1;
                    if processed == full_total || processed % full_report_every == 0 {
                        emit(AnalysisEvent::FullProgress {
                            done: processed,
                            total: full_total,
                        });
                    }
                    match outcome {
                        HashOutcome::Hashed(hash) => Some((size, hash, path)),
                        HashOutcome::Skipped => {
                            skipped.lock().unwrap().push(path);
                            None
                        }
                        HashOutcome::Cancelled => None,
                    }
                },
            )
            .flatten()
            .collect();
        if token.load(Ordering::Relaxed) {
            return Ok(Report::cancelled());
        }

        for (size, hash, path) in hashed {
            map.entry(size)
                .or_default()
                .entry(hash)
                .or_default()
                .push(path);
        }

        println!(
            "Long hashing took {} seconds",
            full_instant.elapsed().as_secs_f32()
        );
        emit(AnalysisEvent::FullDone);

        map.into_iter()
            .map(|(size, hash_map)| {
                let hash_map: IntMap<u64, Vec<PathBuf>> = hash_map
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
                    .map(|p| {
                        p.strip_prefix(root_path)
                            .unwrap_or(&p)
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect::<Vec<String>>(),
            ));
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
            // In quick mode files up to `SAMPLE_SIZE` are hashed completely
            // during the sampling stage, so their groups need no verification.
            sampled: config.quick_mode && size > SAMPLE_SIZE as u64,
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

/// Open a file for a long sequential read. The Windows flag lets the cache
/// manager read ahead and discard already-consumed pages sooner.
fn open_for_hashing(path: &Path) -> std::io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_SEQUENTIAL_SCAN)
        .open(path)
}

fn hash_file(path: &Path, buf: &mut [u8], token: &AtomicBool) -> HashOutcome {
    if token.load(Ordering::Relaxed) {
        return HashOutcome::Cancelled;
    }
    let Ok(file) = open_for_hashing(path) else {
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

#[derive(Clone, Debug)]
pub struct GroupVerification {
    /// Common full-file hash when every file matches, otherwise `None`.
    pub hash: Option<String>,
    pub all_same: bool,
    pub files: Vec<FileVerification>,
}

#[derive(Clone, Debug)]
pub struct FileVerification {
    pub path: String,
    pub hash: Option<String>,
    pub error: Option<String>,
}

/// Hash the full contents of every file in one candidate group. Used to check
/// groups produced by quick mode, where only a 4 KB sample was compared.
pub fn full_hash_group(
    root_path: &Path,
    rel_paths: &[String],
) -> Result<GroupVerification, String> {
    let token = AtomicBool::new(false);
    let files: Vec<FileVerification> = rel_paths
        .par_iter()
        .map_init(
            || vec![0u8; BATCH_SIZE],
            |buf, rel_path| {
                let path = root_path.join(rel_path);
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

pub fn remove_file(root_path: &Path, rel_path: &str) -> Result<(), String> {
    fs::remove_file(root_path.join(rel_path)).map_err(|e| e.to_string())
}

pub fn reveal_file_in_explorer(root_path: &Path, rel_path: &str) -> Result<(), String> {
    let path = root_path.join(rel_path);
    if !path.exists() {
        return Err("File doesn't exist.".into());
    }

    #[cfg(windows)]
    Command::new("explorer")
        .args(["/select,", &path.to_string_lossy()])
        .spawn()
        .map_err(|e| format!("Failed to open explorer: {e}"))?;

    Ok(())
}

/// Recursively read files.
///
/// `include` is only applied to files; directories must always be traversed
/// unless they match `exclude`. Applying `include` to directories would make
/// patterns such as `**/*.mp4` stop the walk at the root.
fn read_entries(
    root: &Path,
    path: &Path,
    map: &mut IntMap<u64, Vec<PathBuf>>,
    include: &Option<wax::Any>,
    exclude: &Option<wax::Any>,
    size_range: &Range<u64>,
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
                read_entries(root, &entry_path, map, include, exclude, size_range, token);
            }
        } else if metadata.is_file() {
            if !is_included(include, root, &entry_path) {
                continue;
            }
            if is_excluded(exclude, root, &entry_path) {
                continue;
            }
            let file_size = metadata.file_size();
            // Reject files that cannot be duplicates early so their paths are
            // never allocated into the size index.
            if file_size == 0 || !size_range.contains(&file_size) {
                continue;
            }
            map.entry(file_size)
                .and_modify(|value: &mut Vec<PathBuf>| {
                    value.push(entry_path.clone());
                })
                .or_insert_with(|| vec![entry_path]);
        }
    }
}

pub fn to_human_readable_size(size: u64) -> String {
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
