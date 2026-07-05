//! Cursor resolution, hover, and go-to-definition.
//!
//! Everything positional funnels through [`Server::under`] — the cursor in
//! every snapshot that knows the file — and [`Server::named`], the tightest
//! reference beneath it.

use lsp_types::{
    GotoDefinitionResponse, Hover, HoverContents, Location, MarkupContent, MarkupKind,
    TextDocumentPositionParams,
};
use syntax::ast::Span;

use super::{Named, Server, Snapshot, canonical, path_of, snapshot::word_at, uri_of};

impl Server {
    /// What the cursor rests on, in every snapshot containing the file:
    /// the snapshot, the file within it, and the byte offset. A component
    /// serves many devices, and each may know different regions of it — a
    /// template only one device invokes is only elaborated there.
    pub(super) fn under(&self, at: &TextDocumentPositionParams) -> Vec<(&Snapshot, usize, usize)> {
        let Some(path) = path_of(&at.text_document.uri).map(|path| canonical(&path)) else {
            return Vec::new();
        };

        self.snapshots
            .iter()
            .filter_map(|snapshot| {
                let source = snapshot
                    .files
                    .iter()
                    .position(|file| file.path.as_deref() == Some(path.as_path()))?;

                let file = &snapshot.files[source];
                let offset = file.index.offset(&file.content, at.position);

                Some((snapshot, source, offset))
            })
            .collect()
    }

    /// What the cursor's reference names — a context path (an entitlement
    /// mention) or a direct definition site (a template, schema, or import
    /// reference) — and the span that names it.
    pub(super) fn named(&self, at: &TextDocumentPositionParams) -> Option<(&Snapshot, Span, Named)> {
        for (snapshot, source, offset) in self.under(at) {
            let covering =
                |span: &Span| span.context == source && span.start <= offset && offset < span.end;

            // the tightest reference under the cursor, of either table
            let reference = snapshot
                .analysis
                .mentions
                .iter()
                .filter(|(span, ..)| covering(span))
                .map(|(span, target)| (*span, Named::Path(target.clone())))
                .chain(
                    snapshot
                        .analysis
                        .definitions
                        .iter()
                        .filter(|(span, ..)| covering(span))
                        .map(|(span, site)| (*span, Named::Site(*site))),
                )
                .min_by_key(|(span, ..)| span.end - span.start);

            if let Some((span, named)) = reference {
                return Some((snapshot, span, named));
            }
        }

        None
    }

    /// The definition whose head the cursor rests within — the fallback
    /// behind explicit references and keywords, since a head span covers
    /// its whole property stack.
    pub(super) fn head_named(
        &self,
        at: &TextDocumentPositionParams,
    ) -> Option<(&Snapshot, Span, Named)> {
        for (snapshot, source, offset) in self.under(at) {
            let head = snapshot
                .analysis
                .locations
                .iter()
                .filter(|(.., location)| {
                    location.head.context == source
                        && location.head.start <= offset
                        && offset < location.head.end
                })
                .min_by_key(|(.., location)| location.head.end - location.head.start);

            if let Some((path, location)) = head {
                return Some((snapshot, location.head, Named::Path(path.clone())));
            }
        }

        None
    }

