//! Local TOML configuration. Project configuration cannot relax hook policy.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub recovery: Recovery,
    pub metrics: bool,
    pub hooks: Hooks,
    pub exclusions: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Recovery {
    pub enabled: bool,
    pub ttl_hours: u64,
    pub max_entries: usize,
    pub max_total_mb: u64,
    pub max_capture_mb: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Hooks {
    pub enabled: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            recovery: Recovery::default(),
            metrics: true,
            hooks: Hooks::default(),
            exclusions: vec![],
        }
    }
}
impl Default for Recovery {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_hours: 24,
            max_entries: 100,
            max_total_mb: 250,
            max_capture_mb: 64,
        }
    }
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("TTC_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("ttc-ai")
        })
}
pub fn config_path() -> PathBuf {
    std::env::var_os("TTC_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("ttc-ai/config.toml")
        })
}
pub fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join(".codex")
        })
}
fn merge(a: &mut toml::Value, b: toml::Value) {
    match (a, b) {
        (toml::Value::Table(a), toml::Value::Table(b)) => {
            for (k, v) in b {
                if let Some(old) = a.get_mut(&k) {
                    merge(old, v)
                } else {
                    a.insert(k, v);
                }
            }
        }
        (a, b) => *a = b,
    }
}
pub fn load(cwd: &Path) -> Result<Config> {
    let mut v = toml::Value::try_from(Config::default())?;
    let global = config_path();
    if global.exists() {
        merge(
            &mut v,
            std::fs::read_to_string(&global)?
                .parse::<toml::Value>()
                .context("invalid global TTC config")?,
        );
    }
    let global_config: Config = v.clone().try_into()?;
    for ancestor in cwd.ancestors() {
        let path = ancestor.join(".ttc.toml");
        if path.exists() {
            merge(
                &mut v,
                std::fs::read_to_string(path)?
                    .parse::<toml::Value>()
                    .context("invalid project TTC config")?,
            );
            break;
        }
        if ancestor.join(".git").exists() {
            break;
        }
    }
    let mut c: Config = v.try_into()?;
    c.hooks.enabled &= global_config.hooks.enabled;
    c.exclusions.extend(global_config.exclusions);
    if std::env::var("TTC_BYPASS").as_deref() == Ok("1") {
        c.enabled = false;
    }
    if std::env::var("TTC_RECOVERY").as_deref() == Ok("0") {
        c.recovery.enabled = false;
    }
    if std::env::var("TTC_METRICS").as_deref() == Ok("0") {
        c.metrics = false;
    }
    if c.exclusions
        .iter()
        .any(|p| glob::Pattern::new(p).is_ok_and(|p| p.matches_path(cwd)))
    {
        c.enabled = false;
    }
    Ok(c)
}
