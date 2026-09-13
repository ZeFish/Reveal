fn main() {
    println!("cargo:rerun-if-changed=native/apple_photos.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
            "aarch64" => "arm64",
            "x86_64" => "x86_64",
            other => panic!("Unsupported macOS architecture: {other}"),
        };
        let status = std::process::Command::new("xcrun")
            .args([
                "clang",
                "-arch",
                arch,
                "-fobjc-arc",
                "-fblocks",
                "-mmacosx-version-min=11.0",
                "-c",
                "native/apple_photos.m",
                "-o",
            ])
            .arg(out.join("apple_photos.o"))
            .status()
            .expect("Apple Photos requires the Xcode command-line tools");
        assert!(
            status.success(),
            "Could not compile the Apple Photos bridge"
        );
        let status = std::process::Command::new("ar")
            .arg("crs")
            .arg(out.join("libreveal_photos.a"))
            .arg(out.join("apple_photos.o"))
            .status()
            .expect("Could not run ar");
        assert!(
            status.success(),
            "Could not archive the Apple Photos bridge"
        );
        println!("cargo:rustc-link-search=native={}", out.display());
        println!("cargo:rustc-link-lib=static=reveal_photos");
        for framework in ["Photos", "AppKit", "Foundation", "ImageIO", "CoreGraphics"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    tauri_build::build()
}
