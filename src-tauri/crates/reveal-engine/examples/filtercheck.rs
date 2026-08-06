//! Proof for the Y/M enlarger-filter bug and its fix:
//!   cargo run -p reveal-engine --example filtercheck
//!
//! Upstream bakes the Y/M dichroic filter shifts into the *filtered enlarger
//! illuminant* inside `Pipeline::new*`; `with_params` never recomputes it. So
//! reusing one cached template across filter changes silently pins the
//! illuminant — which is exactly why both sliders did nothing. This prints the
//! illuminant for three cases; the middle one is the bug.
use spektrafilm_core::params::RuntimeParams;
use spektrafilm_core::pipeline::Pipeline;
use spektrafilm_core::profile;
use std::path::PathBuf;

fn sig(p: &Pipeline) -> f64 {
    // A cheap signature of the illuminant spectrum: sum of its samples.
    p.print_illuminant_slice().iter().sum()
}

fn main() {
    let data_dir = std::env::var("REVEAL_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"));

    let film_name = "kodak_gold_200";
    let paper_name = "kodak_portra_endura";
    let film = profile::load_profile_by_name(&data_dir, film_name).expect("film profile");
    let print = profile::load_profile_by_name(&data_dir, paper_name).expect("paper profile");

    let params_at = |y: f32, m: f32| {
        let mut p = RuntimeParams::default();
        p.enlarger.y_filter_shift = y;
        p.enlarger.m_filter_shift = m;
        p
    };

    // 1. Built fresh at Y=0
    let base = Pipeline::new(film.clone(), print.clone(), params_at(0.0, 0.0));
    let s0 = sig(&base);

    // 2. THE BUG: same template, only `with_params` with Y=+20
    let reused = base.clone().with_params(params_at(20.0, 0.0));
    let s_reused = sig(&reused);

    // 3. THE FIX: rebuilt at Y=+20 (what keying the cache on the shifts does)
    let rebuilt = Pipeline::new(film, print, params_at(20.0, 0.0));
    let s_rebuilt = sig(&rebuilt);

    println!("illuminant Σ  @ Y=0            : {s0:.6}");
    println!("illuminant Σ  @ Y=+20 (reused) : {s_reused:.6}");
    println!("illuminant Σ  @ Y=+20 (rebuilt): {s_rebuilt:.6}");
    println!();
    println!(
        "with_params changed the illuminant? {}  <- must be NO (that's the bug)",
        if (s_reused - s0).abs() > 1e-9 { "yes" } else { "NO" }
    );
    println!(
        "rebuild changed the illuminant?     {}  <- must be YES (that's the fix)",
        if (s_rebuilt - s0).abs() > 1e-9 { "YES" } else { "no" }
    );

    // ---- second suspect: camera EV also feeds a construction-time value ----
    // `print_exposure_factor` (the print-exposure normalization) is computed in
    // new_with_spectral from camera.exposure_compensation_ev. If `with_params`
    // can't refresh it, the EXPOSURE slider moves the image but leaves the print
    // normalization pinned to the EV the template was built at.
    let film2 = profile::load_profile_by_name(&data_dir, film_name).expect("film");
    let print2 = profile::load_profile_by_name(&data_dir, paper_name).expect("paper");
    let ev_params = |ev: f32| {
        let mut p = RuntimeParams::default();
        p.camera.exposure_compensation_ev = ev;
        p
    };
    let ev0 = Pipeline::new_with_spectral(film2.clone(), print2.clone(), ev_params(0.0), &data_dir)
        .expect("spectral pipeline @EV0");
    let f0 = ev0.print_exposure_factor();
    let f_reused = ev0.clone().with_params(ev_params(1.0)).print_exposure_factor();
    let f_rebuilt = Pipeline::new_with_spectral(film2, print2, ev_params(1.0), &data_dir)
        .expect("spectral pipeline @EV+1")
        .print_exposure_factor();

    println!();
    println!("print_exposure_factor @ EV=0             : {f0:.6}");
    println!("print_exposure_factor @ EV=+1 (reused)   : {f_reused:.6}");
    println!("print_exposure_factor @ EV=+1 (rebuilt)  : {f_rebuilt:.6}");
    println!(
        "EV stale through with_params? {}",
        if (f_reused - f0).abs() < 1e-12 && (f_rebuilt - f0).abs() > 1e-12 {
            "YES — exposure needs the key too"
        } else if (f_rebuilt - f0).abs() <= 1e-12 {
            "no — EV doesn't affect the factor here (compensation off/no-op)"
        } else {
            "partially"
        }
    );
}
