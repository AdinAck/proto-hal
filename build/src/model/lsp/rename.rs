//! Rename and find-all-references: the analysis tables, inverted.
//!
//! Both answers share the same collection — every occurrence of what the
//! cursor names. Model elements resolve through the mentions table; templates
//! and schemas through the connected component of the definitions table.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use lsp_types::{
    Location, PrepareRenameResponse, TextDocumentPositionParams, TextEdit, WorkspaceEdit,
};
use syntax::ast::Span;

use super::{Named, Server, Snapshot, snapshot::word_at, uri_of};

/// A span's identity across snapshots: its file, canonically, and its range.
type SpanKey = (PathBuf, usize, usize);

/// Edit ranges per file, deduplicated across the devices that produced them.
type Occurrences = HashMap<PathBuf, BTreeSet<(usize, usize)>>;

impl Server {
    pub(super) fn prepare_rename(
        &self,
        at: &TextDocumentPositionParams,
    ) -> Option<PrepareRenameResponse> {
        let name = self
            .path_target(at)
            .map(|(.., name)| name)
            .or_else(|| self.site_target(at).map(|(.., name)| name))?;

        let (snapshot, source, offset) = self.under(at).into_iter().next()?;
        let file = &snapshot.files[source];
        let (start, end) = word_at(&file.content, offset)?;

        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: file.index.range(
                &file.content,
                &Span {
                    start,
                    end,
                    context: source,
                },
            ),
            placeholder: name,
        })
    }

    /// Rename what the cursor names, everywhere: the definition's name, and
    /// every mention or reference of it across every file and device.
    pub(super) fn rename(
        &self,
        at: &TextDocumentPositionParams,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        let mut characters = new_name.chars();
        let named_like = characters
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && characters.all(|rest| rest.is_ascii_alphanumeric() || rest == '_');

        if !named_like {
            return None;
        }

        let (.., occurrences) = self
            .path_occurrences(at)
            .or_else(|| self.site_occurrences(at))?;

        self.workspace_edit(occurrences, new_name)
    }

    /// Every occurrence of what the cursor names, as locations.
    pub(super) fn references(
        &self,
        at: &TextDocumentPositionParams,
        include_declaration: bool,
    ) -> Option<Vec<Location>> {
        let (declaration, occurrences) = self
            .path_occurrences(at)
            .or_else(|| self.site_occurrences(at))?;

        let mut locations = Vec::new();

        for (path, ranges) in occurrences {
            let file = self.file_at(&path)?;
            let uri = uri_of(&path)?;

            for (start, end) in ranges {
                if !include_declaration && (path.as_path(), start, end) == (declaration.0.as_path(), declaration.1, declaration.2) {
                    continue;
                }

                locations.push(Location {
                    uri: uri.clone(),
                    range: file.index.range(
                        &file.content,
                        &Span {
                            start,
                            end,
                            context: 0,
                        },
                    ),
                });
            }
        }

        Some(locations)
    }

    /// Occurrences of a model element — a peripheral, register, field, or
    /// variant — through the mentions table. Mentions write the name whole
    /// or as an array base's prefix (`ccr[1..=6]`, `ccr3`), so each range
    /// covers exactly the name's characters.
    fn path_occurrences(
        &self,
        at: &TextDocumentPositionParams,
    ) -> Option<(SpanKey, Occurrences)> {
        let (definition, start, end, name) = self.path_target(at)?;
        let declaration = (definition.clone(), start, end);

        let mut occurrences = Occurrences::new();

        occurrences
            .entry(definition.clone())
            .or_default()
            .insert((start, end));

        for snapshot in &self.snapshots {
            for (span, target) in &snapshot.analysis.mentions {
                let Some(location) = snapshot.analysis.locations.get(target) else {
                    continue;
                };

                // the same definition: identified by its file and the
                // name's position
                let site = &snapshot.files[location.head.context];

                if site.path.as_deref() != Some(definition.as_path())
                    || location.head.start != start
                {
                    continue;
                }

                let file = &snapshot.files[span.context];

                let Some(path) = file.path.clone() else {
                    continue;
                };

                let Some(slice) = file.content.get(span.start..span.end.min(file.content.len()))
                else {
                    continue;
                };

                if !slice.starts_with(&name) {
                    continue;
                }

                occurrences
                    .entry(path)
                    .or_default()
                    .insert((span.start, span.start + name.len()));
            }
        }

        Some((declaration, occurrences))
    }

    /// Occurrences of a template or a schema through the definitions table:
    /// the connected component of reference–definition edges, spans matching
    /// by overlap — a placement's name is a segment *within* its template
    /// reference.
    fn site_occurrences(
        &self,
        at: &TextDocumentPositionParams,
    ) -> Option<(SpanKey, Occurrences)> {
        let (site, name) = self.site_target(at)?;

        let mut set: HashMap<PathBuf, Vec<(usize, usize)>> = HashMap::new();
        set.entry(site.0.clone())
            .or_default()
            .push((site.1, site.2));

        let overlapping = |set: &HashMap<PathBuf, Vec<(usize, usize)>>, key: &SpanKey| {
            set.get(&key.0).is_some_and(|ranges| {
                ranges
                    .iter()
                    .any(|(start, end)| key.1 < *end && *start < key.2)
            })
        };

        loop {
            let mut grew = false;

            for snapshot in &self.snapshots {
                for (reference, definition) in &snapshot.analysis.definitions {
                    let (Some(reference), Some(definition)) = (
                        span_key(snapshot, reference),
                        span_key(snapshot, definition),
                    ) else {
                        continue;
                    };

                    for (known, fresh) in [(&definition, &reference), (&reference, &definition)] {
                        if overlapping(&set, known) && !overlapping(&set, fresh) {
                            set.entry(fresh.0.clone())
                                .or_default()
                                .push((fresh.1, fresh.2));
                            grew = true;
                        }
                    }
                }
            }

            if !grew {
                break;
            }
        }

        // each span holds exactly the name within it: the whole span, or
        // its final path segment
        let mut occurrences = Occurrences::new();

        for (path, ranges) in set {
            let file = self.file_at(&path)?;

            for (start, end) in ranges {
                let Some(slice) = file.content.get(start..end.min(file.content.len())) else {
                    continue;
                };

                let fits = slice == name
                    || (slice.ends_with(&name)
                        && slice[..slice.len() - name.len()].ends_with('.'));

                if fits {
                    occurrences
                        .entry(path.clone())
                        .or_default()
                        .insert((end - name.len(), end));
                }
            }
        }

        Some((site, occurrences))
    }

    /// What a rename or reference search at `at` names, through the
    /// mentions table: the definition's file, its name's range, and the
    /// name. The cursor's word must *be* that name — a derived name, like
    /// an array element's `ccr3`, has no written form.
    fn path_target(&self, at: &TextDocumentPositionParams) -> Option<(PathBuf, usize, usize, String)> {
        let cursor = self.cursor_word(at)?;

        let (snapshot, .., named) = self.named(at).or_else(|| self.head_named(at))?;

        let Named::Path(target) = named else {
            return None;
        };

        let location = snapshot.analysis.locations.get(&target)?;
        let file = snapshot.files.get(location.head.context)?;
        let (start, end) = word_at(&file.content, location.head.start)?;
        let name = file.content[start..end].to_string();

        if cursor != name {
            return None;
        }

        Some((file.path.clone()?, start, end, name))
    }

    /// What a rename or reference search at `at` names, through the
    /// definitions table: a template's or placed schema's name site. The
    /// cursor's word must be that name.
    fn site_target(&self, at: &TextDocumentPositionParams) -> Option<(SpanKey, String)> {
        let cursor = self.cursor_word(at)?;

        let (snapshot, .., named) = self.named(at)?;

        let Named::Site(site) = named else {
            return None;
        };

        // an import names a whole file, which a rename cannot move
        if site.start == site.end {
            return None;
        }

        let file = snapshot.files.get(site.context)?;
        let (start, end) = word_at(&file.content, site.start)?;
        let name = file.content[start..end].to_string();

        if cursor != name {
            return None;
        }

        Some(((file.path.clone()?, start, end), name))
    }

    /// The word beneath the cursor.
    fn cursor_word(&self, at: &TextDocumentPositionParams) -> Option<String> {
        let (snapshot, source, offset) = self.under(at).into_iter().next()?;
        let file = &snapshot.files[source];
        let (start, end) = word_at(&file.content, offset)?;

        Some(file.content[start..end].to_string())
    }

    /// Ranges per file, rendered into a protocol edit. A rename that makes
    /// an `as` redundant — the invocation would take that name anyway —
    /// deletes the `as` instead.
    #[allow(clippy::mutable_key_type)]
    fn workspace_edit(&self, occurrences: Occurrences, new_name: &str) -> Option<WorkspaceEdit> {
        let mut changes = HashMap::new();

        for (path, ranges) in occurrences {
            let file = self.file_at(&path)?;
            let mut edits = Vec::new();

            for (start, end) in ranges {
                // `#new_name as old` — renaming the invocation to its
                // template's name: delete ` as old` whole
                if let Some(from) = redundant_before(&file.content, start, new_name) {
                    edits.push((from, end, String::new()));
                    continue;
                }

                edits.push((start, end, new_name.to_string()));

                // `#old as new_name` — renaming the template to the
                // invocation's name: the `as` becomes noise
                if let Some(to) = redundant_after(&file.content, end, new_name) {
                    edits.push((end, to, String::new()));
                }
            }

            let edits = edits
                .into_iter()
                .map(|(start, end, new_text)| TextEdit {
                    range: file.index.range(
                        &file.content,
                        &Span {
                            start,
                            end,
                            context: 0,
                        },
                    ),
                    new_text,
                })
                .collect();

            changes.insert(uri_of(&path)?, edits);
        }

        Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }
}

