//! Bounded, read-only discovery of project manifests used as filter hints.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSetBuilder};
use quick_xml::Reader as XmlReader;
use quick_xml::events::Event as XmlEvent;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_PROJECTS: usize = 4096;
const MAX_DIRECTORY_ENTRIES: usize = 16_384;
const MAX_DEPTH: usize = 64;

#[derive(Clone, Debug, Default)]
pub(crate) struct Project {
    pub path: PathBuf,
    pub name: Option<String>,
    pub aliases: Vec<String>,
    pub scripts: BTreeMap<String, String>,
    pub nx_targets: BTreeMap<String, String>,
    pub dependencies: HashSet<String>,
    pub moon_tasks: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WorkspaceHints {
    pub root: PathBuf,
    pub projects: Vec<Project>,
    pub has_package_workspace: bool,
    pub has_cargo_workspace: bool,
    pub has_go_workspace: bool,
    pub turbo_tasks: HashSet<String>,
    pub nx_targets: HashSet<String>,
    pub lerna_tasks: HashSet<String>,
    pub moon_tasks: HashSet<String>,
    pub package_manager: Option<String>,
    pub yarn_classic: Option<bool>,
    pub pnpm_pre_post: Option<bool>,
    pub has_composer_manifest: bool,
    pub composer_scripts: BTreeMap<String, String>,
    pub maven_modules: Vec<String>,
    pub gradle_modules: Vec<String>,
    pub gradle_dynamic: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiscoveryError {
    Io,
    Malformed,
    Limit,
}

#[derive(Default)]
struct Budget {
    bytes: usize,
    entries: usize,
}

impl Budget {
    fn read(&mut self, path: &Path) -> Result<Vec<u8>, DiscoveryError> {
        let remaining = MAX_TOTAL_BYTES.saturating_sub(self.bytes);
        if remaining == 0 {
            return Err(DiscoveryError::Limit);
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|_| DiscoveryError::Io)?
            .take((MAX_MANIFEST_BYTES.min(remaining) + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| DiscoveryError::Io)?;
        if bytes.len() > MAX_MANIFEST_BYTES || bytes.len() > remaining {
            return Err(DiscoveryError::Limit);
        }
        self.bytes += bytes.len();
        Ok(bytes)
    }

    fn entry(&mut self) -> Result<(), DiscoveryError> {
        self.entries += 1;
        (self.entries <= MAX_DIRECTORY_ENTRIES)
            .then_some(())
            .ok_or(DiscoveryError::Limit)
    }

    fn read_prefix(&mut self, path: &Path, limit: usize) -> Result<Vec<u8>, DiscoveryError> {
        let remaining = MAX_TOTAL_BYTES.saturating_sub(self.bytes);
        if remaining == 0 {
            return Err(DiscoveryError::Limit);
        }
        let amount = limit.min(remaining);
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|_| DiscoveryError::Io)?
            .take(amount as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| DiscoveryError::Io)?;
        self.bytes += bytes.len();
        Ok(bytes)
    }
}

impl WorkspaceHints {
    /// Discovers the nearest enclosing workspace without executing a tool.
    /// A missing project returns `Ok(None)`; an unreadable relevant manifest
    /// returns an error so callers can fail open for this invocation.
    pub(crate) fn discover(cwd: &Path) -> Result<Option<Self>, DiscoveryError> {
        let cwd = fs::canonicalize(cwd).map_err(|_| DiscoveryError::Io)?;
        let mut budget = Budget::default();
        let mut package_root = None;
        let mut workspace_root = None;
        let mut ancestors = 0;
        for directory in cwd.ancestors() {
            ancestors += 1;
            if ancestors > MAX_DEPTH {
                return Err(DiscoveryError::Limit);
            }
            if directory.join("package.json").exists() && package_root.is_none() {
                package_root = Some(directory.to_path_buf());
            }
            let package_workspace = if directory.join("package.json").exists() {
                let bytes = budget.read(&directory.join("package.json"))?;
                unique_json(&bytes)
                    .ok()
                    .and_then(|value| value.get("workspaces").cloned())
                    .is_some()
            } else {
                false
            };
            let cargo_workspace = if directory.join("Cargo.toml").exists() {
                let bytes = budget.read(&directory.join("Cargo.toml"))?;
                std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(|s| toml::from_str::<toml::Value>(s).ok())
                    .is_some_and(|v| v.get("workspace").is_some())
            } else {
                false
            };
            let marker_workspace = directory.join("pnpm-workspace.yaml").exists()
                || directory.join("go.work").exists()
                || directory.join("go.mod").exists()
                || directory.join("composer.json").exists()
                || directory.join("pom.xml").exists()
                || directory.join("settings.gradle").exists()
                || directory.join("settings.gradle.kts").exists()
                || directory.join("build.gradle").exists()
                || directory.join("build.gradle.kts").exists()
                || package_workspace
                || cargo_workspace
                || [
                    "turbo.json",
                    "nx.json",
                    "lerna.json",
                    "moon.yml",
                    "moon.yaml",
                    ".moon/workspace.yml",
                    ".moon/workspace.yaml",
                ]
                .iter()
                .any(|name| directory.join(name).exists());
            if marker_workspace {
                workspace_root = Some(directory.to_path_buf());
                break;
            }
        }
        let Some(root) = workspace_root.or(package_root) else {
            return Ok(None);
        };
        Self::load_from(root, &cwd, &mut budget).map(Some)
    }

    fn load_from(root: PathBuf, cwd: &Path, budget: &mut Budget) -> Result<Self, DiscoveryError> {
        let mut result = Self {
            root: root.clone(),
            ..Self::default()
        };
        let root_package = read_package(&root, budget)?;
        result.package_manager = root_package
            .as_ref()
            .and_then(|package| package.package_manager.clone());
        result.has_package_workspace = root_package
            .as_ref()
            .is_some_and(|p| !p.patterns.is_empty());
        result.projects.push(Project {
            path: root.clone(),
            name: root_package.as_ref().and_then(|p| p.name.clone()),
            aliases: Vec::new(),
            scripts: root_package
                .as_ref()
                .map(|p| p.scripts.clone())
                .unwrap_or_default(),
            nx_targets: BTreeMap::new(),
            dependencies: root_package
                .as_ref()
                .map(|p| p.dependencies.clone())
                .unwrap_or_default(),
            moon_tasks: BTreeMap::new(),
        });

        let mut patterns = root_package.map(|p| p.patterns).unwrap_or_default();
        if root.join("pnpm-workspace.yaml").exists() {
            result.has_package_workspace = true;
            let bytes = budget.read(&root.join("pnpm-workspace.yaml"))?;
            let settings = parse_pnpm_workspace(&bytes)?;
            patterns.extend(settings.patterns);
            result.pnpm_pre_post = settings.enable_pre_post_scripts;
        }
        if let Some(manager) = &result.package_manager
            && let Some(version) = manager.strip_prefix("yarn@")
        {
            result.yarn_classic = Some(version.starts_with('1'));
        }
        if result.yarn_classic.is_none() && root.join("yarn.lock").exists() {
            let prefix = budget.read_prefix(&root.join("yarn.lock"), 128)?;
            let prefix = String::from_utf8_lossy(&prefix);
            if prefix.contains("# yarn lockfile v1") {
                result.yarn_classic = Some(true);
            } else if prefix.contains("__metadata:") {
                result.yarn_classic = Some(false);
            }
        }
        if result.yarn_classic.is_none() && root.join(".yarnrc.yml").exists() {
            result.yarn_classic = Some(false);
        }
        if !patterns.is_empty() {
            discover_packages(&root, &patterns, &mut result.projects, budget)?;
        }

        let cargo = root.join("Cargo.toml");
        if cargo.exists() {
            let bytes = budget.read(&cargo)?;
            let source = std::str::from_utf8(&bytes).map_err(|_| DiscoveryError::Malformed)?;
            let value: toml::Value =
                toml::from_str(source).map_err(|_| DiscoveryError::Malformed)?;
            if let Some(workspace) = value.get("workspace") {
                result.has_cargo_workspace = true;
                let members = string_array(workspace.get("members"))?;
                let excludes = string_array(workspace.get("exclude"))?;
                for member in members {
                    for full in expand_project_pattern(&root, &member, budget)? {
                        if excludes
                            .iter()
                            .any(|exclude| glob_matches(exclude, &full, &root))
                        {
                            continue;
                        }
                        add_project_path(&mut result.projects, &full, &root, budget)?;
                    }
                }
            }
        }

        let go_work = root.join("go.work");
        if go_work.exists() {
            result.has_go_workspace = true;
            let bytes = budget.read(&go_work)?;
            for module in parse_go_work(&bytes)? {
                add_project_path(&mut result.projects, &root.join(module), &root, budget)?;
            }
        }

        load_runner_manifests(&root, &mut result, budget)?;
        load_php_jvm_manifests(&root, &mut result, budget)?;
        if result.projects.len() > MAX_PROJECTS {
            return Err(DiscoveryError::Limit);
        }
        result.projects.sort_by(|a, b| a.path.cmp(&b.path));
        // The supplied cwd must belong to the discovered root. Canonical paths
        // keep symlink aliases from creating duplicate project identities.
        if !cwd.starts_with(&root) {
            return Err(DiscoveryError::Malformed);
        }
        Ok(result)
    }

    pub(crate) fn project_at(&self, path: &Path) -> Option<&Project> {
        let canonical = fs::canonicalize(path).ok()?;
        self.projects
            .iter()
            .find(|project| project.path == canonical)
    }

    pub(crate) fn select_packages(&self, selectors: &[String]) -> Vec<&Project> {
        if selectors.is_empty() {
            return self.projects.iter().collect();
        }
        let mut selected = HashSet::new();
        if selectors
            .first()
            .is_some_and(|selector| selector.starts_with('!'))
        {
            selected.extend(0..self.projects.len());
        }
        for selector in selectors {
            if selector.contains('[') {
                continue;
            }
            let exclude = selector.starts_with('!');
            let mut pattern = selector.trim_start_matches('!');
            let dependents = pattern.starts_with("...");
            let dependencies = pattern.ends_with("...");
            if dependents {
                pattern = &pattern[3..];
            }
            if dependencies {
                pattern = &pattern[..pattern.len().saturating_sub(3)];
            }
            let only_dependencies = pattern.ends_with('^');
            if only_dependencies {
                pattern = &pattern[..pattern.len() - 1];
            }
            let only_dependents = pattern.starts_with('^');
            if only_dependents {
                pattern = &pattern[1..];
            }
            let matched: HashSet<usize> = self
                .projects
                .iter()
                .enumerate()
                .filter_map(|(index, project)| {
                    let relative_path = project
                        .path
                        .strip_prefix(&self.root)
                        .unwrap_or(Path::new("."));
                    let path = if relative_path.as_os_str().is_empty() {
                        Path::new(".")
                    } else {
                        relative_path
                    };
                    let path_text = path.to_string_lossy().replace('\\', "/");
                    let pattern = pattern.strip_prefix("./").unwrap_or(pattern);
                    let matcher = GlobBuilder::new(pattern)
                        .literal_separator(false)
                        .build()
                        .ok()
                        .map(|glob| glob.compile_matcher());
                    let name_match = project.name.as_deref().is_some_and(|name| {
                        name == pattern || matcher.as_ref().is_some_and(|glob| glob.is_match(name))
                    });
                    let glob_match = matcher
                        .as_ref()
                        .is_some_and(|glob| glob.is_match(&path_text));
                    (name_match || glob_match).then_some(index)
                })
                .collect();
            let mut expanded = matched.clone();
            if dependencies || only_dependencies {
                let mut frontier: Vec<usize> = matched.iter().copied().collect();
                while let Some(index) = frontier.pop() {
                    let deps = &self.projects[index].dependencies;
                    for (candidate, project) in self.projects.iter().enumerate() {
                        if project
                            .name
                            .as_ref()
                            .is_some_and(|name| deps.contains(name))
                            && expanded.insert(candidate)
                        {
                            frontier.push(candidate);
                        }
                    }
                }
                if only_dependencies {
                    for index in &matched {
                        expanded.remove(index);
                    }
                }
            }
            if dependents || only_dependents {
                let mut frontier: Vec<usize> = matched.iter().copied().collect();
                while let Some(index) = frontier.pop() {
                    let Some(name) = self.projects[index].name.as_ref() else {
                        continue;
                    };
                    for (candidate, project) in self.projects.iter().enumerate() {
                        if project.dependencies.contains(name) && expanded.insert(candidate) {
                            frontier.push(candidate);
                        }
                    }
                }
                if only_dependents {
                    for index in &matched {
                        expanded.remove(index);
                    }
                }
            }
            for index in expanded {
                if exclude {
                    selected.remove(&index);
                } else {
                    selected.insert(index);
                }
            }
        }
        self.projects
            .iter()
            .enumerate()
            .filter_map(|(i, p)| selected.contains(&i).then_some(p))
            .collect()
    }
}

struct PackageFile {
    name: Option<String>,
    scripts: BTreeMap<String, String>,
    dependencies: HashSet<String>,
    patterns: Vec<String>,
    package_manager: Option<String>,
}

fn read_package(
    directory: &Path,
    budget: &mut Budget,
) -> Result<Option<PackageFile>, DiscoveryError> {
    let path = directory.join("package.json");
    if !path.exists() {
        return Ok(None);
    }
    let bytes = budget.read(&path)?;
    let value = unique_json(&bytes).map_err(|_| DiscoveryError::Malformed)?;
    let object = value.as_object().ok_or(DiscoveryError::Malformed)?;
    let scripts = object
        .get("scripts")
        .map(parse_string_map)
        .transpose()?
        .unwrap_or_default();
    let name = object.get("name").map(parse_string).transpose()?;
    let package_manager = object.get("packageManager").map(parse_string).transpose()?;
    let mut patterns = Vec::new();
    if let Some(workspaces) = object.get("workspaces") {
        let list = if let Some(list) = workspaces.as_array() {
            list
        } else {
            workspaces
                .get("packages")
                .and_then(serde_json::Value::as_array)
                .ok_or(DiscoveryError::Malformed)?
        };
        let workspace_patterns = list
            .iter()
            .map(|v| {
                v.as_str()
                    .filter(|s| s.len() <= 256)
                    .map(str::to_owned)
                    .ok_or(DiscoveryError::Malformed)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if workspace_patterns.len() > 1024 {
            return Err(DiscoveryError::Limit);
        }
        patterns.extend(workspace_patterns);
    }
    let mut dependencies = HashSet::new();
    for section in ["dependencies", "devDependencies", "optionalDependencies"] {
        if let Some(values) = object.get(section) {
            let values = values.as_object().ok_or(DiscoveryError::Malformed)?;
            dependencies.extend(values.keys().cloned());
        }
    }
    Ok(Some(PackageFile {
        name,
        scripts,
        dependencies,
        patterns,
        package_manager,
    }))
}

fn parse_string(value: &serde_json::Value) -> Result<String, DiscoveryError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or(DiscoveryError::Malformed)
}

fn parse_string_map(value: &serde_json::Value) -> Result<BTreeMap<String, String>, DiscoveryError> {
    let object = value.as_object().ok_or(DiscoveryError::Malformed)?;
    object
        .iter()
        .map(|(k, v)| Ok((k.clone(), parse_string(v)?)))
        .collect()
}

struct UniqueJson(serde_json::Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct JsonVisitor;
        impl<'de> Visitor<'de> for JsonVisitor {
            type Value = UniqueJson;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON value without duplicate object keys")
            }
            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|number| UniqueJson(number.into()))
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueJson(serde_json::Value::Null))
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                self.visit_unit()
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element::<UniqueJson>()? {
                    values.push(value.0);
                }
                Ok(UniqueJson(values.into()))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(serde::de::Error::custom("duplicate JSON object key"));
                    }
                    let value = map.next_value::<UniqueJson>()?;
                    values.insert(key, value.0);
                }
                Ok(UniqueJson(values.into()))
            }
        }
        deserializer.deserialize_any(JsonVisitor)
    }
}

