//! Loading model descriptions: the entry file and, transitively, everything
//! it imports.
//!
//! `import st.gpio` loads `st/gpio.phm` relative to the importing file's
//! directory, and its definitions become referenceable as `gpio.<name>` —
//! imports are named by their final path segment. Files are loaded once no
//! matter how many routes import them, so import cycles are harmless.
//!
//! # Model roots
//!
//! A `phm.toml` manifest marks the [root](root) of a model, binding the
//! layout conventions: `devices/` holds the entry files — exactly one
//! device each — and `components/` holds the importable descriptions.
//! A device's imports resolve within `components/`; a component's resolve
//! beside the importing file, so a component tree stays self-contained.
//!
//! # Provided sources
//!
//! A dependency may *provide* model component descriptions instead of
//! shipping loose files: [`load_with`] takes `(import path, content)` pairs,
//! and an import resolves against them before the filesystem. A crate
//! exports its descriptions (`("cortex_m/nvic.phm", include_str!(...))`),
//! and a dependent's build script passes them through — the descriptions
//! travel inside the cargo dependency.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use syntax::ast::{FileItem, SourceId, Span};

use crate::model::semantic::Diagnostic;

/// The loaded source files of a model description: the entry file first,
/// then its imports in discovery order. [`SourceId`]s index into this.
pub struct Sources {
    pub(crate) files: Vec<SourceFile>,
    /// Load-time diagnostics: unreadable imports, alias conflicts.
    pub(crate) issues: Vec<Diagnostic>,
}

pub(crate) struct SourceFile {
    /// The display name diagnostics render with.
    pub(crate) name: String,
    /// The filesystem path, if the file came from disk.
    pub(crate) path: Option<PathBuf>,
    pub(crate) content: String,
    /// Import aliases: referenceable name → the imported source.
    pub(crate) imports: HashMap<String, SourceId>,
}

impl Sources {
    /// A single source that imports nothing — for embedded or in-memory
    /// model descriptions.
    pub fn single(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            files: vec![SourceFile {
                name: name.into(),
                path: None,
                content: content.into(),
                imports: HashMap::new(),
            }],
            issues: Vec::new(),
        }
    }

    /// Assemble sources entirely in memory: an embedded entry file whose
    /// imports resolve against [provided](load_with) descriptions — for
    /// consumers without a filesystem, like the proc-macro path.
    pub fn assemble(entry: (&str, &str), provided: &[(&str, &str)]) -> Self {
        let mut sources = Sources {
            files: vec![SourceFile {
                name: entry.0.to_string(),
                path: None,
                content: entry.1.to_string(),
                imports: HashMap::new(),
            }],
            issues: Vec::new(),
        };

        resolve_imports(&mut sources, provided, None);
        sources
    }

    /// The display name of a source.
    pub fn name(&self, source: SourceId) -> &str {
        &self.files[source].name
    }

    /// The filesystem paths of every source that came from disk.
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.files.iter().filter_map(|file| file.path.as_deref())
    }
}

/// The model root governing `from`: the nearest ancestor directory —
/// `from` included, when it is one — holding a `phm.toml` manifest.
pub fn root(from: impl AsRef<Path>) -> Option<PathBuf> {
    // parents are lexical — `.` has none — so ancestry needs the real path
    let start = from.as_ref().canonicalize().ok()?;
    let mut current = Some(start.as_path());

    while let Some(directory) = current {
        if directory.join("phm.toml").is_file() {
            return Some(directory.to_path_buf());
        }

        current = directory.parent();
    }

    None
}

/// Load a model description: the entry file at `path` and, transitively,
/// everything it imports.
///
/// Failures *within* the graph — unreadable imports, alias conflicts — are
/// recorded as diagnostics and reported during evaluation; only an unreadable
/// entry file is an outright error.
pub fn load(path: impl AsRef<Path>) -> Result<Sources, String> {
    load_with(path, &[])
}

/// [`load`], with provided sources: `(import path, content)` pairs an import
/// may resolve to before the filesystem — `("cortex_m/nvic.phm", ...)`
/// satisfies `import cortex_m.nvic` from any file.
pub fn load_with(
    path: impl AsRef<Path>,
    provided: &[(&str, &str)],
) -> Result<Sources, String> {
    let path = path.as_ref();

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;

    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    let mut sources = Sources {
        files: vec![SourceFile {
            name,
            path: Some(path.to_path_buf()),
            content,
            imports: HashMap::new(),
        }],
        issues: Vec::new(),
    };

    // within a phm.toml-rooted model, the device's imports resolve within
    // the root's components
    let components = path
        .parent()
        .and_then(root)
        .map(|root| root.join("components"));

    resolve_imports(&mut sources, provided, components.as_deref());
    Ok(sources)
}