fn span_key(snapshot: &Snapshot, span: &Span) -> Option<SpanKey> {
    let file = snapshot.files.get(span.context)?;

    Some((file.path.clone()?, span.start, span.end))
}

/// Where deletion begins when the renamed word follows `<new_name> as `:
/// the position right after the path segment the name now repeats.
fn redundant_before(content: &str, start: usize, new_name: &str) -> Option<usize> {
    let wordish = |byte: &u8| byte.is_ascii_alphanumeric() || *byte == b'_';
    let bytes = content.as_bytes();

    let mut at = start;
    let spaces = |at: &mut usize| {
        let from = *at;
        while *at > 0 && bytes[*at - 1] == b' ' {
            *at -= 1;
        }
        from != *at
    };

    if !spaces(&mut at) || at < 2 || &bytes[at - 2..at] != b"as" {
        return None;
    }

    at -= 2;

    if !spaces(&mut at) {
        return None;
    }

    let word_end = at;
    while at > 0 && wordish(&bytes[at - 1]) {
        at -= 1;
    }

    (&content[at..word_end] == new_name).then_some(word_end)
}

/// Where deletion ends when the renamed word precedes ` as <new_name>`:
/// the position right after the now-redundant invocation name.
fn redundant_after(content: &str, end: usize, new_name: &str) -> Option<usize> {
    let wordish = |byte: &u8| byte.is_ascii_alphanumeric() || *byte == b'_';
    let bytes = content.as_bytes();

    let mut at = end;
    let spaces = |at: &mut usize| {
        let from = *at;
        while *at < bytes.len() && bytes[*at] == b' ' {
            *at += 1;
        }
        from != *at
    };

    if !spaces(&mut at) || at + 2 > bytes.len() || &bytes[at..at + 2] != b"as" {
        return None;
    }

    at += 2;

    if !spaces(&mut at) {
        return None;
    }

    let word_start = at;
    while at < bytes.len() && wordish(&bytes[at]) {
        at += 1;
    }

    (&content[word_start..at] == new_name).then_some(at)
}