fn unique_json(bytes: &[u8]) -> Result<serde_json::Value, serde_json::Error> {
    let value: UniqueJson = serde_json::from_slice(bytes)?;
    Ok(value.0)
}

fn validate_yaml(source: &str) -> Result<(), DiscoveryError> {
    use std::collections::HashSet;
    use yaml_rust2::parser::{Event, EventReceiver, Parser};
    enum Container {
        Mapping {
            keys: HashSet<String>,
            expecting_key: bool,
        },
        Sequence,
    }
    #[derive(Default)]
    struct StrictYaml {
        invalid: bool,
        documents: usize,
        containers: Vec<Container>,
    }
    impl StrictYaml {
        fn consume_value(&mut self) {
            if let Some(Container::Mapping { expecting_key, .. }) = self.containers.last_mut() {
                if *expecting_key {
                    self.invalid = true;
                }
                *expecting_key = true;
            }
        }
    }
    impl EventReceiver for StrictYaml {
        fn on_event(&mut self, event: Event) {
            match event {
                Event::DocumentStart => self.documents += 1,
                Event::Alias(_) => self.invalid = true,
                Event::Scalar(key, _, anchor, tag) => {
                    if anchor != 0 || tag.is_some() {
                        self.invalid = true;
                    }
                    if let Some(Container::Mapping {
                        keys,
                        expecting_key,
                    }) = self.containers.last_mut()
                    {
                        if *expecting_key {
                            if !keys.insert(key) {
                                self.invalid = true;
                            }
                            *expecting_key = false;
                        } else {
                            *expecting_key = true;
                        }
                    }
                }
                Event::SequenceStart(anchor, tag) => {
                    if anchor != 0 || tag.is_some() {
                        self.invalid = true;
                    }
                    self.consume_value();
                    self.containers.push(Container::Sequence);
                }
                Event::MappingStart(anchor, tag) => {
                    if anchor != 0 || tag.is_some() {
                        self.invalid = true;
                    }
                    self.consume_value();
                    self.containers.push(Container::Mapping {
                        keys: HashSet::new(),
                        expecting_key: true,
                    });
                }
                Event::MappingEnd | Event::SequenceEnd => {
                    self.containers.pop();
                }
                _ => {}
            }
        }
    }
    let mut strict = StrictYaml::default();
    Parser::new_from_str(source)
        .load(&mut strict, true)
        .map_err(|_| DiscoveryError::Malformed)?;
    if strict.invalid || strict.documents != 1 || !strict.containers.is_empty() {
        return Err(DiscoveryError::Malformed);
    }
    Ok(())
}

