#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    local_agent_core::run();
}
