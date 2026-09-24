#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::{TestDir, ttc_command};
use core::config::Config;
use std::fs;

#[test]
fn defaults_and_valid_smaller_limits() {
    let dir = TestDir::new();
    let path = dir.path().join("config.toml");
    assert_eq!(Config::load_from(&path).unwrap(), Config::default());
    fs::write(&path, "max_raw_mb = 2\nretention_hours = 3\n").unwrap();
    let config = Config::load_from(&path).unwrap();
    assert_eq!(config.max_raw_mb, 2);
    assert_eq!(config.retention_hours, 3);
}

#[test]
fn invalid_config_executes_original_once_with_exact_output() {
    for body in [
        "max_raw_mb = 33\n",
        "retention_hours = 25\n",
        "max_raw_mb = 0\n",
        "other = 1\n",
        "[oops\n",
    ] {
        let dir = TestDir::new();
        let config_dir = dir.path().join(".config/ttc");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.toml"), body).unwrap();
        let count = dir.path().join("count");
        let script = "printf x >> \"$COUNT\"; printf 'raw\\n'; printf 'warn\\n' >&2; exit 7";
        let output = ttc_command()
            .args(["/bin/sh", "-c", script])
            .env("HOME", dir.path())
            .env("COUNT", &count)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout, b"raw\n");
        assert_eq!(output.stderr, b"warn\n");
        assert_eq!(fs::read(count).unwrap(), b"x");
    }
}