struct PnpmWorkspaceSettings {
    patterns: Vec<String>,
    enable_pre_post_scripts: Option<bool>,
}

fn parse_pnpm_workspace(bytes: &[u8]) -> Result<PnpmWorkspaceSettings, DiscoveryError> {
    use yaml_rust2::{YamlLoader, yaml::Yaml};
    let source = std::str::from_utf8(bytes).map_err(|_| DiscoveryError::Malformed)?;
    validate_yaml(source)?;
    let docs = YamlLoader::load_from_str(source).map_err(|_| DiscoveryError::Malformed)?;
    if docs.len() != 1 {
        return Err(DiscoveryError::Malformed);
    }
    let root = docs[0].as_hash().ok_or(DiscoveryError::Malformed)?;
    let packages = root.get(&Yaml::String("packages".into()));
    let patterns = if let Some(packages) = packages {
        let list = packages.as_vec().ok_or(DiscoveryError::Malformed)?;
        list.iter()
            .map(|item| {
                item.as_str()
                    .filter(|s| s.len() <= 256)
                    .map(str::to_owned)
                    .ok_or(DiscoveryError::Malformed)
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let enable_pre_post_scripts = root
        .get(&Yaml::String("enablePrePostScripts".into()))
        .map(|value| value.as_bool().ok_or(DiscoveryError::Malformed))
        .transpose()?;
    Ok(PnpmWorkspaceSettings {
        patterns,
        enable_pre_post_scripts,
    })
}

fn discover_packages(
    root: &Path,
    patterns: &[String],
    projects: &mut Vec<Project>,
    budget: &mut Budget,
) -> Result<(), DiscoveryError> {
    let mut include_builder = GlobSetBuilder::new();
    let mut exclude_builder = GlobSetBuilder::new();
    let mut include_count = 0usize;
    for pattern in patterns {
        let (builder, pattern) = if let Some(pattern) = pattern.strip_prefix('!') {
            (&mut exclude_builder, pattern)
        } else {
            include_count += 1;
            (&mut include_builder, pattern.as_str())
        };
        builder.add(
            GlobBuilder::new(pattern)
                .literal_separator(false)
                .build()
                .map_err(|_| DiscoveryError::Malformed)?,
        );
    }
    let matcher = include_builder
        .build()
        .map_err(|_| DiscoveryError::Malformed)?;
    let exclude_matcher = exclude_builder
        .build()
        .map_err(|_| DiscoveryError::Malformed)?;
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut seen = HashSet::new();
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(DiscoveryError::Limit);
        }
        let canonical = fs::canonicalize(&directory).map_err(|_| DiscoveryError::Io)?;
        if !canonical.starts_with(root) || !seen.insert(canonical.clone()) {
            continue;
        }
        for entry in fs::read_dir(&canonical).map_err(|_| DiscoveryError::Io)? {
            budget.entry()?;
            let entry = entry.map_err(|_| DiscoveryError::Io)?;
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(".git" | "node_modules" | "target" | ".hg" | ".svn")
            ) {
                continue;
            }
            let path = entry.path();
            let file_type = entry.file_type().map_err(|_| DiscoveryError::Io)?;
            if file_type.is_dir() || file_type.is_symlink() && path.is_dir() {
                pending.push((path.clone(), depth + 1));
            }
            if file_type.is_file() && path.file_name().is_some_and(|name| name == "package.json") {
                let parent = path.parent().ok_or(DiscoveryError::Malformed)?;
                let relative = parent.strip_prefix(root).unwrap_or(Path::new("."));
                let rel = relative.to_string_lossy().replace('\\', "/");
                if include_count > 0 && matcher.is_match(&rel) && !exclude_matcher.is_match(&rel) {
                    add_project_path(projects, parent, root, budget)?;
                }
            }
        }
    }
    Ok(())
}

