fn main() {
    // Homebrew libraw via pkg-config (same library the retired Swift shell
    // linked through its CLibRaw shim). Link directives are emitted by hand
    // because libraw.pc says `-lstdc++` (a Linux-ism): macOS only ships
    // libc++, so that flag breaks the link.
    let lib = pkg_config::Config::new()
        .cargo_metadata(false)
        .probe("libraw")
        .expect("libraw not found — `brew install libraw`");
    for p in &lib.link_paths {
        println!("cargo:rustc-link-search=native={}", p.display());
    }
    for l in &lib.libs {
        let l = if l == "stdc++" { "c++" } else { l };
        println!("cargo:rustc-link-lib={l}");
    }

    let mut builder = bindgen::Builder::default()
        .header_contents("wrapper.h", "#include <libraw/libraw.h>")
        .allowlist_function("libraw_.*")
        .allowlist_type("libraw_.*")
        .allowlist_var("LIBRAW_.*")
        .layout_tests(false)
        .derive_debug(false);
    for p in &lib.include_paths {
        builder = builder.clang_arg(format!("-I{}", p.display()));
    }

    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    builder
        .generate()
        .expect("bindgen failed on libraw.h")
        .write_to_file(out.join("libraw_bindings.rs"))
        .expect("writing libraw bindings");
}
