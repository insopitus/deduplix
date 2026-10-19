// Portable app: never show a console window on Windows.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    dedupper_lib::run();
}