fn add_project_path(
    projects: &mut Vec<Project>,
    path: &Path,
    root: &Path,
    budget: &mut Budget,
) -> Result<(), DiscoveryError> {
    let canonical = fs::canonicalize(path).map_err(|_| DiscoveryError::Io)?;
    if !canonical.starts_with(root) {
        return Err(DiscoveryError::Malformed);
    }
    if projects.iter().any(|project| project.path == canonical) {
        return Ok(());
    }
    if projects.len() >= MAX_PROJECTS {
        return Err(DiscoveryError::Limit);
    }
    let package = read_package(&canonical, budget)?;
    let mut name = package.as_ref().and_then(|p| p.name.clone());
    let mut dependencies = package
        .as_ref()
        .map(|p| p.dependencies.clone())
        .unwrap_or_default();
    let cargo = canonical.join("Cargo.toml");
    if cargo.exists() {
        let bytes = budget.read(&cargo)?;
        let source = std::str::from_utf8(&bytes).map_err(|_| DiscoveryError::Malformed)?;
        let manifest: toml::Value =
            toml::from_str(source).map_err(|_| DiscoveryError::Malformed)?;
        if name.is_none() {
            name = manifest
                .get("package")
                .and_then(|v| v.get("name"))
                .and_then(toml::Value::as_str)
                .map(str::to_owned);
        }
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(deps) = manifest.get(section).and_then(toml::Value::as_table) {
                dependencies.extend(deps.keys().cloned());
            }
        }
    }
    let go_mod = canonical.join("go.mod");
    if go_mod.exists() {
        let bytes = budget.read(&go_mod)?;
        let source = std::str::from_utf8(&bytes).map_err(|_| DiscoveryError::Malformed)?;
        for line in source.lines() {
            let line = line.trim();
            if name.is_none()
                && let Some(module) = line.strip_prefix("module ")
            {
                name = Some(module.trim().trim_matches(['"', '`']).to_owned());
            }
            if let Some(require) = line.strip_prefix("require ")
                && let Some(module) = require.split_ascii_whitespace().next()
            {
                dependencies.insert(module.to_owned());
            }
        }
    }
    let project = Project {
        path: canonical,
        name,
        aliases: Vec::new(),
        scripts: package
            .as_ref()
            .map(|p| p.scripts.clone())
            .unwrap_or_default(),
        nx_targets: BTreeMap::new(),
        dependencies,
        moon_tasks: BTreeMap::new(),
    };
    projects.push(project);
    Ok(())
}

fn load_php_jvm_manifests(
    root: &Path,
    hints: &mut WorkspaceHints,
    budget: &mut Budget,
) -> Result<(), DiscoveryError> {
    let composer = root.join("composer.json");
    if composer.exists() {
        let bytes = budget.read(&composer)?;
        let value = unique_json(&bytes).map_err(|_| DiscoveryError::Malformed)?;
        let object = value.as_object().ok_or(DiscoveryError::Malformed)?;
        hints.has_composer_manifest = true;
        if let Some(scripts) = object.get("scripts") {
            let scripts = scripts.as_object().ok_or(DiscoveryError::Malformed)?;
            for (name, value) in scripts {
                if name.is_empty() || name.len() > 128 {
                    return Err(DiscoveryError::Malformed);
                }
                let body = if let Some(command) = value.as_str() {
                    command.to_owned()
                } else if let Some(commands) = value.as_array() {
                    let mut lines = Vec::with_capacity(commands.len());
                    for command in commands {
                        lines.push(
                            command
                                .as_str()
                                .filter(|command| command.len() <= 4096)
                                .ok_or(DiscoveryError::Malformed)?
                                .to_owned(),
                        );
                    }
                    lines.join("\n")
                } else {
                    return Err(DiscoveryError::Malformed);
                };
                if body.len() > 16 * 1024 {
                    return Err(DiscoveryError::Limit);
                }
                hints.composer_scripts.insert(name.clone(), body);
            }
        }
    }

    let pom = root.join("pom.xml");
    if pom.exists() {
        let bytes = budget.read(&pom)?;
        let source = std::str::from_utf8(&bytes).map_err(|_| DiscoveryError::Malformed)?;
        hints.maven_modules = parse_maven_modules(source)?;
    }

    let groovy = root.join("settings.gradle");
    let kotlin = root.join("settings.gradle.kts");
    if groovy.exists() && kotlin.exists() {
        return Err(DiscoveryError::Malformed);
    }
    let settings = if groovy.exists() { groovy } else { kotlin };
    if settings.exists() {
        let bytes = budget.read(&settings)?;
        let source = std::str::from_utf8(&bytes).map_err(|_| DiscoveryError::Malformed)?;
        let (modules, dynamic) = parse_gradle_settings(source)?;
        hints.gradle_modules = modules;
        hints.gradle_dynamic = dynamic;
    }
    Ok(())
}

