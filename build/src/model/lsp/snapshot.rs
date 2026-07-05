//! Evaluation into snapshots: published diagnostics, and the text geometry
//! every answer is measured with.

use std::collections::HashMap;

use lsp_server::{Connection, Message, Request};
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString,
    Position, PublishDiagnosticsParams, Range, Uri,
    notification::{Notification as _, PublishDiagnostics},
    request::{InlayHintRefreshRequest, Request as _},
};
use syntax::ast::Span;

use crate::model::{Rank, Sources, devices, evaluate_sources, load_overlaid, report};

use super::{Failure, Server, Snapshot, SnapshotFile, canonical, uri_of};

impl Server {
    /// Evaluate every device of every model in the workspace and publish
    /// the diagnostics, file by file. A file that healed receives an empty
    /// publication to clear it.
    // `Uri` caches parse state in a `Cell`, which clippy flags as a
    // mutable key — its hashing and equality are on the text alone
    #[allow(clippy::mutable_key_type)]
    pub(super) fn publish(&mut self, connection: &Connection) -> Result<(), Failure> {
        let mut fresh: HashMap<Uri, Vec<Diagnostic>> = HashMap::new();
        let mut snapshots = Vec::new();

        for model in self.models() {
            for device in devices(&model) {
                let Ok(sources) = load_overlaid(&device, &self.overlays) else {
                    continue;
                };

                let evaluation = evaluate_sources(&sources);
                converted(&sources, &evaluation.diagnostics, &mut fresh);

                snapshots.push(Snapshot {
                    files: sources
                        .files
                        .iter()
                        .map(|file| SnapshotFile {
                            path: file.path.as_deref().map(canonical),
                            content: file.content.clone(),
                            index: LineIndex::new(&file.content),
                        })
                        .collect(),
                    analysis: evaluation.analysis,
                });
            }
        }

        self.snapshots = snapshots;

        for healed in &self.published {
            if !fresh.contains_key(healed) {
                fresh.insert(healed.clone(), Vec::new());
            }
        }

        self.published = fresh
            .iter()
            .filter(|(.., diagnostics)| !diagnostics.is_empty())
            .map(|(uri, ..)| uri.clone())
            .collect();

        for (uri, diagnostics) in fresh {
            connection
                .sender
                .send(Message::Notification(lsp_server::Notification::new(
                    PublishDiagnostics::METHOD.to_string(),
                    PublishDiagnosticsParams {
                        uri,
                        diagnostics,
                        version: None,
                    },
                )))?;
        }

        self.trace.note(format!(
            "published: {} snapshots, {} vectors in view",
            self.snapshots.len(),
            self.snapshots
                .iter()
                .map(|snapshot| snapshot.analysis.vectors.len())
                .sum::<usize>(),
        ));

        // ask the editor to refetch hints only when they actually changed:
        // a refresh invalidates editor caches, and needless invalidations
        // race the editor's own scroll-driven fetching
        let vectors = {
            use std::hash::{Hash as _, Hasher as _};

            let mut hasher = std::collections::hash_map::DefaultHasher::new();

            for snapshot in &self.snapshots {
                for (span, position) in &snapshot.analysis.vectors {
                    snapshot.files[span.context].path.hash(&mut hasher);
                    span.start.hash(&mut hasher);
                    span.end.hash(&mut hasher);
                    position.hash(&mut hasher);
                }
            }

            hasher.finish()
        };

        // no refresh for the session's first picture: the editor is about
        // to fetch it anyway, and a refresh racing that initial fetch makes
        // it cancel half of its own work
        if self.refreshes
            && let Some(known) = self.vectors
            && known != vectors
        {
            self.revision += 1;

            connection.sender.send(Message::Request(Request::new(
                lsp_server::RequestId::from(self.revision),
                InlayHintRefreshRequest::METHOD.to_string(),
                serde_json::Value::Null,
            )))?;

            self.trace
                .note(format!("← workspace/inlayHint/refresh #{}", self.revision));
        }

        self.vectors = Some(vectors);

        Ok(())
    }
}

