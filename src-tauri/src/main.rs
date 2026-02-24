// Prevents an additional console window from appearing in release builds on
// Windows. DO NOT REMOVE.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    jamiecam_lib::run();
}