fn parse_maven_modules(source: &str) -> Result<Vec<String>, DiscoveryError> {
    let mut reader = XmlReader::from_str(source);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<String> = Vec::new();
    let mut modules = Vec::new();
    let mut in_modules = false;
    let mut saw_project = false;
    let mut module_text: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(XmlEvent::Start(element)) => {
                let name = element.local_name().as_ref().to_ascii_lowercase();
                if stack.is_empty() {
                    if name != "project" {
                        return Err(DiscoveryError::Malformed);
                    }
                    saw_project = true;
                }
                if name == "modules" {
                    if in_modules || stack.len() != 1 {
                        return Err(DiscoveryError::Malformed);
                    }
                    in_modules = true;
                }
                if name == "module" && in_modules {
                    if stack.len() != 2 || module_text.is_some() {
                        return Err(DiscoveryError::Malformed);
                    }
                    module_text = Some(String::new());
                }
                stack.push(name);
            }
            Ok(XmlEvent::Empty(element)) => {
                let local_name = element.local_name();
                let name = local_name.as_ref();
                if name.eq_ignore_ascii_case("module") && in_modules {
                    return Err(DiscoveryError::Malformed);
                }
                if stack.is_empty() {
                    if !name.eq_ignore_ascii_case("project") || saw_project {
                        return Err(DiscoveryError::Malformed);
                    }
                    saw_project = true;
                }
            }
            Ok(XmlEvent::Text(text))
                if module_text.is_some()
                    && stack.len() == 3
                    && stack.last().is_some_and(|name| name == "module") =>
            {
                let content = quick_xml::escape::unescape(text.as_ref())
                    .map_err(|_| DiscoveryError::Malformed)?
                    .into_owned();
                module_text.as_mut().unwrap().push_str(&content);
            }
            Ok(XmlEvent::End(element)) => {
                let name = element.local_name().as_ref().to_ascii_lowercase();
                if name == "module" && in_modules {
                    let module = module_text.take().ok_or(DiscoveryError::Malformed)?;
                    let module = module.trim().to_owned();
                    let path = Path::new(&module);
                    if module.is_empty()
                        || module.len() > 512
                        || path.is_absolute()
                        || path.components().any(|component| {
                            matches!(
                                component,
                                std::path::Component::ParentDir
                                    | std::path::Component::CurDir
                                    | std::path::Component::RootDir
                                    | std::path::Component::Prefix(_)
                            )
                        })
                        || module.chars().any(|ch| matches!(ch, '$' | '&' | '\\'))
                    {
                        return Err(DiscoveryError::Malformed);
                    }
                    modules.push(module);
                    if modules.len() > MAX_PROJECTS {
                        return Err(DiscoveryError::Limit);
                    }
                }
                if name == "modules" {
                    if !in_modules {
                        return Err(DiscoveryError::Malformed);
                    }
                    in_modules = false;
                }
                if stack.pop().as_deref() != Some(name.as_str()) {
                    return Err(DiscoveryError::Malformed);
                }
            }
            Ok(XmlEvent::CData(_)) if module_text.is_some() => {
                return Err(DiscoveryError::Malformed);
            }
            Ok(XmlEvent::GeneralRef(_)) if module_text.is_some() => {
                return Err(DiscoveryError::Malformed);
            }
            Ok(XmlEvent::DocType(_)) => return Err(DiscoveryError::Malformed),
            Ok(XmlEvent::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(DiscoveryError::Malformed),
        }
    }
    if !stack.is_empty() || in_modules || module_text.is_some() || !saw_project {
        return Err(DiscoveryError::Malformed);
    }
    modules.sort();
    if modules.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(DiscoveryError::Malformed);
    }
    Ok(modules)
}

fn parse_gradle_settings(source: &str) -> Result<(Vec<String>, bool), DiscoveryError> {
    let mut modules = Vec::new();
    let mut dynamic = false;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with('*') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("includeBuild") {
            if !rest.is_empty() {
                dynamic = true;
            }
            continue;
        }
        let Some(rest) = line.strip_prefix("include") else {
            if line.contains("include(") || line.contains("include ") {
                dynamic = true;
            }
            continue;
        };
        if !rest.starts_with(char::is_whitespace) && !rest.starts_with('(') {
            continue;
        }
        let mut remaining = rest.trim_start().trim_start_matches('(').trim();
        if remaining.starts_with(['\'', '"']) {
            while !remaining.is_empty() {
                let Some(quote) = remaining.chars().next() else {
                    break;
                };
                if !matches!(quote, '\'' | '"') {
                    if remaining.starts_with(',') || remaining.starts_with(')') {
                        remaining = remaining[1..].trim_start();
                        continue;
                    }
                    dynamic = true;
                    break;
                }
                remaining = &remaining[quote.len_utf8()..];
                let Some(end) = remaining.find(quote) else {
                    return Err(DiscoveryError::Malformed);
                };
                let project = &remaining[..end];
                if project.is_empty()
                    || project.contains(['$', '{', '}', '\\'])
                    || project.len() > 256
                {
                    return Err(DiscoveryError::Malformed);
                }
                let normalized = project.trim_start_matches(':').replace(':', "/");
                if normalized
                    .split('/')
                    .any(|part| part == ".." || part.is_empty())
                {
                    return Err(DiscoveryError::Malformed);
                }
                modules.push(normalized);
                remaining = remaining[end + quote.len_utf8()..].trim_start();
                if remaining.is_empty() || remaining.starts_with(')') {
                    break;
                }
                if remaining.starts_with(',') {
                    remaining = remaining[1..].trim_start();
                    continue;
                }
                dynamic = true;
                break;
            }
        } else {
            dynamic = true;
        }
        if modules.len() > MAX_PROJECTS {
            return Err(DiscoveryError::Limit);
        }
    }
    modules.sort();
    if modules.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(DiscoveryError::Malformed);
    }
    Ok((modules, dynamic))
}

fn load_runner_manifests(
    root: &Path,
    hints: &mut WorkspaceHints,
    budget: &mut Budget,
) -> Result<(), DiscoveryError> {
    for (file, target) in [
        ("turbo.json", &mut hints.turbo_tasks),
        ("nx.json", &mut hints.nx_targets),
        ("lerna.json", &mut hints.lerna_tasks),
    ] {
        let path = root.join(file);
        if path.exists() {
            let bytes = budget.read(&path)?;
            let value = unique_json(&bytes).map_err(|_| DiscoveryError::Malformed)?;
            collect_task_names(&value, target);
        }
    }
    for file in [
        "moon.yml",
        "moon.yaml",
        ".moon/workspace.yml",
        ".moon/workspace.yaml",
    ] {
        let path = root.join(file);
        if path.exists() {
            let bytes = budget.read(&path)?;
            let source = std::str::from_utf8(&bytes).map_err(|_| DiscoveryError::Malformed)?;
            validate_yaml(source)?;
            let docs = yaml_rust2::YamlLoader::load_from_str(source)
                .map_err(|_| DiscoveryError::Malformed)?;
            if docs.len() != 1 || docs[0].as_hash().is_none() {
                return Err(DiscoveryError::Malformed);
            }
            collect_yaml_task_names(&docs[0], &mut hints.moon_tasks);
        }
    }
    if root.join("nx.json").exists() {
        discover_nx_projects(root, hints, budget)?;
    }
    if [".moon/workspace.yml", ".moon/workspace.yaml"]
        .iter()
        .any(|file| root.join(file).exists())
    {
        discover_moon_projects(root, hints, budget)?;
    }
    Ok(())
}

