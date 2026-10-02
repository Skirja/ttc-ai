//! Versioned ownership manifest. It never reads harness configuration.

use super::files::Result;
use std::path::Path;
use toml::{Table, Value};

pub(super) const MAX_METADATA: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PathOwnership {
    pub file: String,
    pub block: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Metadata {
    pub version: String,
    pub sha256: String,
    pub installed_path: String,
    pub active_harnesses: Vec<String>,
    pub path: Option<PathOwnership>,
}

pub(super) fn semver(value: &str) -> bool {
    let fields: Vec<_> = value.split('.').collect();
    fields.len() == 3
        && fields.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
        })
}

pub(super) fn checksum(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn text_path(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| "Path instalasi harus UTF-8".into())
}

impl Metadata {
    pub fn parse(bytes: &[u8], installed_path: &Path, bashrc: &Path, block: &str) -> Result<Self> {
        let text = std::str::from_utf8(bytes).map_err(|_| "Metadata bukan UTF-8")?;
        let table: Table = toml::from_str(text).map_err(|_| "Metadata instalasi tidak valid")?;
        let keys = [
            "schema_version",
            "owner",
            "version",
            "sha256",
            "installed_path",
            "active_harnesses",
            "path_owned",
            "path_file",
            "path_block",
        ];
        if table.len() != keys.len() || table.keys().any(|key| !keys.contains(&key.as_str())) {
            return Err("Field metadata instalasi tidak dikenal".into());
        }
        let string = |key: &str| -> Result<String> {
            table
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("Field metadata {key} tidak valid"))
        };
        if table.get("schema_version").and_then(Value::as_integer) != Some(1)
            || string("owner")? != "ttc"
        {
            return Err("Schema atau owner metadata tidak dikenal".into());
        }
        let version = string("version")?;
        let sha256 = string("sha256")?;
        let recorded_path = string("installed_path")?;
        if !semver(&version) || !checksum(&sha256) || recorded_path != text_path(installed_path)? {
            return Err("Versi, checksum, atau installed path metadata tidak valid".into());
        }
        let harnesses = table
            .get("active_harnesses")
            .and_then(Value::as_array)
            .ok_or("Daftar integrasi tidak valid")?;
        if harnesses.len() > 64 {
            return Err("Daftar integrasi terlalu besar".into());
        }
        let mut active_harnesses = Vec::new();
        for value in harnesses {
            let value = value.as_str().ok_or("Nama integrasi tidak valid")?;
            if value.is_empty()
                || value.len() > 64
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err("Nama integrasi tidak valid".into());
            }
            active_harnesses.push(value.to_owned());
        }
        let owned = table
            .get("path_owned")
            .and_then(Value::as_bool)
            .ok_or("PATH ownership tidak valid")?;
        let path_file = string("path_file")?;
        let path_block = string("path_block")?;
        let path = if owned {
            if path_file != text_path(bashrc)?
                || (path_block != block && path_block != format!("\n{block}"))
            {
                return Err("PATH ownership tidak dikenal".into());
            }
            Some(PathOwnership {
                file: path_file,
                block: path_block,
            })
        } else {
            if !path_file.is_empty() || !path_block.is_empty() {
                return Err("PATH ownership ambigu".into());
            }
            None
        };
        Ok(Self {
            version,
            sha256,
            installed_path: recorded_path,
            active_harnesses,
            path,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut table = Table::new();
        table.insert("schema_version".into(), Value::Integer(1));
        table.insert("owner".into(), Value::String("ttc".into()));
        table.insert("version".into(), Value::String(self.version.clone()));
        table.insert("sha256".into(), Value::String(self.sha256.clone()));
        table.insert(
            "installed_path".into(),
            Value::String(self.installed_path.clone()),
        );
        table.insert(
            "active_harnesses".into(),
            Value::Array(
                self.active_harnesses
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
        table.insert("path_owned".into(), Value::Boolean(self.path.is_some()));
        table.insert(
            "path_file".into(),
            Value::String(
                self.path
                    .as_ref()
                    .map(|p| p.file.clone())
                    .unwrap_or_default(),
            ),
        );
        table.insert(
            "path_block".into(),
            Value::String(
                self.path
                    .as_ref()
                    .map(|p| p.block.clone())
                    .unwrap_or_default(),
            ),
        );
        toml::to_string(&table)
            .map(String::into_bytes)
            .map_err(|error| error.to_string())
    }
}
