//! Repo-local architectural lint for enforcing hexagonal boundaries.
//!
//! The Wildside backend is a hexagonal modular monolith. The "hexagon" is
//! enforced at the Rust module level (`domain` + ports, inbound adapters,
//! outbound adapters). This crate provides a lightweight lint that:
//!
//! - forbids `domain` code from depending on adapter modules (`inbound`,
//!   `outbound`) or framework/infrastructure crates
//! - forbids `inbound` adapters from importing `outbound` modules or
//!   infrastructure crates directly
//! - forbids `outbound` adapters from importing `inbound` modules
//!
//! The lint is executed by `make lint` via `cargo run -p architecture-lint`.

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs::Dir;

use syn::visit::Visit;

/// A single boundary violation discovered by the linter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// File path relative to `backend/src`.
    pub file: Utf8PathBuf,
    /// Human-readable description of the violated rule.
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file, self.message)
    }
}

/// Failure modes returned by the architecture lint.
#[derive(Debug)]
pub enum ArchitectureLintError {
    /// Filesystem traversal or reading failed.
    Io(io::Error),
    /// Rust source parsing failed.
    Parse { file: Utf8PathBuf, message: String },
    /// One or more boundary violations were found.
    Violations(Vec<Violation>),
}

impl fmt::Display for ArchitectureLintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error while linting architecture: {err}"),
            Self::Parse { file, message } => write!(
                f,
                "Failed to parse Rust source while linting architecture ({file}): {message}",
            ),
            Self::Violations(violations) => {
                writeln!(f, "Architecture boundary violations:")?;
                for violation in violations {
                    writeln!(f, "- {violation}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ArchitectureLintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for ArchitectureLintError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn cargo_toml_declares_workspace(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("Cargo.toml"))
        .ok()
        .is_some_and(|contents| contents.contains("[workspace]"))
}

/// Lint the backend crate sources on disk.
///
/// `backend_dir` must be a capability handle to the `backend/` directory at
/// the repository root.
pub fn lint_backend_sources(backend_dir: &Dir) -> Result<(), ArchitectureLintError> {
    let src_dir = backend_dir.open_dir("src")?;
    let sources = collect_lint_sources(&src_dir)?;
    lint_sources(&sources)
}

/// Lint the provided Rust sources. Intended for unit and behaviour tests.
pub fn lint_sources(sources: &[LintSource]) -> Result<(), ArchitectureLintError> {
    let mut violations = Vec::new();

    for source in sources {
        let layer = ModuleLayer::infer_from_path(&source.file).ok_or_else(|| {
            ArchitectureLintError::Parse {
                file: source.file.clone(),
                message: "unable to infer module layer from file path".to_owned(),
            }
        })?;
        let parsed =
            syn::parse_file(&source.contents).map_err(|err| ArchitectureLintError::Parse {
                file: source.file.clone(),
                message: err.to_string(),
            })?;
        violations.extend(lint_parsed_source(&source.file, layer, &parsed));
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(ArchitectureLintError::Violations(violations))
    }
}

/// A Rust source file to be linted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintSource {
    /// Path relative to `backend/src`.
    pub file: Utf8PathBuf,
    pub contents: String,
}

/// The architectural "layer" inferred from a file path under `backend/src`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleLayer {
    Domain,
    Inbound,
    Outbound,
}

impl ModuleLayer {
    fn infer_from_path(relative_path: &Utf8Path) -> Option<Self> {
        let first = relative_path.components().next()?.as_str();
        match first {
            "domain" => Some(Self::Domain),
            "inbound" => Some(Self::Inbound),
            "outbound" => Some(Self::Outbound),
            _ => None,
        }
    }

    fn forbidden_module_roots(self) -> BTreeSet<&'static str> {
        match self {
            Self::Domain => BTreeSet::from(["inbound", "outbound"]),
            Self::Inbound => BTreeSet::from(["outbound"]),
            Self::Outbound => BTreeSet::from(["inbound"]),
        }
    }

    fn forbidden_crate_roots(self) -> BTreeSet<&'static str> {
        match self {
            Self::Domain => BTreeSet::from([
                "actix",
                "actix_service",
                "actix_web",
                "actix_web_actors",
                "awc",
                "diesel",
                "diesel_async",
                "diesel_migrations",
                "pg_embedded_setup_unpriv",
                "postgres",
                "postgresql_embedded",
                "utoipa",
            ]),
            Self::Inbound => BTreeSet::from([
                "diesel",
                "diesel_async",
                "diesel_migrations",
                "pg_embedded_setup_unpriv",
                "postgres",
                "postgresql_embedded",
            ]),
            Self::Outbound => BTreeSet::from([
                "actix",
                "actix_service",
                "actix_web",
                "actix_web_actors",
                "awc",
            ]),
        }
    }
}

