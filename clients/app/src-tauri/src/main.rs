// 桌面端入口：真正的逻辑在库 crate，便于测试与复用。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    cicada_app_lib::run();
}