fn discover_nx_projects(
    root: &Path,
    hints: &mut WorkspaceHints,
    budget: &mut Budget,
) -> Result<(), DiscoveryError> {
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut seen = HashSet::new();
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(DiscoveryError::Limit);
        }
        let canonical = fs::canonicalize(&directory).map_err(|_| DiscoveryError::Io)?;
        if !canonical.starts_with(root) || !seen.insert(canonical.clone()) {
            continue;
        }
        for entry in fs::read_dir(&canonical).map_err(|_| DiscoveryError::Io)? {
            budget.entry()?;
            let entry = entry.map_err(|_| DiscoveryError::Io)?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|_| DiscoveryError::Io)?;
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(".git" | "node_modules" | "target" | ".hg" | ".svn")
            ) {
                continue;
            }
            if file_type.is_dir() || file_type.is_symlink() && path.is_dir() {
                pending.push((path.clone(), depth + 1));
            }
            if file_type.is_file() && name == "project.json" {
                let bytes = budget.read(&path)?;
                let value = unique_json(&bytes).map_err(|_| DiscoveryError::Malformed)?;
                let object = value.as_object().ok_or(DiscoveryError::Malformed)?;
                let declared_root = object.get("root").map(parse_string).transpose()?;
                let project_path = if let Some(relative) = declared_root {
                    root.join(relative)
                } else {
                    path.parent()
                        .ok_or(DiscoveryError::Malformed)?
                        .to_path_buf()
                };
                let project_path =
                    fs::canonicalize(project_path).map_err(|_| DiscoveryError::Io)?;
                if !project_path.starts_with(root) {
                    return Err(DiscoveryError::Malformed);
                }
                let name = object.get("name").map(parse_string).transpose()?;
                let targets = object.get("targets").and_then(serde_json::Value::as_object);
                let mut nx_targets = BTreeMap::new();
                if let Some(targets) = targets {
                    for (target_name, target) in targets {
                        let command = nx_target_command(target)
                            .unwrap_or_else(|| "__ttc_unknown_nx_target__".to_owned());
                        nx_targets.insert(target_name.clone(), command);
                    }
                }
                if let Some(existing) = hints
                    .projects
                    .iter_mut()
                    .find(|project| project.path == project_path)
                {
                    if let (Some(previous), Some(current)) = (&existing.name, &name)
                        && previous != current
                    {
                        return Err(DiscoveryError::Malformed);
                    }
                    if existing.name.is_none() {
                        existing.name = name;
                    }
                    existing.nx_targets.extend(nx_targets);
                } else {
                    if hints.projects.len() >= MAX_PROJECTS {
                        return Err(DiscoveryError::Limit);
                    }
                    hints.projects.push(Project {
                        path: project_path,
                        name,
                        aliases: Vec::new(),
                        scripts: BTreeMap::new(),
                        nx_targets,
                        dependencies: HashSet::new(),
                        moon_tasks: BTreeMap::new(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn discover_moon_projects(
    root: &Path,
    hints: &mut WorkspaceHints,
    budget: &mut Budget,
) -> Result<(), DiscoveryError> {
    use yaml_rust2::yaml::Yaml;
    let workspace_path = [".moon/workspace.yml", ".moon/workspace.yaml"]
        .iter()
        .map(|file| root.join(file))
        .find(|path| path.exists())
        .ok_or(DiscoveryError::Malformed)?;
    let workspace = read_yaml(&workspace_path, budget)?;
    let workspace_map = workspace.as_hash().ok_or(DiscoveryError::Malformed)?;
    let mut declared: BTreeMap<PathBuf, String> = BTreeMap::new();
    if let Some(projects) = workspace_map.get(&Yaml::String("projects".into())) {
        if let Some(map) = projects.as_hash() {
            for (name, path) in map {
                if let (Some(name), Some(path)) = (name.as_str(), path.as_str()) {
                    if name == "globFormat" {
                        continue;
                    }
                    declared.insert(canonical_workspace_path(root, path)?, name.to_owned());
                } else if let Some(field) = name.as_str() {
                    if matches!(field, "globs" | "sources") {
                        let values = path;
                        if let Some(items) = values.as_vec() {
                            for item in items {
                                let pattern = item.as_str().ok_or(DiscoveryError::Malformed)?;
                                for directory in expand_directories(root, pattern, budget)? {
                                    let name = directory
                                        .strip_prefix(root)
                                        .unwrap_or(&directory)
                                        .file_name()
                                        .and_then(|value| value.to_str())
                                        .ok_or(DiscoveryError::Malformed)?
                                        .to_owned();
                                    declared.entry(directory).or_insert(name);
                                }
                            }
                        } else if let Some(sources) = values.as_hash() {
                            for (name, path) in sources {
                                let name = name.as_str().ok_or(DiscoveryError::Malformed)?;
                                let path = path.as_str().ok_or(DiscoveryError::Malformed)?;
                                declared
                                    .insert(canonical_workspace_path(root, path)?, name.to_owned());
                            }
                        } else {
                            return Err(DiscoveryError::Malformed);
                        }
                    }
                } else {
                    return Err(DiscoveryError::Malformed);
                }
            }
        } else if let Some(list) = projects.as_vec() {
            for item in list {
                let pattern = item.as_str().ok_or(DiscoveryError::Malformed)?;
                for directory in expand_directories(root, pattern, budget)? {
                    let name = directory
                        .strip_prefix(root)
                        .unwrap_or(&directory)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or(DiscoveryError::Malformed)?
                        .to_owned();
                    declared.entry(directory).or_insert(name);
                }
            }
        } else {
            return Err(DiscoveryError::Malformed);
        }
    } else {
        return Err(DiscoveryError::Malformed);
    }

    let mut global_tasks = BTreeMap::new();
    let tasks_root = root.join(".moon/tasks");
    if tasks_root.exists() {
        let mut pending = vec![(tasks_root, 0usize)];
        while let Some((directory, depth)) = pending.pop() {
            if depth > MAX_DEPTH {
                return Err(DiscoveryError::Limit);
            }
            for entry in fs::read_dir(&directory).map_err(|_| DiscoveryError::Io)? {
                budget.entry()?;
                let entry = entry.map_err(|_| DiscoveryError::Io)?;
                let path = entry.path();
                if entry.file_type().map_err(|_| DiscoveryError::Io)?.is_dir() {
                    pending.push((path, depth + 1));
                } else if path
                    .extension()
                    .is_some_and(|extension| matches!(extension.to_str(), Some("yml" | "yaml")))
                {
                    let value = read_yaml(&path, budget)?;
                    global_tasks.extend(yaml_task_commands(&value)?);
                }
            }
        }
    }

    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut seen = HashSet::new();
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(DiscoveryError::Limit);
        }
        let canonical = fs::canonicalize(&directory).map_err(|_| DiscoveryError::Io)?;
        if !canonical.starts_with(root) || !seen.insert(canonical.clone()) {
            continue;
        }
        for entry in fs::read_dir(&canonical).map_err(|_| DiscoveryError::Io)? {
            budget.entry()?;
            let entry = entry.map_err(|_| DiscoveryError::Io)?;
            let path = entry.path();
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(".git" | "node_modules" | "target" | ".hg" | ".svn" | ".moon")
            ) {
                continue;
            }
            let file_type = entry.file_type().map_err(|_| DiscoveryError::Io)?;
            if file_type.is_dir() || file_type.is_symlink() && path.is_dir() {
                pending.push((path.clone(), depth + 1));
            }
            if file_type.is_file() && matches!(name.to_str(), Some("moon.yml" | "moon.yaml")) {
                let project_path = path
                    .parent()
                    .ok_or(DiscoveryError::Malformed)?
                    .to_path_buf();
                let value = read_yaml(&path, budget)?;
                let project_map = value.as_hash().ok_or(DiscoveryError::Malformed)?;
                let mut tasks = global_tasks.clone();
                tasks.extend(yaml_task_commands(&value)?);
                let project_name = project_map
                    .get(&Yaml::String("project".into()))
                    .and_then(Yaml::as_hash)
                    .and_then(|map| map.get(&Yaml::String("name".into())))
                    .and_then(Yaml::as_str)
                    .map(str::to_owned);
                let declared_name = declared.get(&project_path).cloned().or(project_name);
                add_moon_project(hints, &project_path, declared_name, tasks)?;
            }
        }
    }
    for (path, name) in declared {
        if !hints.projects.iter().any(|project| project.path == path) {
            add_moon_project(hints, &path, Some(name), global_tasks.clone())?;
        } else if let Some(project) = hints
            .projects
            .iter_mut()
            .find(|project| project.path == path)
        {
            add_project_alias(project, name)?;
            for (task, command) in &global_tasks {
                project
                    .moon_tasks
                    .entry(task.clone())
                    .or_insert(command.clone());
            }
        }
    }
    Ok(())
}

fn add_moon_project(
    hints: &mut WorkspaceHints,
    path: &Path,
    name: Option<String>,
    tasks: BTreeMap<String, String>,
) -> Result<(), DiscoveryError> {
    let canonical = fs::canonicalize(path).map_err(|_| DiscoveryError::Io)?;
    if !canonical.starts_with(&hints.root) {
        return Err(DiscoveryError::Malformed);
    }
    if let Some(project) = hints
        .projects
        .iter_mut()
        .find(|project| project.path == canonical)
    {
        if let Some(name) = name {
            add_project_alias(project, name)?;
        }
        project.moon_tasks.extend(tasks);
        return Ok(());
    }
    if hints.projects.len() >= MAX_PROJECTS {
        return Err(DiscoveryError::Limit);
    }
    hints.projects.push(Project {
        path: canonical,
        name,
        aliases: Vec::new(),
        scripts: BTreeMap::new(),
        nx_targets: BTreeMap::new(),
        dependencies: HashSet::new(),
        moon_tasks: tasks,
    });
    Ok(())
}

fn add_project_alias(project: &mut Project, alias: String) -> Result<(), DiscoveryError> {
    if alias.is_empty() || alias.len() > 256 || alias.chars().any(char::is_control) {
        return Err(DiscoveryError::Malformed);
    }
    if project.name.is_none() {
        project.name = Some(alias);
    } else if project.name.as_deref() != Some(&alias) && !project.aliases.contains(&alias) {
        project.aliases.push(alias);
    }
    Ok(())
}

fn canonical_workspace_path(root: &Path, relative: &str) -> Result<PathBuf, DiscoveryError> {
    let path = fs::canonicalize(root.join(relative)).map_err(|_| DiscoveryError::Io)?;
    if !path.starts_with(root) {
        return Err(DiscoveryError::Malformed);
    }
    Ok(path)
}

fn read_yaml(path: &Path, budget: &mut Budget) -> Result<yaml_rust2::Yaml, DiscoveryError> {
    let bytes = budget.read(path)?;
    let source = std::str::from_utf8(&bytes).map_err(|_| DiscoveryError::Malformed)?;
    validate_yaml(source)?;
    let documents =
        yaml_rust2::YamlLoader::load_from_str(source).map_err(|_| DiscoveryError::Malformed)?;
    if documents.len() != 1 {
        return Err(DiscoveryError::Malformed);
    }
    Ok(documents[0].clone())
}

fn yaml_task_commands(
    value: &yaml_rust2::Yaml,
) -> Result<BTreeMap<String, String>, DiscoveryError> {
    use yaml_rust2::yaml::Yaml;
    let Some(tasks) = value
        .as_hash()
        .and_then(|map| map.get(&Yaml::String("tasks".into())))
    else {
        return Ok(BTreeMap::new());
    };
    let tasks = tasks.as_hash().ok_or(DiscoveryError::Malformed)?;
    let mut result = BTreeMap::new();
    for (name, task) in tasks {
        let name = name.as_str().ok_or(DiscoveryError::Malformed)?.to_owned();
        let command = if let Some(command) = task.as_str() {
            Some(command.to_owned())
        } else if let Some(map) = task.as_hash() {
            let command = match map.get(&Yaml::String("command".into())) {
                Some(Yaml::String(command)) => Some(command.clone()),
                Some(Yaml::Array(words)) => Some(
                    words
                        .iter()
                        .map(|word| {
                            word.as_str()
                                .map(shell_quote)
                                .ok_or(DiscoveryError::Malformed)
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .join(" "),
                ),
                Some(_) => return Err(DiscoveryError::Malformed),
                None => None,
            };
            let args = map.get(&Yaml::String("args".into()));
            match (command, args) {
                (Some(command), Some(Yaml::Array(args))) => {
                    let args = args
                        .iter()
                        .map(Yaml::as_str)
                        .collect::<Option<Vec<_>>>()
                        .ok_or(DiscoveryError::Malformed)?;
                    Some(format!(
                        "{command} {}",
                        args.iter()
                            .map(|arg| shell_quote(arg))
                            .collect::<Vec<_>>()
                            .join(" ")
                    ))
                }
                (Some(command), _) => Some(command),
                (None, _) => None,
            }
        } else {
            None
        };
        result.insert(
            name,
            command.unwrap_or_else(|| "__ttc_unknown_moon_task__".into()),
        );
    }
    Ok(result)
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./:@+-".contains(&byte))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn expand_directories(
    root: &Path,
    pattern: &str,
    budget: &mut Budget,
) -> Result<Vec<PathBuf>, DiscoveryError> {
    if pattern.len() > 256 {
        return Err(DiscoveryError::Limit);
    }
    let matcher = GlobBuilder::new(pattern)
        .literal_separator(false)
        .build()
        .map_err(|_| DiscoveryError::Malformed)?
        .compile_matcher();
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(DiscoveryError::Limit);
        }
        let canonical = fs::canonicalize(&directory).map_err(|_| DiscoveryError::Io)?;
        if !canonical.starts_with(root) || !seen.insert(canonical.clone()) {
            continue;
        }
        for entry in fs::read_dir(&canonical).map_err(|_| DiscoveryError::Io)? {
            budget.entry()?;
            let entry = entry.map_err(|_| DiscoveryError::Io)?;
            let path = entry.path();
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(".git" | "node_modules" | "target" | ".hg" | ".svn")
            ) {
                continue;
            }
            let kind = entry.file_type().map_err(|_| DiscoveryError::Io)?;
            if kind.is_dir() || kind.is_symlink() && path.is_dir() {
                let canonical_child = fs::canonicalize(&path).map_err(|_| DiscoveryError::Io)?;
                let relative = canonical_child
                    .strip_prefix(root)
                    .unwrap_or(&canonical_child);
                let text = relative.to_string_lossy().replace('\\', "/");
                if matcher.is_match(&text)
                    || [
                        "moon.yml",
                        "moon.yaml",
                        "project.json",
                        "package.json",
                        "Cargo.toml",
                        "go.mod",
                    ]
                    .iter()
                    .any(|manifest| matcher.is_match(format!("{text}/{manifest}")))
                {
                    found.push(canonical_child);
                }
                pending.push((path, depth + 1));
            }
        }
    }
    Ok(found)
}

fn nx_target_command(target: &serde_json::Value) -> Option<String> {
    let object = target.as_object()?;
    if let Some(command) = object.get("command").and_then(serde_json::Value::as_str) {
        return Some(command.to_owned());
    }
    let options = object.get("options").and_then(serde_json::Value::as_object);
    if let Some(command) = options
        .and_then(|options| options.get("command"))
        .and_then(serde_json::Value::as_str)
    {
        return Some(command.to_owned());
    }
    if let Some(commands) = options
        .and_then(|options| options.get("commands"))
        .and_then(serde_json::Value::as_array)
    {
        let values = commands
            .iter()
            .map(|command| {
                command
                    .as_object()?
                    .get("command")?
                    .as_str()
                    .map(str::to_owned)
            })
            .collect::<Option<Vec<_>>>()?;
        return Some(values.join(" && "));
    }
    let executor = object.get("executor").and_then(serde_json::Value::as_str)?;
    let command = match executor {
        executor if executor.contains("jest") => "jest",
        executor if executor.contains("vitest") => "vitest run",
        executor if executor.contains("playwright") => "playwright test",
        executor if executor.contains("cypress") => "cypress run",
        executor if executor.contains("eslint") => "eslint .",
        executor if executor.contains("tsc") || executor.contains("typecheck") => "tsc --noEmit",
        executor if executor.contains("webpack") => "webpack",
        executor if executor.contains("vite") && executor.contains("build") => "vite build",
        executor if executor.contains("vite") => "vitest run",
        executor if executor.contains("rollup") => "rollup",
        executor if executor.contains("esbuild") => "esbuild",
        _ => return None,
    };
    Some(command.to_owned())
}

fn collect_task_names(value: &serde_json::Value, names: &mut HashSet<String>) {
    if let Some(object) = value.as_object() {
        for key in ["tasks", "targets", "scripts"] {
            if let Some(items) = object.get(key).and_then(serde_json::Value::as_object) {
                names.extend(items.keys().cloned());
            }
        }
        for child in object.values() {
            collect_task_names(child, names);
        }
    } else if let Some(array) = value.as_array() {
        for child in array {
            collect_task_names(child, names);
        }
    }
}

fn collect_yaml_task_names(value: &yaml_rust2::Yaml, names: &mut HashSet<String>) {
    use yaml_rust2::yaml::Yaml;
    if let Some(map) = value.as_hash() {
        for key in ["tasks", "targets"] {
            if let Some(tasks) = map.get(&Yaml::String(key.into())).and_then(Yaml::as_hash) {
                names.extend(tasks.keys().filter_map(Yaml::as_str).map(str::to_owned));
            }
        }
        for child in map.values() {
            collect_yaml_task_names(child, names);
        }
    } else if let Some(array) = value.as_vec() {
        for child in array {
            collect_yaml_task_names(child, names);
        }
    }
}

fn string_array(value: Option<&toml::Value>) -> Result<Vec<String>, DiscoveryError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or(DiscoveryError::Malformed)?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or(DiscoveryError::Malformed)
        })
        .collect()
}

