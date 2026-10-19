# Deduplix

A local duplicate file finder with a native [GPUI](https://gpui.rs) interface.

## How it works

1. **Size** — files are grouped by exact byte size; unique sizes are discarded.
2. **Sampled hash** — for each remaining file, a 4 KiB sample is hashed with XXH64.
3. **Full hash** — candidates are hashed end to end. Quick mode stops at step 2 and
   leaves the final decision to you (nothing is deleted automatically).

Analysis can be cancelled at any time.

## Build

Requires the Rust toolchain (MSVC on Windows).

```sh
cargo build --release
```

The result is a single portable executable at `target/release/deduplix.exe`. No
installer or runtime is needed — copy it anywhere and run it.

On Windows the program icon is embedded at build time (`app.rc` →
`assets/icon.ico`), and the binary is built as a GUI subsystem app so no console
window appears.

## Development

```sh
cargo run
```
