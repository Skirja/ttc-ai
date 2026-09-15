use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=profiles/codex.json");
    println!("cargo:rerun-if-env-changed=TTC_CODEX_PROFILES_BUILD");
    let source = env::var_os("TTC_CODEX_PROFILES_BUILD")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("profiles/codex.json"));
    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("codex-profiles.json");
    fs::copy(&source, &output).unwrap_or_else(|error| {
        panic!(
            "failed to compile Codex compatibility profiles from {}: {error}",
            source.display()
        )
    });
}
