use std::path::{Path, PathBuf};

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

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());

        if let Some(profile_dir) = out.ancestors().find(|p| p.file_name().is_some_and(|n| n == "build")).and_then(|p| p.parent())
            && let Some(dll) = steam_api_dll()
        {
            let _ = std::fs::copy(dll, profile_dir.join("steam_api64.dll"));
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
}
