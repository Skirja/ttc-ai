//! Loss-aware renderers for well-defined human-facing records.
use regex::Regex;
use std::sync::OnceLock;
#[derive(Default)]
pub struct Renderer {
    search_file: [Option<String>; 2],
}
impl Renderer {
    pub fn line(&mut self, families: &[String], channel: usize, s: &str) -> Option<String> {
        if families.iter().any(|f| f == "git-status") {
            if let Some(branch) = s.strip_prefix("On branch ") {
                return Some(format!("branch {branch}"));
            }
            let header = match s {
                "Changes to be committed:" => Some("staged:"),
                "Changes not staged for commit:" => Some("unstaged:"),
                "Untracked files:" => Some("untracked:"),
                "Unmerged paths:" => Some("unmerged:"),
                _ => None,
            };
            if let Some(header) = header {
                return Some(header.into());
            }
            if let Some(rest) = s.strip_prefix('\t') {
                for (label, short) in [
                    ("modified:", "M"),
                    ("new file:", "A"),
                    ("deleted:", "D"),
                    ("renamed:", "R"),
                    ("both modified:", "UU"),
                ] {
                    if let Some(path) = rest.strip_prefix(label) {
                        return Some(format!("  {short} {}", path.trim_start()));
                    }
                }
            }
        }
        if families.iter().any(|f| f == "search-lines") {
            static SEARCH: OnceLock<Regex> = OnceLock::new();
            let re = SEARCH.get_or_init(|| {
                Regex::new(r"^([^:\r\n]+):([0-9]+):(.*)$").expect("static search record")
            });
            if let Some(c) = re.captures(s) {
                let file = &c[1];
                let line = &c[2];
                let content = &c[3];
                let result = if self.search_file[channel].as_deref() == Some(file) {
                    format!("  {line}: {content}")
                } else {
                    format!("{file}:\n  {line}: {content}")
                };
                self.search_file[channel] = Some(file.into());
                return Some(result);
            }
            self.search_file[channel] = None;
        }
        None
    }
}
