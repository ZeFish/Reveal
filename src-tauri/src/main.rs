// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // OpenMP workers spin for ~200 ms after each parallel region by default.
    // With libraw's per-decode regions that is pure burn between photos, and it
    // starves whatever else is runnable. Set before libomp reads it.
    if std::env::var_os("KMP_BLOCKTIME").is_none() {
        unsafe { std::env::set_var("KMP_BLOCKTIME", "0") };
    }
    // Half the cores for libraw's OpenMP decodes, the rest left for everything
    // else that reads RAWs at the same time (AI cull, grid thumbnails). All
    // of them on libraw's pool made a cull's 60-photo prefilter take 47 s
    // behind four prefetch decodes; at half it takes 4.6 s (measured on this
    // 8-core Mac, 2026-09-30).
    if std::env::var_os("OMP_NUM_THREADS").is_none() {
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        unsafe { std::env::set_var("OMP_NUM_THREADS", (cores / 2).max(2).to_string()) };
    }
    reveal_lib::run()
}
