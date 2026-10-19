use std::path::{Path, PathBuf};
use std::process::Command;

fn get_git_hash() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    println!("cargo::rustc-env=GIT_HASH={}", git_hash);
    println!("cargo::rerun-if-changed=.git/HEAD");
}

fn steam_api_dll() -> Option<PathBuf> {
    let home = std::env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| std::env::var_os("USERPROFILE").map(|h| Path::new(&h).join(".cargo")))?;

    std::fs::read_dir(home.join("registry").join("src"))
        .ok()?
        .flatten()
        .flat_map(|index| std::fs::read_dir(index.path()).into_iter().flatten().flatten())
        .filter(|krate| krate.file_name().to_string_lossy().starts_with("steamworks-sys-0.13."))
        .map(|krate| krate.path().join("lib/steam/redistributable_bin/win64/steam_api64.dll"))
        .filter(|dll| dll.is_file())
        .max()
}

fn set_windows_icon() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.compile().unwrap();
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());

        if let Some(profile_dir) = out.ancestors().find(|p| p.file_name().is_some_and(|n| n == "build")).and_then(|p| p.parent())
            && let Some(dll) = steam_api_dll()
        {
            let _ = std::fs::copy(dll, profile_dir.join("steam_api64.dll"));
        }

        set_windows_icon();
    }

    get_git_hash();

    println!("cargo:rerun-if-changed=build.rs");
}