    pub(super) fn hover(&self, at: &TextDocumentPositionParams) -> Option<Hover> {
        // an explicit reference answers first; keywords next — a keyword
        // within some definition's head still teaches itself — and the
        // enclosing head last
        if let Some(hover) = self.reference_hover(self.named(at)) {
            return Some(hover);
        }

        // interrupt entries show their vector position
        for (snapshot, source, offset) in self.under(at) {
            let entry = snapshot.analysis.vectors.iter().find(|(span, ..)| {
                span.context == source && span.start <= offset && offset < span.end
            });

            if let Some((span, position)) = entry {
                let file = &snapshot.files[source];
                let text = &file.content[span.start..span.end.min(file.content.len())];
                let mut value = format!("`{text}` — {position}");

                let line = file.index.position(&file.content, span.start).line as usize;
                let docs = docs_above(&file.content, line);

                if !docs.is_empty() {
                    value.push_str("\n\n");
                    value.push_str(&docs.join("\n"));
                }

                // `reserved` is also a keyword — its teaching follows
                if let Some(teaching) = keyword(text) {
                    value.push_str("\n\n");
                    value.push_str(teaching);
                }

                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value,
                    }),
                    range: Some(file.index.range(&file.content, span)),
                });
            }
        }

        // keywords and symbols teach themselves
        if let Some((snapshot, source, offset)) = self.under(at).into_iter().next() {
            let file = &snapshot.files[source];

            let teaching = word_at(&file.content, offset)
                .and_then(|(start, end)| {
                    keyword(&file.content[start..end]).map(|teaching| (start, end, teaching))
                })
                .or_else(|| symbol_at(&file.content, offset));

            if let Some((start, end, teaching)) = teaching {
                let glyph = &file.content[start..end];

                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("`{glyph}` — {teaching}"),
                    }),
                    range: Some(file.index.range(
                        &file.content,
                        &Span {
                            start,
                            end,
                            context: source,
                        },
                    )),
                });
            }
        }

        // the enclosing head answers last — highlighting only the word
        // under the cursor, not the whole property stack
        let mut hover = self.reference_hover(self.head_named(at))?;

        if let Some((snapshot, source, offset)) = self.under(at).into_iter().next()
            && let Some((start, end)) = word_at(&snapshot.files[source].content, offset)
        {
            let file = &snapshot.files[source];

            hover.range = Some(file.index.range(
                &file.content,
                &Span {
                    start,
                    end,
                    context: source,
                },
            ));
        }

        Some(hover)
    }

    fn reference_hover(&self, named: Option<(&Snapshot, Span, Named)>) -> Option<Hover> {
        let (snapshot, span, named) = named?;
        let file = &snapshot.files[span.context];

        let value = match named {
            Named::Path(target) => description(snapshot, &target)?,
            Named::Site(site) => site_card(snapshot, site)?,
        };

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(file.index.range(&file.content, &span)),
        })
    }

    pub(super) fn definition(
        &self,
        at: &TextDocumentPositionParams,
    ) -> Option<GotoDefinitionResponse> {
        let (snapshot, .., named) = self.named(at).or_else(|| self.head_named(at))?;

        let site = match named {
            Named::Path(target) => snapshot.analysis.locations.get(&target)?.head,
            Named::Site(site) => site,
        };

        let file = &snapshot.files[site.context];

        Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri_of(file.path.as_deref()?)?,
            range: file.index.range(&file.content, &site),
        }))
    }
}

/// A hover card for the element at `target`: its head as written, the doc
/// comments above it, and its context path.
fn description(snapshot: &Snapshot, target: &[String]) -> Option<String> {
    let location = snapshot.analysis.locations.get(target)?;
    let mut markdown = card(snapshot, location.head, location.head.end)?;

    markdown.push_str(&format!("\n\n`{}`", target.join(".")));
    Some(markdown)
}

/// A hover card for a direct definition site: a template or schema by its
/// name — or a whole file, when an import names one.
fn site_card(snapshot: &Snapshot, site: Span) -> Option<String> {
    let file = snapshot.files.get(site.context)?;

    if site.start == site.end {
        // an import — the site is the file itself
        let name = file
            .path
            .as_deref()?
            .file_name()?
            .to_string_lossy()
            .into_owned();
        return Some(format!("`{name}`"));
    }

    // through the end of the name's line, the head as written
    let line = file.index.position(&file.content, site.start).line as usize;
    let to = file
        .index
        .starts
        .get(line + 1)
        .map(|start| start.saturating_sub(1))
        .unwrap_or(file.content.len());

    card(snapshot, site, to)
}

/// The definition head at `span` — rendered from the start of its first
/// line through `to` — with the doc comments above it.
fn card(snapshot: &Snapshot, span: Span, to: usize) -> Option<String> {
    let file = snapshot.files.get(span.context)?;

    // the head, from the start of its first line — kind words included
    let line = file.index.position(&file.content, span.start).line as usize;
    let from = file.index.starts[line];
    // a body brace trailing the head is noise
    let head = dedented(
        file.content[from..to.min(file.content.len())]
            .trim_end()
            .trim_end_matches('{')
            .trim_end(),
    );

    let docs = docs_above(&file.content, line);
    let mut markdown = format!("```phm\n{head}\n```");

    if !docs.is_empty() {
        markdown.push_str("\n\n");
        markdown.push_str(&docs.join("\n"));
    }

    Some(markdown)
}

