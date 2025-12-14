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
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use syn::visit::Visit;

/// A single boundary violation discovered by the linter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// File path relative to `backend/src`.
    pub file: PathBuf,
    /// Human-readable description of the violated rule.
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.message)
    }
}

/// Failure modes returned by the architecture lint.
#[derive(Debug)]
pub enum ArchitectureLintError {
    /// Filesystem traversal or reading failed.
    Io(io::Error),
    /// Rust source parsing failed.
    Parse { file: PathBuf, message: String },
    /// One or more boundary violations were found.
    Violations(Vec<Violation>),
}

impl fmt::Display for ArchitectureLintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error while linting architecture: {err}"),
            Self::Parse { file, message } => write!(
                f,
                "Failed to parse Rust source while linting architecture ({}): {message}",
                file.display()
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

/// Lint the backend crate sources on disk.
///
/// `backend_dir` must be the `backend/` directory at the repository root.
pub fn lint_backend_sources(backend_dir: &Path) -> Result<(), ArchitectureLintError> {
    let src_dir = backend_dir.join("src");
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
    pub file: PathBuf,
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
    fn infer_from_path(relative_path: &Path) -> Option<Self> {
        let first = relative_path
            .components()
            .next()?
            .as_os_str()
            .to_string_lossy();
        match first.as_ref() {
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

fn lint_parsed_source(file: &Path, layer: ModuleLayer, parsed: &syn::File) -> Vec<Violation> {
    let forbidden_modules = layer.forbidden_module_roots();
    let forbidden_crates = layer.forbidden_crate_roots();
    let layer_name = layer_name(layer);

    let mut collector = PathCollector::default();
    collector.visit_file(parsed);

    let mut violations = Vec::new();
    for segments in &collector.paths {
        if let Some(root) = forbidden_internal_module_root(segments, &forbidden_modules) {
            violations.push(Violation {
                file: file.to_path_buf(),
                message: format!("{layer_name} module must not depend on crate::{root}"),
            });
        }

        if let Some(root) = forbidden_external_crate_root(segments, &forbidden_crates) {
            violations.push(Violation {
                file: file.to_path_buf(),
                message: format!("{layer_name} module must not depend on external crate `{root}`"),
            });
        }
    }

    violations
}

const fn layer_name(layer: ModuleLayer) -> &'static str {
    match layer {
        ModuleLayer::Domain => "domain",
        ModuleLayer::Inbound => "inbound",
        ModuleLayer::Outbound => "outbound",
    }
}

fn forbidden_internal_module_root<'a>(
    segments: &'a [String],
    forbidden_roots: &BTreeSet<&'static str>,
) -> Option<&'a str> {
    let first = segments.first()?.as_str();
    if !is_relative_module_segment(first) {
        return None;
    }

    let root = segments
        .iter()
        .find(|segment| !is_relative_module_segment(segment.as_str()))
        .map(String::as_str)?;

    if forbidden_roots.contains(root) {
        Some(root)
    } else {
        None
    }
}

fn forbidden_external_crate_root<'a>(
    segments: &'a [String],
    forbidden_roots: &BTreeSet<&'static str>,
) -> Option<&'a str> {
    let root = segments.first()?.as_str();
    if forbidden_roots.contains(root) {
        Some(root)
    } else {
        None
    }
}

fn is_relative_module_segment(segment: &str) -> bool {
    matches!(segment, "crate" | "self" | "super")
}

#[derive(Default)]
struct PathCollector {
    paths: Vec<Vec<String>>,
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
        self.paths.push(segments);
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
                self.paths.push(segments);
            }
            syn::UseTree::Rename(rename) => {
                let mut segments = prefix;
                segments.push(rename.ident.to_string());
                self.paths.push(segments);
            }
            syn::UseTree::Glob(_) => {
                let mut segments = prefix;
                segments.push("*".to_owned());
                self.paths.push(segments);
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
        syn::visit::visit_item_use(self, node);
    }
}

fn collect_lint_sources(src_dir: &Path) -> Result<Vec<LintSource>, ArchitectureLintError> {
    let mut sources = Vec::new();
    for layer_dir in ["domain", "inbound", "outbound"] {
        let dir = src_dir.join(layer_dir);
        if !dir.exists() {
            continue;
        }
        collect_sources_under(src_dir, &dir, &mut sources)?;
    }
    Ok(sources)
}

fn collect_sources_under(
    src_root: &Path,
    current: &Path,
    sources: &mut Vec<LintSource>,
) -> Result<(), ArchitectureLintError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_sources_under(src_root, &path, sources)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let relative = path
            .strip_prefix(src_root)
            .map_err(|err| ArchitectureLintError::Parse {
                file: path.clone(),
                message: err.to_string(),
            })?
            .to_path_buf();
        let contents = fs::read_to_string(&path)?;
        sources.push(LintSource {
            file: relative,
            contents,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn lint_single(file: &str, contents: &str) -> Result<(), ArchitectureLintError> {
        lint_sources(&[LintSource {
            file: PathBuf::from(file),
            contents: contents.to_owned(),
        }])
    }

    #[rstest]
    #[case(
        "inbound/http/users.rs",
        "use crate::domain::UserId; fn handler() { let _ = UserId::new(\"x\"); }",
        true
    )]
    #[case(
        "inbound/http/users.rs",
        "use crate::outbound::persistence::DieselUserRepository; fn handler() { let _ = DieselUserRepository; }",
        false
    )]
    #[case(
        "inbound/http/users.rs",
        "use diesel::prelude::*; fn handler() {}",
        false
    )]
    #[case(
        "domain/user.rs",
        "use crate::inbound::http; fn thing() { let _ = 1; }",
        false
    )]
    #[case(
        "outbound/persistence/user_repository.rs",
        "use crate::inbound::http; fn thing() { let _ = 1; }",
        false
    )]
    fn detects_boundary_violations(#[case] file: &str, #[case] contents: &str, #[case] ok: bool) {
        let result = lint_single(file, contents);
        assert_eq!(result.is_ok(), ok, "result: {result:?}");
    }
}
