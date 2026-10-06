use super::{TestDir, ttc};
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

pub struct Fixture {
    pub root: TestDir,
    binary: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let fixture = Self {
            root: TestDir::new(),
            binary: std::env::var_os("TTC_M10_TEST_BINARY")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(ttc())),
        };
        std::fs::create_dir(fixture.root.path().join("tools")).unwrap();
        fixture.codex("0.160.0", true);
        let bytes = std::fs::read(&fixture.binary).unwrap();
        let result = fixture
            .source()
            .args([
                "__install",
                "--sha256",
                &format!("{:x}", Sha256::digest(bytes)),
            ])
            .output()
            .unwrap();
        assert!(result.status.success(), "{:?}", result);
        fixture
    }
    pub fn codex(&self, version: &str, hooks: bool) {
        let file = self.root.path().join("tools/codex");
        std::fs::write(&file,format!("#!/bin/sh\ncase \"$*\" in\n--version) printf 'codex-cli {version}\\n';;\n'features list') printf 'hooks stable {hooks}\\n';;\n*) exit 1;;\nesac\n")).unwrap();
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    pub fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }
    pub fn data(&self) -> PathBuf {
        self.root.path().join("data/ttc")
    }
    pub fn config(&self) -> PathBuf {
        self.root.path().join("codex/config.toml")
    }
    pub fn json(&self) -> PathBuf {
        self.root.path().join("codex/hooks.json")
    }
    fn environment(&self, mut command: Command) -> Command {
        std::fs::create_dir_all(self.home()).unwrap();
        command
            .env("HOME", self.home())
            .env("XDG_DATA_HOME", self.root.path().join("data"))
            .env("XDG_STATE_HOME", self.root.path().join("state"))
            .env("XDG_CONFIG_HOME", self.root.path().join("config"))
            .env("CODEX_HOME", self.root.path().join("codex"))
            .env("TMPDIR", self.root.path())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.root.path().join("tools").display(),
                    std::env::var("PATH").unwrap()
                ),
            );
        command
    }
    pub fn source(&self) -> Command {
        self.environment(Command::new(&self.binary))
    }
    pub fn command(&self) -> Command {
        self.environment(Command::new(self.home().join(".local/bin/ttc")))
    }
    pub fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
    pub fn write(&self, path: &std::path::Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    pub fn install(&self) {
        let result = self.run(&["install", "codex"]);
        assert!(result.status.success(), "{:?}", result);
    }
    pub fn uninstall(&self) {
        let result = self.run(&["uninstall", "codex"]);
        assert!(result.status.success(), "{:?}", result);
    }
}