/// Discover and load every import, breadth-first: provided sources first,
/// then the filesystem — the device's within the model's `components`,
/// a component's beside the importing file.
fn resolve_imports(sources: &mut Sources, provided: &[(&str, &str)], components: Option<&Path>) {
    let mut loaded: HashMap<PathBuf, SourceId> = HashMap::new();

    if let Some(path) = &sources.files[0].path
        && let Ok(canonical) = path.canonicalize()
    {
        loaded.insert(canonical, 0);
    }

    let mut queue = vec![0];
    // provided sources already materialized, by import path
    let mut provisioned: HashMap<String, SourceId> = HashMap::new();
    // per (file, alias): where the import was first declared, for conflict
    // labels
    let mut declared: HashMap<(SourceId, String), Span> = HashMap::new();

    while let Some(source) = queue.pop() {
        // parse a copy of the content to discover imports; evaluation
        // re-parses (and reports) later
        let content = sources.files[source].content.clone();
        let (file, ..) = syntax::parse(&content, source);

        let Some(file) = file else {
            continue;
        };

        // the device — the entry — imports from the model's components
        let anchored = source == 0 && components.is_some();

        let base = match (anchored, components) {
            (true, Some(components)) => components.to_path_buf(),
            _ => sources.files[source]
                .path
                .as_ref()
                .and_then(|path| path.parent())
                .map(Path::to_path_buf)
                // a provided file's imports resolve against its logical
                // directory
                .or_else(|| {
                    PathBuf::from(&sources.files[source].name)
                        .parent()
                        .map(Path::to_path_buf)
                })
                .unwrap_or_default(),
        };

        for item in &file.items {
            let FileItem::Import(import) = &item.inner else {
                continue;
            };

            let segments = import
                .path
                .inner
                .segments
                .iter()
                .map(|segment| segment.inner.to_string())
                .collect::<Vec<_>>();

            let Some(alias) = segments.last().cloned() else {
                continue;
            };

            let key = format!("{}.phm", segments.join("/"));
            let relative = PathBuf::from(&key);
            let target = base.join(&relative);

            // the name an import resolves to among provided sources: the
            // path as written — prefixed, within a provided file, by the
            // provider's own directory, so a pack's internal imports stay
            // within the pack
            let provided_key = if sources.files[source].path.is_some() {
                key.clone()
            } else {
                base.join(&relative).to_string_lossy().into_owned()
            };

            // provided sources satisfy an import before the filesystem
            let id = if let Some(id) = provisioned.get(&provided_key) {
                *id
            } else if let Some((.., content)) = provided
                .iter()
                .find(|(name, ..)| *name == provided_key)
            {
                let id = sources.files.len();

                sources.files.push(SourceFile {
                    name: provided_key.clone(),
                    path: None,
                    content: content.to_string(),
                    imports: HashMap::new(),
                });

                provisioned.insert(provided_key, id);
                queue.push(id);
                id
            } else {
                match target.canonicalize().ok().and_then(|c| loaded.get(&c).copied()) {
                    Some(id) => id,
                    None => {
                        let content = match std::fs::read_to_string(&target) {
                            Ok(content) => content,
                            Err(e) => {
                                sources.issues.push(Diagnostic::unreadable_import(
                                    import.path.span,
                                    &segments.join("."),
                                    &target.display().to_string(),
                                    format!("{e}"),
                                    anchored,
                                ));
                                continue;
                            }
                        };

                        let id = sources.files.len();

                        sources.files.push(SourceFile {
                            name: relative.display().to_string(),
                            path: Some(target.clone()),
                            content,
                            imports: HashMap::new(),
                        });

                        if let Ok(canonical) = target.canonicalize() {
                            loaded.insert(canonical, id);
                        }

                        queue.push(id);
                        id
                    }
                }
            };

            if sources.files[source].imports.contains_key(&alias) {
                sources.issues.push(Diagnostic::conflicting_import(
                    import.path.span,
                    &alias,
                    declared.get(&(source, alias.clone())).copied(),
                ));
                continue;
            }

            declared.insert((source, alias.clone()), import.path.span);
            sources.files[source].imports.insert(alias, id);
        }
    }
}
