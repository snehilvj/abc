//! Sets `ABC_BUILD_ID`: the version and a hash of the workspace's sources,
//! lock file and build profile, so any rebuild that changes them changes it.
//! abc and abcd both embed it and compare it when they meet.

use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

fn main() {
    let crate_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = crate_dir.join("../..");
    let mut files = vec![root.join("Cargo.lock"), root.join("Cargo.toml")];
    for krate in ["abc-core", "abcd", "abc"] {
        let dir = root.join("crates").join(krate);
        println!("cargo:rerun-if-changed={}", dir.join("src").display());
        println!(
            "cargo:rerun-if-changed={}",
            dir.join("Cargo.toml").display()
        );
        files.push(dir.join("Cargo.toml"));
        collect(&dir.join("src"), &mut files);
    }
    files.push(crate_dir.join("build.rs"));
    println!(
        "cargo:rerun-if-changed={}",
        root.join("Cargo.lock").display()
    );
    println!("cargo:rerun-if-changed=build.rs");
    files.sort();

    let mut hasher = DefaultHasher::new();
    std::env::var("PROFILE")
        .unwrap_or_default()
        .hash(&mut hasher);
    for file in &files {
        file.strip_prefix(&root).unwrap_or(file).hash(&mut hasher);
        fs::read(file).unwrap_or_default().hash(&mut hasher);
    }
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let hash = hasher.finish() as u32;
    println!("cargo:rustc-env=ABC_BUILD_ID={version}+{hash:08x}");
}

/// Every file under `dir`.
fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}