/// The doc comments directly above a line, in order.
fn docs_above(content: &str, line: usize) -> Vec<String> {
    let lines = content.lines().collect::<Vec<_>>();
    let mut docs = Vec::new();
    let mut above = line;

    while above > 0 {
        above -= 1;

        match lines
            .get(above)
            .and_then(|text| text.trim().strip_prefix("///"))
        {
            Some(doc) => docs.push(doc.trim().to_string()),
            None => break,
        }
    }

    docs.reverse();
    docs
}

/// Strip the common leading whitespace from every line.
fn dedented(text: &str) -> String {
    let indent = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    text.lines()
        .map(|line| line.get(indent.min(line.len())..).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The symbol — a contextual operator — around a byte offset, with its
/// teaching. Longest symbols match first, so `...` wins over `..`.
fn symbol_at(content: &str, offset: usize) -> Option<(usize, usize, &'static str)> {
    const TEACHINGS: &[(&str, &str)] = &[
        (
            "...",
            "continues the pattern: positions keep stepping — by the natural \
             stride, or an explicit one, `...+0x14`; designators like \
             `[0..=31, ...]` continue across the enclosing array's elements",
        ),
        ("..=", "an inclusive range: both ends belong"),
        ("..", "an exclusive range: the end does not belong"),
        (
            "@",
            "positions the definition: an address, an offset from the \
             enclosing base, or a bit domain",
        ),
        ("~", "what the variant occupies: its value within the field"),
        (
            "#",
            "invokes a template — the referenced definition, with the \
             invocation's own properties overriding",
        ),
        (
            "&",
            "joins entitled fields into one pattern: all must hold, each \
             field distinct",
        ),
        (
            "|",
            "joins patterns into a space: any one satisfies the requirement",
        ),
    ];

    let bytes = content.as_bytes();

    for (symbol, teaching) in TEACHINGS {
        let width = symbol.len();

        for start in offset.saturating_sub(width - 1)..=offset {
            if start + width <= bytes.len() && &bytes[start..start + width] == symbol.as_bytes() {
                return Some((start, start + width, teaching));
            }
        }
    }

    None
}

/// What each keyword means, for hover.
fn keyword(word: &str) -> Option<&'static str> {
    Some(match word {
        "import" => {
            "loads another description — its definitions become referenceable \
             through the import's final segment"
        }
        "device" => {
            "the root of a model: an entry file defines exactly one, and its \
             body holds everything the machine has"
        }
        "peripheral" => "a hardware block, at an absolute address",
        "register" => "a word of its peripheral, at an offset from the base",
        "field" => "a bit domain of its register, with an access modality",
        "schema" => {
            "a variant vocabulary — placed, it serves any field that assumes \
             it; fields may also copy from it with `extends`"
        }
        "variant" => "a state its field may inhabit, occupying a value",
        "group" => "a named module level gathering sibling definitions",
        "array" => {
            "one definition, many elements: designators in brackets, and \
             bracketed positions zip element-wise"
        }
        "interrupts" => "the device's interrupt vector, in position order",
        "reserved" => "an unoccupied interrupt position",
        "extends" => {
            "copies the referenced schemas' variants into the field — no \
             placement required"
        }
        "assumes" => "the field exactly reflects the referenced placed schema",
        "requires" => {
            "the states other fields must inhabit for this to be available: \
             `|` joins patterns, `&` joins distinct fields, `{A, B}` admits \
             a set of variants, and `name[a..b]` corresponds element-wise \
             within an array"
        }
        "reset" => "the state at power-on: a value, or a variant's name",
        "leaky" => "state the model cannot fully witness — hardware may change it",
        "inert" => "writing this variant leaves the field's state untouched",
        "read" => {
            "readable — and before `variant`, the variant occupies only the \
             read numericity"
        }
        "write" => {
            "writable — and before `variant`, the variant occupies only the \
             write numericity; `write requires` gates software's writes"
        }
        "store" => "software-owned storage: what is written is what is stored",
        "volatile" => {
            "with `store`: hardware writes it too — `hardware write requires` \
             gates those"
        }
        "hardware" => {
            "prefixes `write requires`: the entitlements governing hardware's \
             writes"
        }
        "as" => "names a template's invocation",
        _ => return None,
    })
}
