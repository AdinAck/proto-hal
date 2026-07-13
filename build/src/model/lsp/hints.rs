//! Inlay hints: every interrupt entry's vector position.

use std::collections::HashSet;

use lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams};

use super::{Server, canonical, path_of};

impl Server {
    pub(super) fn hints(&self, params: &InlayHintParams) -> Option<Vec<InlayHint>> {
        let path = canonical(&path_of(&params.text_document.uri)?);
        let mut hints: Vec<InlayHint> = Vec::new();
        let mut covered: HashSet<(u32, u32)> = HashSet::new();

        for snapshot in &self.snapshots {
            let Some(source) = snapshot
                .files
                .iter()
                .position(|file| file.path.as_deref() == Some(path.as_path()))
            else {
                continue;
            };

            let file = &snapshot.files[source];

            for (span, position) in &snapshot.analysis.vectors {
                if span.context != source {
                    continue;
                }

                // after the entry, where nothing shifts — inlays only ever
                // insert, so a leading hint pushes the names around
                let at = file
                    .index
                    .position(&file.content, span.end.min(file.content.len()));

                if at < params.range.start
                    || at > params.range.end
                    || !covered.insert((at.line, at.character))
                {
                    continue;
                }

                hints.push(InlayHint {
                    position: at,
                    label: InlayHintLabel::String(position.to_string()),
                    // editors gate kind-less hints behind an "other hints"
                    // setting; type hints render by default
                    kind: Some(InlayHintKind::TYPE),
                    text_edits: None,
                    tooltip: None,
                    padding_left: Some(true),
                    padding_right: None,
                    data: None,
                });
            }
        }

        Some(hints)
    }
}