fn lint_parsed_source(file: &Utf8Path, layer: ModuleLayer, parsed: &syn::File) -> Vec<Violation> {
    let forbidden_modules = layer.forbidden_module_roots();
    let forbidden_crates = layer.forbidden_crate_roots();
    let layer_name = layer_name(layer);

    let mut collector = PathCollector::default();
    collector.visit_file(parsed);

    let mut messages = BTreeSet::new();
    for segments in &collector.paths {
        if let Some(root) = forbidden_internal_module_root(segments, &forbidden_modules) {
            messages.insert(format!(
                "{layer_name} module must not depend on crate::{root}"
            ));
        }

        if let Some(root) = forbidden_external_crate_root(segments, &forbidden_crates) {
            messages.insert(format!(
                "{layer_name} module must not depend on external crate `{root}`"
            ));
        }
    }

    messages
        .into_iter()
        .map(|message| Violation {
            file: file.to_path_buf(),
            message,
        })
        .collect()
}

const fn layer_name(layer: ModuleLayer) -> &'static str {
    match layer {
        ModuleLayer::Domain => "domain",
        ModuleLayer::Inbound => "inbound",
        ModuleLayer::Outbound => "outbound",
    }
}

fn forbidden_internal_module_root(
    segments: &[String],
    forbidden_roots: &BTreeSet<&'static str>,
) -> Option<&'static str> {
    let root = internal_module_root(segments)?;
    forbidden_roots.get(root).copied()
}

fn forbidden_external_crate_root(
    segments: &[String],
    forbidden_roots: &BTreeSet<&'static str>,
) -> Option<&'static str> {
    let root = external_crate_root(segments)?;
    forbidden_roots.get(root).copied()
}

fn is_relative_module_segment(segment: &str) -> bool {
    matches!(segment, "crate" | "self" | "super")
}

fn internal_module_root(segments: &[String]) -> Option<&str> {
    let first = segments.first()?.as_str();
    if matches!(first, "domain" | "inbound" | "outbound") {
        return Some(first);
    }
    let start_index = match first {
        "crate" | "self" | "super" => segments
            .iter()
            .position(|segment| !is_relative_module_segment(segment.as_str()))?,
        "backend" => 1,
        _ => return None,
    };
    segments.get(start_index).map(|segment| segment.as_str())
}

fn external_crate_root(segments: &[String]) -> Option<&str> {
    let root = segments.first()?.as_str();
    if is_relative_module_segment(root) || root == "backend" {
        return None;
    }
    Some(root)
}

#[derive(Default)]
struct PathCollector {
    paths: BTreeSet<Vec<String>>,
}

impl PathCollector {
    fn record_path(&mut self, path: &syn::Path) {
        let segments = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        if segments.is_empty() {
            return;
        }
        self.paths.insert(segments);
    }

    fn record_use_tree(&mut self, tree: &syn::UseTree, prefix: Vec<String>) {
        match tree {
            syn::UseTree::Path(path) => {
                let mut next = prefix;
                next.push(path.ident.to_string());
                self.record_use_tree(&path.tree, next);
            }
            syn::UseTree::Name(name) => {
                let mut segments = prefix;
                segments.push(name.ident.to_string());
                self.paths.insert(segments);
            }
            syn::UseTree::Rename(rename) => {
                let mut segments = prefix;
                segments.push(rename.ident.to_string());
                self.paths.insert(segments);
            }
            syn::UseTree::Glob(_) => {
                let mut segments = prefix;
                segments.push("*".to_owned());
                self.paths.insert(segments);
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.record_use_tree(item, prefix.clone());
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for PathCollector {
    fn visit_path(&mut self, node: &'ast syn::Path) {
        self.record_path(node);
        syn::visit::visit_path(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        self.record_use_tree(&node.tree, Vec::new());
    }
}

fn collect_lint_sources(src_dir: &Dir) -> Result<Vec<LintSource>, ArchitectureLintError> {
    let mut sources = Vec::new();
    for layer_dir in ["domain", "inbound", "outbound"] {
        let Ok(dir) = src_dir.open_dir(layer_dir) else {
            continue;
        };
        let prefix = Utf8PathBuf::from(layer_dir);
        collect_sources_under(&dir, &prefix, &mut sources)?;
    }
    Ok(sources)
}

fn collect_sources_under(
    current: &Dir,
    prefix: &Utf8Path,
    sources: &mut Vec<LintSource>,
) -> Result<(), ArchitectureLintError> {
    for entry in current.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "non-UTF8 filename"))?;
        let relative = prefix.join(name_str);

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let subdir = current.open_dir(name_str)?;
            collect_sources_under(&subdir, &relative, sources)?;
            continue;
        }

        if !name_str.ends_with(".rs") {
            continue;
        }

        let contents = current.read_to_string(name_str)?;
        sources.push(LintSource {
            file: relative,
            contents,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
