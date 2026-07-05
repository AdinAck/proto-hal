//! Completions: what may be written where the cursor stands.

use lsp_types::{CompletionItem, CompletionItemKind, CompletionResponse, TextDocumentPositionParams};

use super::{Server, Snapshot};

impl Server {
    pub(super) fn complete(&self, at: &TextDocumentPositionParams) -> Option<CompletionResponse> {
        // devices diverge — an 8-channel model continues where a 6-channel
        // one ends — so offerings union across every snapshot of the file
        let mut items: Vec<CompletionItem> = Vec::new();

        for (snapshot, source, offset) in self.under(at) {
            for item in self.offerings(snapshot, source, offset).unwrap_or_default() {
                if !items.iter().any(|existing| existing.label == item.label) {
                    items.push(item);
                }
            }
        }

        (!items.is_empty()).then_some(CompletionResponse::Array(items))
    }

    fn offerings(
        &self,
        snapshot: &Snapshot,
        source: usize,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let file = &snapshot.files[source];
        let line_number = file.index.position(&file.content, offset).line as usize;
        let line_start = file.index.starts[line_number];
        let line = &file.content[line_start..offset];

        // the dotted path being written, and what precedes it
        let written_from = line
            .rfind(|character: char| {
                !character.is_alphanumeric() && character != '_' && character != '.'
            })
            .map(|position| position + 1)
            .unwrap_or(0);
        let written = &line[written_from..];
        let before = line[..written_from].trim_end();

        let items: Vec<CompletionItem> = if before.ends_with('#') {
            // a template reference
            snapshot
                .analysis
                .templates
                .iter()
                .map(|name| completion(name, CompletionItemKind::CONSTRUCTOR))
                .collect()
        } else if before.ends_with("assumes") || before.ends_with("extends") {
            // schema references: placements in reach, and templates
            snapshot
                .analysis
                .placements
                .iter()
                .map(|(name, ..)| completion(name, CompletionItemKind::INTERFACE))
                .collect()
        } else if before.ends_with("requires")
            || before.ends_with('&')
            || before.ends_with('|')
            || before.ends_with('(')
            || written.contains('.')
        {
            // an entitlement path: the names that may follow the written
            // prefix
            let prefix = match written.rsplit_once('.') {
                Some((prefix, ..)) => prefix
                    .split('.')
                    .map(|segment| segment.to_string())
                    .collect::<Vec<_>>(),
                None => Vec::new(),
            };

            snapshot
                .analysis
                .tree
                .get(&prefix)?
                .iter()
                .map(|name| {
                    let kind = match name.starts_with(|c: char| c.is_uppercase()) {
                        true => CompletionItemKind::ENUM_MEMBER,
                        false => CompletionItemKind::FIELD,
                    };

                    completion(name, kind)
                })
                .collect()
        } else {
            return None;
        };

        Some(items)
    }
}

fn completion(name: &str, kind: CompletionItemKind) -> CompletionItem {
    CompletionItem {
        label: name.to_string(),
        kind: Some(kind),
        ..Default::default()
    }
}
