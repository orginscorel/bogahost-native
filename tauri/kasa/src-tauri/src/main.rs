// Windows'ta konsol penceresi acilmasin (yalnizca release'te).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    bogahost_kasa_lib::run()
}
