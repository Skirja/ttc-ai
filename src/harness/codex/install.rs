use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use serde::{Deserialize, Serialize};

use super::config;
use crate::distribution::integration::{self, Context, FileSnapshot};

type Result<T> = std::result::Result<T, String>;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    owner: String,
    config: String,
    format: String,
    created: bool,
    definition: String,
    insertion: String,
}

fn codex_home() -> Result<PathBuf> {
    let path = std::env::var_os("CODEX_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|v| PathBuf::from(v).join(".codex")))
        .ok_or("HOME tidak tersedia")?;
    if !path.is_absolute() {
        return Err("CODEX_HOME harus absolute".into());
    }
    integration::text_path(&path)?;
    Ok(path)
}

fn probe(arguments: &[&str]) -> Result<String> {
    let mut child = Command::new("codex")
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Codex CLI tidak tersedia")?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or("stdout Codex tidak tersedia")?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > 64 * 1024 {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Probe Codex tidak valid atau terlalu besar".into());
    }
    if !child.wait().map_err(|_| "Probe Codex gagal")?.success() {
        return Err("Probe Codex gagal".into());
    }
    String::from_utf8(bytes).map_err(|_| "Probe Codex bukan UTF-8".into())
}

fn capability() -> Result<()> {
    let version = probe(&["--version"])?;
    let version = version
        .trim()
        .strip_prefix("codex-cli ")
        .ok_or("Versi Codex tidak dikenal")?;
    let numbers = version
        .split('.')
        .map(str::parse::<u64>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| "Versi Codex tidak valid")?;
    if numbers.len() != 3 || numbers.as_slice() < [0, 154, 0].as_slice() {
        return Err("Codex CLI minimum 0.154.0 diperlukan".into());
    }
    let features = probe(&["features", "list"])?;
    if !features.lines().any(|line| {
        let columns = line.split_whitespace().collect::<Vec<_>>();
        columns.first() == Some(&"hooks") && columns.last() == Some(&"true")
    }) {
        return Err("Fitur hooks Codex harus tersedia dan aktif; config dipertahankan".into());
    }
    Ok(())
}

fn run(installing: bool) -> Result<()> {
    let home = codex_home()?;
    if !installing && !integration::registered()? {
        println!("TTC tidak terdaftar; tidak ada hook yang dihapus");
        return Ok(());
    }
    if installing {
        capability()?;
    }
    let context = Context::begin()?;
    let receipt_snapshot = FileSnapshot::read(&context.receipt("codex"))?;
    let owned = config::group(&integration::text_path(context.binary())?);
    let previous=receipt_snapshot.bytes.as_ref().map(|bytes|->Result<Receipt>{
        let text=std::str::from_utf8(bytes).map_err(|_| "Receipt bukan UTF-8")?;
        let receipt:Receipt=toml::from_str(text).map_err(|_| "Receipt Codex tidak valid")?;
        if receipt.schema_version!=1 || receipt.owner!="ttc" || !matches!(receipt.format.as_str(),"toml"|"json") {
            return Err("Receipt Codex tidak dikenal".into());
        }
        let expected=home.join(if receipt.format=="json" {"hooks.json"} else {"config.toml"});
        if receipt.config!=integration::text_path(&expected)? { return Err("Lokasi config berubah; hapus integrasi pada lokasi yang tercatat terlebih dahulu".into()); }
        let definition:serde_json::Value=serde_json::from_str(&receipt.definition).map_err(|_| "Definisi receipt invalid")?;
        if definition!=owned { return Err("Definisi receipt Codex tidak dikenal".into()); }
        Ok(receipt)
    }).transpose()?;
    if previous.is_some() != context.active("codex") {
        return Err("Receipt dan active_harnesses tidak cocok".into());
    }
    if !installing && previous.is_none() {
        println!("Hook Codex TTC tidak terdaftar");
        return Ok(());
    }
    let (path, format) = if let Some(receipt) = &previous {
        (PathBuf::from(&receipt.config), receipt.format.clone())
    } else if std::fs::symlink_metadata(home.join("hooks.json")).is_ok() {
        (home.join("hooks.json"), "json".into())
    } else {
        (home.join("config.toml"), "toml".into())
    };
    let snapshot = FileSnapshot::read(&path)?;
    let original = match snapshot.bytes.as_ref() {
        Some(bytes) => std::str::from_utf8(bytes).map_err(|_| "Config Codex bukan UTF-8")?,
        None if format == "json" => "{}",
        None => "",
    };
    let value = config::parse(original, &format)?;
    let changes = if installing {
        if previous.is_some() {
            config::verify(&value, &owned, true)?;
            println!("Hook Codex TTC sudah terpasang; review/trust melalui /hooks bila diperlukan");
            return Ok(());
        }
        let (after, insertion) = config::install(original, &format, &owned)?;
        let receipt = Receipt {
            schema_version: 1,
            owner: "ttc".into(),
            config: integration::text_path(&path)?,
            format,
            created: snapshot.bytes.is_none(),
            definition: serde_json::to_string(&owned).map_err(|_| "Receipt encode gagal")?,
            insertion,
        };
        let receipt_bytes = toml::to_string(&receipt)
            .map_err(|_| "Receipt encode gagal")?
            .into_bytes();
        vec![
            snapshot.change(Some(after.into_bytes())),
            receipt_snapshot.change(Some(receipt_bytes)),
        ]
    } else {
        let receipt = previous.ok_or("Receipt tidak tersedia")?;
        let after = config::uninstall(original, &format, &owned, &receipt.insertion)?;
        let after = if receipt.created
            && ((format == "toml" && after.is_empty()) || (format == "json" && after == "{}"))
        {
            None
        } else {
            Some(after.into_bytes())
        };
        vec![snapshot.change(after), receipt_snapshot.change(None)]
    };
    let backups = context.commit("codex", installing, changes)?;
    for path in backups {
        println!("Backup/recovery: {}", path.display());
    }
    if installing {
        println!(
            "Hook Codex TTC terpasang. Buka Codex dan review/trust hook melalui /hooks; hook baru berjalan setelah trusted."
        );
    } else {
        println!("Hook Codex TTC dihapus; perubahan config pengguna dipertahankan");
    }
    Ok(())
}

fn status(result: Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ttc codex: {error}");
            ExitCode::FAILURE
        }
    }
}
pub(crate) fn install() -> ExitCode {
    status(run(true))
}
pub(crate) fn uninstall() -> ExitCode {
    status(run(false))
}