fn parse_go_work(bytes: &[u8]) -> Result<Vec<String>, DiscoveryError> {
    let text = std::str::from_utf8(bytes).map_err(|_| DiscoveryError::Malformed)?;
    let mut paths = Vec::new();
    let mut in_use = false;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line == "use (" {
            in_use = true;
            continue;
        }
        if in_use && line == ")" {
            in_use = false;
            continue;
        }
        let value = if in_use {
            line
        } else if let Some(value) = line.strip_prefix("use ") {
            value.trim()
        } else {
            continue;
        };
        let value = value.trim_matches(['"', '`']);
        if value.is_empty() || value.starts_with('-') {
            return Err(DiscoveryError::Malformed);
        }
        paths.push(value.to_owned());
    }
    if in_use {
        return Err(DiscoveryError::Malformed);
    }
    Ok(paths)
}

fn expand_project_pattern(
    root: &Path,
    pattern: &str,
    budget: &mut Budget,
) -> Result<Vec<PathBuf>, DiscoveryError> {
    if pattern.len() > 256 {
        return Err(DiscoveryError::Limit);
    }
    if !pattern.contains(['*', '?', '[', '{']) {
        let path = root.join(pattern);
        return if path.join("Cargo.toml").exists() {
            Ok(vec![path])
        } else {
            Err(DiscoveryError::Io)
        };
    }
    let glob = GlobBuilder::new(pattern)
        .literal_separator(false)
        .build()
        .map_err(|_| DiscoveryError::Malformed)?
        .compile_matcher();
    let mut result = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut seen = HashSet::new();
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(DiscoveryError::Limit);
        }
        let canonical = fs::canonicalize(&directory).map_err(|_| DiscoveryError::Io)?;
        if !canonical.starts_with(root) || !seen.insert(canonical.clone()) {
            continue;
        }
        for entry in fs::read_dir(&canonical).map_err(|_| DiscoveryError::Io)? {
            budget.entry()?;
            let entry = entry.map_err(|_| DiscoveryError::Io)?;
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(".git" | "node_modules" | "target" | ".hg" | ".svn")
            ) {
                continue;
            }
            let path = entry.path();
            let kind = entry.file_type().map_err(|_| DiscoveryError::Io)?;
            if kind.is_dir() || kind.is_symlink() && path.is_dir() {
                let canonical_child = fs::canonicalize(&path).map_err(|_| DiscoveryError::Io)?;
                if glob.is_match(
                    canonical_child
                        .strip_prefix(root)
                        .unwrap_or(&canonical_child),
                ) && canonical_child.join("Cargo.toml").exists()
                {
                    result.push(canonical_child.clone());
                }
                pending.push((path, depth + 1));
            }
        }
    }
    if root.join(pattern).join("Cargo.toml").exists() {
        result.push(root.join(pattern));
    }
    result.sort();
    result.dedup();
    Ok(result)
}

fn glob_matches(pattern: &str, path: &Path, root: &Path) -> bool {
    let Ok(glob) = GlobBuilder::new(pattern).literal_separator(false).build() else {
        return false;
    };
    glob.compile_matcher()
        .is_match(path.strip_prefix(root).unwrap_or(path))
}
