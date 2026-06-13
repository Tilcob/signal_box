//! Copies `assets/` next to the built executable so the game also runs when
//! launched directly (double-click on target\debug\signal_box.exe) — the
//! game loads assets relative to the working directory, which is the exe's
//! folder in that case, not the project root.

use std::path::Path;
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=assets");

    let profile = env::var("PROFILE").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let src = Path::new(&manifest_dir).join("assets");
    let dst = Path::new(&manifest_dir)
        .join("target")
        .join(&profile)
        .join("assets");

    if src.exists() {
        // Full refresh: stale copies are worse than the copy cost (deleted
        // or renamed levels would silently survive next to the exe).
        let _ = fs::remove_dir_all(&dst);
        if let Err(e) = copy_dir(&src, &dst) {
            println!("cargo:warning=asset copy failed: {e}");
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
