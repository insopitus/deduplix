// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use rayon::prelude::*;
use std::{
    collections::HashMap,
    fs,
    hash::Hasher,
    io::{BufReader, Read},
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Instant,
};
use tauri::Emitter;
use twox_hash::XxHash64;
use wax::Pattern;

const SEED: u64 = 1233135;

#[tauri::command(async)]
fn start_analysis(path: &str, config: Config, window: tauri::Window) -> Result<Report, String> {
    let root_path = path;
    let include = if config.include.is_empty() {
        None
    } else {
        Some(
            wax::any(config.include.split(","))
                .map_err(|_| "Invalid include pattern".to_string())?,
        )
    };
    let exclude = if config.exclude.is_empty() {
        None
    } else {
        Some(
            wax::any(config.exclude.split(","))
                .map_err(|_| "Invalid exclude pattern".to_string())?,
        )
    };
    // partial hash sample byte count
    const SAMPLE_SIZE: usize = 1024;
    // full hash byte size per batch
    const BATCH_SIZE: usize = 64 * 1024;
    let size_range = config.size_extend.0..config.size_extend.1;
    let mut size_map = HashMap::new();
    let instant = Instant::now();
    read_entries(path, &mut size_map, &include, &exclude);
    let size_map: HashMap<_, _> = size_map
        .into_iter()
        .filter(|(size, list)| *size != 0 && size_range.contains(size) && list.len() > 1)
        .collect();
    let elapsed = instant.elapsed().as_secs_f32();
    println!("Ingestion took {} seconds", elapsed);
    let instant = Instant::now();
    let _ = window.emit("ingestion-done", ());
    let map: HashMap<u64, HashMap<u64, Vec<PathBuf>>> = size_map
        .into_par_iter()
        .map(|(size, list)| {
            let sample_size = SAMPLE_SIZE.min(size as usize);
            let mut quick_hash_map = HashMap::new();
            let mut buf = vec![0; sample_size];
            list.into_iter().for_each(|p| {
                if let Ok(file) = fs::File::open(&p) {
                    let mut reader = BufReader::new(file);
                    if let Ok(n) = reader.read(&mut buf) {
                        let hash = XxHash64::oneshot(SEED, &buf[..n]);
                        quick_hash_map
                            .entry(hash)
                            .and_modify(|v: &mut Vec<_>| {
                                v.push(p.clone());
                            })
                            .or_insert_with(|| vec![p]);
                    }
                }
            });
            let quick_hash_map: HashMap<u64, Vec<PathBuf>> = quick_hash_map
                .into_iter()
                .filter(|(_, list)| list.len() > 1)
                .collect();

            (size, quick_hash_map)
        })
        .collect();
    let elapsed = instant.elapsed().as_secs_f32();
    println!("Rough hashing took {} seconds", elapsed);
    let _ = window.emit("quick-hash-done", ());
    let instant = Instant::now();
    let map: HashMap<u64, HashMap<u64, Vec<PathBuf>>> = map
        .into_iter()
        .map(|(size, quick_map)| {
            let mut long_hash_map = HashMap::new();
            // if the file size is small enough, no need to hash again.
            if size <= SAMPLE_SIZE as u64 {
                for (hash, list) in quick_map {
                    long_hash_map.insert(hash, list);
                }
            } else {
                for (_, list) in quick_map {
                    // Compute hashes in parallel and collect results
                    let hash_path_pairs: Vec<(u64, PathBuf)> = list
                        .into_par_iter()
                        .map(|p| {
                            let mut buf = vec![0; BATCH_SIZE];
                            if let Ok(file) = fs::File::open(&p) {
                                let mut reader = BufReader::new(file);
                                let mut hasher = XxHash64::with_seed(SEED);

                                while let Ok(n) = reader.read(&mut buf) {
                                    if n == 0 {
                                        break;
                                    }
                                    hasher.write(&buf[..n]);
                                }
                                let hash = hasher.finish();
                                Some((hash, p))
                            } else {
                                None
                            }
                        })
                        .flatten()
                        .collect();

                    // Merge results into long_hash_map sequentially
                    for (hash, p) in hash_path_pairs {
                        long_hash_map
                            .entry(hash)
                            .and_modify(|vec: &mut Vec<_>| {
                                vec.push(p.clone());
                            })
                            .or_insert_with(|| vec![p]);
                    }
                }
            }

            let long_hash_map: HashMap<u64, Vec<PathBuf>> = long_hash_map
                .into_iter()
                .filter(|(_, list)| list.len() > 1)
                .collect();
            (size, long_hash_map)
        })
        .collect();
    let elapsed = instant.elapsed().as_secs_f32();
    println!("Long hashing took {} seconds", elapsed);
    let _ = window.emit("full-hash-done", ());
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
    // sort by size (decending)
    pairs.sort_by(|a, b| b.1.cmp(&a.1));
    let pairs = pairs
        .into_iter()
        .map(|(hash, size, list)| Record {
            hash: format!("{:016X}", hash),
            size: to_human_readable_size(size),
            files: list,
        })
        .collect();

    Ok(Report { pairs: pairs })
}

#[tauri::command]
fn remove_file(root_path: &str, rel_path: &str) -> Result<(), String> {
    let path = PathBuf::from(root_path).join(rel_path);
    fs::remove_file(path).map_err(|e| e.to_string())
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            start_analysis,
            remove_file,
            reveal_file_in_explorer
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// recursively read files
fn read_entries(
    path: impl AsRef<Path>,
    map: &mut HashMap<u64, Vec<PathBuf>>,
    include: &Option<wax::Any>,
    exclude: &Option<wax::Any>,
) {
    if let Ok(entries) = fs::read_dir(path.as_ref()) {
        for entry in entries.flatten() {
            if let Some(include) = include {
                if !include.is_match(entry.path().as_path()) {
                    continue;
                }
            }
            if let Some(exclude) = exclude {
                if exclude.is_match(entry.path().as_path()) {
                    continue;
                }
            }
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() && !metadata.is_symlink() {
                    let size = metadata.file_size();
                    map.entry(size)
                        .and_modify(|value: &mut Vec<PathBuf>| {
                            value.push(entry.path());
                        })
                        .or_insert_with(|| vec![entry.path()]);
                } else if metadata.is_dir() {
                    read_entries(entry.path(), map, include, exclude);
                }
            }
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
}
#[derive(Serialize)]
struct Record {
    hash: String,
    size: String,
    files: Vec<PathBuf>,
}

#[derive(Deserialize)]
struct Config {
    pub include: String,
    pub exclude: String,
    pub size_extend: (u64, u64),
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
