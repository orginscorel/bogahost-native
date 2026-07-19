// Windows'ta surum derlemesinde ek konsol penceresini engeller. KALDIRMAYIN.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    bogahost_chat_lib::run()
}