/// Convert evaluated diagnostics into per-file publications, deduplicating
/// across the devices that surfaced them.
#[allow(clippy::mutable_key_type)]
fn converted(
    sources: &Sources,
    diagnostics: &[report::Diagnostic],
    fresh: &mut HashMap<Uri, Vec<Diagnostic>>,
) {
    let indices = sources
        .files
        .iter()
        .map(|file| LineIndex::new(&file.content))
        .collect::<Vec<_>>();

    let located = |span: &Span| -> Option<(Uri, Range)> {
        let file = &sources.files[span.context];
        let uri = uri_of(&canonical(file.path.as_ref()?))?;

        Some((uri, indices[span.context].range(&file.content, span)))
    };

    for diagnostic in diagnostics {
        let rendering = report::rendering(diagnostic);

        // the primary label carries the anchoring span
        let Some((span, ..)) = rendering.labels.first() else {
            continue;
        };

        let Some((uri, range)) = located(span) else {
            continue;
        };

        let related = rendering.labels[1..]
            .iter()
            .filter_map(|(span, message, ..)| {
                let (uri, range) = located(span)?;

                Some(DiagnosticRelatedInformation {
                    location: Location { uri, range },
                    message: message.clone(),
                })
            })
            .collect::<Vec<_>>();

        let mut message = rendering.message.clone();

        for note in rendering.notes {
            message.push_str("\nnote: ");
            message.push_str(note);
        }

        let (severity, letter) = match rendering.rank {
            Rank::Error => (DiagnosticSeverity::ERROR, 'E'),
            Rank::Warning => (DiagnosticSeverity::WARNING, 'W'),
        };

        let converted = Diagnostic {
            range,
            severity: Some(severity),
            code: Some(NumberOrString::String(format!(
                "{letter}{:04}",
                rendering.kind as u32,
            ))),
            source: Some("phm".to_string()),
            message,
            related_information: (!related.is_empty()).then_some(related),
            ..Default::default()
        };

        let bucket = fresh.entry(uri).or_default();

        if !bucket.contains(&converted) {
            bucket.push(converted);
        }
    }
}

/// Byte offsets to protocol positions: line starts once, UTF-16 columns on
/// demand.
pub(super) struct LineIndex {
    pub(super) starts: Vec<usize>,
}

impl LineIndex {
    pub(super) fn new(content: &str) -> Self {
        let mut starts = vec![0];

        for (offset, byte) in content.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(offset + 1);
            }
        }

        Self { starts }
    }

    pub(super) fn position(&self, content: &str, offset: usize) -> Position {
        let offset = offset.min(content.len());
        let line = self.starts.partition_point(|start| *start <= offset) - 1;
        let character = content[self.starts[line]..offset].encode_utf16().count();

        Position {
            line: line as u32,
            character: character as u32,
        }
    }

    pub(super) fn range(&self, content: &str, span: &Span) -> Range {
        Range {
            start: self.position(content, span.start),
            end: self.position(content, span.end),
        }
    }

    /// The byte offset of a protocol position.
    pub(super) fn offset(&self, content: &str, position: Position) -> usize {
        let Some(start) = self.starts.get(position.line as usize).copied() else {
            return content.len();
        };

        let line_end = self
            .starts
            .get(position.line as usize + 1)
            .copied()
            .unwrap_or(content.len());

        let mut characters = position.character as usize;
        let mut offset = start;

        for character in content[start..line_end].chars() {
            let width = character.len_utf16();

            if characters < width {
                break;
            }

            characters -= width;
            offset += character.len_utf8();
        }

        offset
    }
}

/// The word — identifier characters — around a byte offset.
pub(super) fn word_at(content: &str, offset: usize) -> Option<(usize, usize)> {
    let bytes = content.as_bytes();
    let wordish = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';

    if offset >= bytes.len() || !wordish(bytes[offset]) {
        return None;
    }

    let mut start = offset;
    let mut end = offset;

    while start > 0 && wordish(bytes[start - 1]) {
        start -= 1;
    }

    while end < bytes.len() && wordish(bytes[end]) {
        end += 1;
    }

    Some((start, end))
}
