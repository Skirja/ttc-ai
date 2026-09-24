//! Optional, user-owned limits for raw capture.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub(crate) const MAX_RAW_MB: u64 = 32;
pub(crate) const MAX_RETENTION_HOURS: u64 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Config {
    pub max_raw_mb: u64,
    pub retention_hours: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_raw_mb: MAX_RAW_MB,
            retention_hours: MAX_RETENTION_HOURS,
        }
    }
}

impl Config {
    pub(crate) fn load() -> Result<Self, String> {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        let path = PathBuf::from(home).join(".config/ttc/config.toml");
        Self::load_from(&path)
    }

    pub(crate) fn load_from(path: &Path) -> Result<Self, String> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(format!("cannot read config: {error}")),
        };
        let document: toml::Table =
            toml::from_str(&text).map_err(|error| format!("invalid config: {error}"))?;
        if document
            .keys()
            .any(|key| key != "max_raw_mb" && key != "retention_hours")
        {
            return Err("unknown config key".into());
        }
        let number = |name: &str, default: u64, max: u64| -> Result<u64, String> {
            match document.get(name) {
                None => Ok(default),
                Some(toml::Value::Integer(value)) if *value > 0 && (*value as u64) <= max => {
                    Ok(*value as u64)
                }
                _ => Err(format!("{name} must be an integer from 1 to {max}")),
            }
        };
        Ok(Self {
            max_raw_mb: number("max_raw_mb", MAX_RAW_MB, MAX_RAW_MB)?,
            retention_hours: number("retention_hours", MAX_RETENTION_HOURS, MAX_RETENTION_HOURS)?,
        })
    }

    pub(crate) fn max_bytes(self) -> usize {
        (self.max_raw_mb * 1024 * 1024) as usize
    }
}
