//! The diagnostic renderer: one report, drawn against the source text.
//!
//! ```text
//! error[E5028]: `read` variant `Noop` of schema `noop` cannot occupy ...
//!     ╭─[ device.phm:35:50 ]
//!     │
//!  34 │                 /// Extends a schema, copying its variants.
//!  35 │                 write field extended @ 1 extends noop {
//!     │                 ──┬──                            ──┬─
//!     │                   │                                ╰─── extended here
//!     │                   ╰──── field `extended` is declared `write` here
//!  36 │                     /// An appended state.
//!     │
//!     │ note: a `read` variant occupies the read numericity — ...
//! ────╯
//! ```
//!
//! The rules of the drawing:
//! - every labeled line appears **once**, however many labels it carries,
//! - only labeled lines render — but ranges within [reach](MERGE_DISTANCE)
//!   of one another merge, the lines between them shown,
//! - the labeled portions of the source text draw in their label's color,
//! - distant ranges are separated by `┆`; other files get their own
//!   `├─[ file ]` heading,
//! - the primary label draws in the report's color, secondary labels in
//!   blue, and notes are lowercase and unnumbered,
//! - text wraps at [word boundaries](wrap), continuations keeping their
//!   indentation.

use colored::{Color, Colorize};
use syntax::ast::{SourceId, Span};

use crate::model::{
    report::{Kind, Rank},
    source::Sources,
};

/// Labeled ranges at most this many lines apart merge — the lines between
/// them render rather than eliding into a `┆`.
const MERGE_DISTANCE: usize = 2;

/// The column messages and notes wrap at.
const WRAP_WIDTH: usize = 100;

/// A report, independent of which phase produced it.
pub(crate) struct Rendering<'a> {
    pub kind: Kind,
    pub rank: Rank,
    pub message: String,
    /// `(span, message, primary)` — exactly one label should be primary.
    pub labels: Vec<(Span, String, bool)>,
    pub notes: &'a [String],
}

/// Render a report to a string, ready to print.
pub(super) fn render(sources: &Sources, rendering: &Rendering) -> String {
    Renderer::new(sources, rendering).finish()
}

// ---------------------------------------------------------------- geometry

/// Byte offsets of every line start, plus a lookup from byte offset to
/// `(line, character column)`.
struct Lines<'a> {
    content: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(content: &'a str) -> Self {
        let mut starts = vec![0];

        for (offset, byte) in content.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(offset + 1);
            }
        }

        Self { content, starts }
    }

    /// The text of a (zero-based) line, newline excluded, tabs widened.
    fn text(&self, line: usize) -> String {
        let start = self.starts[line];
        let end = self
            .starts
            .get(line + 1)
            .map(|next| next - 1)
            .unwrap_or(self.content.len());

        self.content[start..end.max(start)].replace('\t', "    ")
    }

    /// The `(line, character column)` of a byte offset.
    fn place(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.content.len());
        let line = self.starts.partition_point(|start| *start <= offset) - 1;
        let prefix = &self.content[self.starts[line]..offset];

        let column = prefix
            .chars()
            .map(|character| if character == '\t' { 4 } else { 1 })
            .sum();

        (line, column)
    }
}

/// A label, resolved against its source: lines and character columns.
struct Placed {
    message: String,
    color: Color,
    start_line: usize,
    end_line: usize,
    /// Character column of the span's start, within its start line.
    start_column: usize,
    /// Character column of the span's end, within its end line.
    end_column: usize,
}

impl Placed {
    fn multiline(&self) -> bool {
        self.start_line != self.end_line
    }

    /// The width of the underline, at least one character.
    fn width(&self) -> usize {
        (self.end_column.max(self.start_column + 1) - self.start_column).max(1)
    }

    /// The column the arrow rises from: the middle of the underline.
    fn anchor(&self) -> usize {
        self.start_column + self.width() / 2
    }
}

// ---------------------------------------------------------------- wrapping

/// Break text at word boundaries so no line exceeds `width` columns.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(16);
    let mut lines = vec![String::new()];

    for word in text.split_whitespace() {
        let line = lines.last_mut().unwrap();

        if line.is_empty() {
            line.push_str(word);
        } else if line.chars().count() + 1 + word.chars().count() <= width {
            line.push(' ');
            line.push_str(word);
        } else {
            lines.push(word.to_string());
        }
    }

    lines
}

/// Paint regions of a line of source text in their labels' colors.
///
/// Regions are `(start, end, color)` in character columns, sorted;
/// overlapping starts defer to the earlier region.
fn painted(text: &str, regions: &[(usize, usize, Color)]) -> String {
    let characters = text.chars().collect::<Vec<_>>();
    let mut out = String::new();
    let mut cursor = 0;

    for (start, end, color) in regions {
        let start = (*start).clamp(cursor, characters.len());
        let end = (*end).clamp(start, characters.len());

        if start > cursor {
            out.extend(&characters[cursor..start]);
        }

        if end > start {
            let region = characters[start..end].iter().collect::<String>();
            out.push_str(&format!("{}", region.color(*color)));
        }

        cursor = end.max(cursor);
    }

    out.extend(&characters[cursor.min(characters.len())..]);
    out
}

// ---------------------------------------------------------------- renderer

struct Renderer<'a> {
    sources: &'a Sources,
    rendering: &'a Rendering<'a>,
    /// The report's color: red for errors, yellow for warnings.
    color: Color,
    /// The width of the line-number gutter.
    gutter: usize,
    out: String,
}

impl<'a> Renderer<'a> {
    fn new(sources: &'a Sources, rendering: &'a Rendering<'a>) -> Self {
        let color = match rendering.rank {
            Rank::Warning => Color::Yellow,
            Rank::Error => Color::Red,
        };

        let gutter = rendering
            .labels
            .iter()
            .map(|(span, ..)| {
                let lines = Lines::new(&sources.files[span.context].content);
                lines.place(span.end.max(span.start)).0 + 1
            })
            .max()
            .unwrap_or(1)
            .to_string()
            .len()
            .max(2);

        Self {
            sources,
            rendering,
            color,
            gutter,
            out: String::new(),
        }
    }

    fn finish(mut self) -> String {
        self.headline();

        for (position, file) in self.files().iter().enumerate() {
            self.group(position, *file);
        }

        self.notes();
        self.footer();
        self.out
    }

    /// A gutter-and-rail row: `    │ {body}`.
    fn row(&mut self, number: Option<usize>, body: &str) {
        let gutter = match number {
            Some(number) => format!("{:>width$}", number, width = self.gutter),
            None => " ".repeat(self.gutter),
        };

        let body = if body.is_empty() {
            String::new()
        } else {
            format!(" {body}")
        };

        self.out.push_str(&format!(
            "{} {}{body}\n",
            gutter.bright_black(),
            "│".bright_black(),
        ));
    }

    /// `error[Exxxx]: the judgement`, wrapped with a hanging indent.
    fn headline(&mut self) {
        let code = format!("{:04}", self.rendering.kind as u32);

        let title = match self.rendering.rank {
            Rank::Warning => format!("warning[W{code}]"),
            Rank::Error => format!("error[E{code}]"),
        };

        let indent = title.chars().count() + 2;
        let mut lines = wrap(&self.rendering.message, WRAP_WIDTH - indent).into_iter();

        self.out.push_str(&format!(
            "{}{} {}\n",
            title.color(self.color).bold(),
            ":".bold(),
            lines.next().unwrap_or_default().bold(),
        ));

        for line in lines {
            self.out
                .push_str(&format!("{}{}\n", " ".repeat(indent), line.bold()));
        }
    }

    /// The files the labels live in: the primary label's first, the rest in
    /// order of appearance.
    fn files(&self) -> Vec<SourceId> {
        let mut files = Vec::new();

        if let Some((span, ..)) = self
            .rendering
            .labels
            .iter()
            .find(|(.., primary)| *primary)
            .or(self.rendering.labels.first())
        {
            files.push(span.context);
        }

        for (span, ..) in &self.rendering.labels {
            if !files.contains(&span.context) {
                files.push(span.context);
            }
        }

        files
    }

    /// One file's window: its heading, its labeled lines with context, and
    /// their labels.
    fn group(&mut self, position: usize, file: SourceId) {
        let lines = Lines::new(&self.sources.files[file].content);
        let mut placed = self.placed(file, &lines);

        self.heading(position, file, &lines);
        self.row(None, "");

        let margin = placed.iter().any(Placed::multiline);
        let ranges = Self::ranges(&placed);

        // eliminate the indent every visible line shares, so deep nesting
        // doesn't push the window off to the right — the heading keeps the
        // true file coordinates
        let dedent = ranges
            .iter()
            .flat_map(|(from, to)| *from..=*to)
            .filter_map(|line| {
                let text = lines.text(line);

                (!text.trim().is_empty())
                    .then(|| text.chars().take_while(|character| *character == ' ').count())
            })
            .min()
            .unwrap_or(0);

        for label in &mut placed {
            label.start_column = label.start_column.saturating_sub(dedent);
            label.end_column = label.end_column.saturating_sub(dedent);
        }

        for (index, (from, to)) in ranges.iter().enumerate() {
            if index > 0 {
                self.out.push_str(&format!(
                    "{} {}\n",
                    " ".repeat(self.gutter).bright_black(),
                    "┆".bright_black(),
                ));
            }

            for line in *from..=*to {
                self.source_line(&lines, line, &placed, margin, dedent);

                let labeled = self.inline_labels(line, &placed, margin);
                let labeled = self.multiline_messages(line, &placed) || labeled;

                // a line of air between a label block and what follows it
                let last = index == ranges.len() - 1 && line == *to;

                if labeled && !last {
                    self.row(None, "");
                }
            }
        }
    }

    /// This file's labels, placed and sorted by position. The primary label
    /// keeps the report's color; the rest are blue.
    fn placed(&self, file: SourceId, lines: &Lines) -> Vec<Placed> {
        let mut placed = self
            .rendering
            .labels
            .iter()
            .filter(|(span, ..)| span.context == file)
            .map(|(span, message, primary)| {
                let (start_line, start_column) = lines.place(span.start);
                let (end_line, end_column) = lines.place(span.end.max(span.start));

                Placed {
                    message: message.clone(),
                    color: if *primary { self.color } else { Color::Blue },
                    start_line,
                    end_line,
                    start_column,
                    end_column,
                }
            })
            .collect::<Vec<_>>();

        placed.sort_by_key(|label| (label.start_line, label.start_column));
        placed
    }

    /// `╭─[ file:line:column ]`, pointing at the file's first label — the
    /// path in white, the frame in grey.
    fn heading(&mut self, position: usize, file: SourceId, lines: &Lines) {
        let labels = || {
            self.rendering
                .labels
                .iter()
                .filter(|(span, ..)| span.context == file)
        };

        let head = labels()
            .find(|(.., primary)| *primary)
            .or_else(|| labels().next())
            .map(|(span, ..)| lines.place(span.start))
            .map(|(line, column)| (line + 1, column + 1))
            .unwrap_or((1, 1));

        let marker = if position == 0 { "╭─[" } else { "├─[" };

        self.out.push_str(&format!(
            "{} {} {}\n",
            format!("{} {marker}", " ".repeat(self.gutter)).bright_black(),
            format!("{}:{}:{}", self.sources.name(file), head.0, head.1).white(),
            "]".bright_black(),
        ));
    }

    /// The line ranges to render: exactly the labeled lines — merged, along
    /// with everything between, when they come within [`MERGE_DISTANCE`] of
    /// one another.
    fn ranges(placed: &[Placed]) -> Vec<(usize, usize)> {
        let mut show = Vec::new();

        for label in placed {
            show.push((label.start_line, label.start_line));
            show.push((label.end_line, label.end_line));

            // short multiline spans render whole
            if label.multiline() && label.end_line - label.start_line <= 3 {
                show.push((label.start_line, label.end_line));
            }
        }

        show.sort();

        let mut ranges: Vec<(usize, usize)> = Vec::new();

        for (from, to) in show {
            match ranges.last_mut() {
                Some((.., end)) if from <= *end + MERGE_DISTANCE + 1 => {
                    *end = (*end).max(to);
                }
                _ => ranges.push((from, to)),
            }
        }

        ranges
    }

    /// A line of source text: number, margin cell, and the text itself —
    /// with the labeled portions painted in their labels' colors.
    fn source_line(
        &mut self,
        lines: &Lines,
        line: usize,
        placed: &[Placed],
        margin: bool,
        dedent: usize,
    ) {
        let text = lines.text(line).chars().skip(dedent).collect::<String>();
        let end = text.chars().count();

        // the regions of this line each label claims
        let mut regions = Vec::new();

        for label in placed {
            if !label.multiline() && label.start_line == line {
                regions.push((label.start_column, label.end_column.max(label.start_column + 1), label.color));
            } else if label.multiline() && line == label.start_line {
                regions.push((label.start_column, end, label.color));
            } else if label.multiline() && line == label.end_line {
                regions.push((0, label.end_column, label.color));
            } else if label.multiline() && line > label.start_line && line < label.end_line {
                regions.push((0, end, label.color));
            }
        }

        regions.sort_by_key(|(start, ..)| *start);

        let body = format!(
            "{}{}",
            self.margin_cell(line, placed, margin),
            painted(&text, &regions),
        );

        self.row(Some(line + 1), &body);
    }

    /// The margin cell of a line: side arrows for multiline labels.
    fn margin_cell(&self, line: usize, placed: &[Placed], margin: bool) -> String {
        if !margin {
            return String::new();
        }

        for label in placed.iter().filter(|label| label.multiline()) {
            if line == label.start_line {
                return format!("{} ", "╭─▶".color(label.color));
            } else if line == label.end_line {
                return format!("{} ", "├─▶".color(label.color));
            } else if line > label.start_line && line < label.end_line {
                return format!("{}   ", "│".color(label.color));
            }
        }

        "    ".to_string()
    }

    /// The hanging block under a labeled line: one underline row for every
    /// span at once, then a message row per label, rightmost first, wrapped
    /// text keeping its indentation.
    fn inline_labels(&mut self, line: usize, placed: &[Placed], margin: bool) -> bool {
        let inline = placed
            .iter()
            .filter(|label| !label.multiline() && label.start_line == line)
            .collect::<Vec<_>>();

        if inline.is_empty() {
            return false;
        }

        let pad = if margin { "    " } else { "" };

        // the underline row — the cursor tracks display columns, since the
        // text carries color escapes
        let mut underline = String::new();
        let mut cursor = 0;

        for label in &inline {
            if cursor < label.start_column {
                underline.push_str(&" ".repeat(label.start_column - cursor));
                cursor = label.start_column;
            }

            let middle = label.width() / 2;
            let mark = (0..label.width())
                .map(|at| if at == middle { '┬' } else { '─' })
                .collect::<String>();

            underline.push_str(&format!("{}", mark.color(label.color)));
            cursor += label.width();
        }

        self.row(None, &format!("{pad}{underline}"));

        // message rows, rightmost label first; labels still pending to the
        // left keep a riser through every row, wrapped lines included
        for (at, label) in inline.iter().enumerate().rev() {
            // the risers of the labels still pending to the left
            let risers = || {
                let mut row = String::new();
                let mut cursor = 0;

                for earlier in inline.iter().take(at) {
                    if cursor < earlier.anchor() {
                        row.push_str(&" ".repeat(earlier.anchor() - cursor));
                        cursor = earlier.anchor();
                    }

                    row.push_str(&format!("{}", "│".color(earlier.color)));
                    cursor += 1;
                }

                (row, cursor)
            };

            let (mut row, cursor) = risers();

            if cursor < label.anchor() {
                row.push_str(&" ".repeat(label.anchor() - cursor));
            }

            let arrow = format!("╰{}", "─".repeat(label.width() / 2 + 2));
            let indent = label.anchor() + arrow.chars().count() + 1;
            let mut text = wrap(&label.message, WRAP_WIDTH - indent).into_iter();

            row.push_str(&format!(
                "{} {}",
                arrow.color(label.color),
                text.next().unwrap_or_default(),
            ));

            self.row(None, &format!("{pad}{row}"));

            for continuation in text {
                let (mut row, cursor) = risers();

                if cursor < indent {
                    row.push_str(&" ".repeat(indent - cursor));
                }

                row.push_str(&continuation);
                self.row(None, &format!("{pad}{row}"));
            }
        }

        true
    }

    /// A multiline label's message hangs after its end line.
    fn multiline_messages(&mut self, line: usize, placed: &[Placed]) -> bool {
        let mut labeled = false;

        for label in placed
            .iter()
            .filter(|label| label.multiline() && label.end_line == line)
        {
            labeled = true;

            let arrow = "╰──▶";
            let indent = arrow.chars().count() + 1;
            let mut text = wrap(&label.message, WRAP_WIDTH - indent).into_iter();

            self.row(
                None,
                &format!(
                    "{} {}",
                    arrow.color(label.color),
                    text.next().unwrap_or_default(),
                ),
            );

            for continuation in text {
                self.row(None, &format!("{}{continuation}", " ".repeat(indent)));
            }
        }

        labeled
    }

    /// `note: ...` rows, wrapped with the text keeping its indentation.
    fn notes(&mut self) {
        if self.rendering.notes.is_empty() {
            return;
        }

        self.row(None, "");

        for note in self.rendering.notes {
            let prefix = "note: ";
            let mut lines = wrap(note, WRAP_WIDTH - prefix.chars().count()).into_iter();

            let first = format!(
                "{} {}",
                "note:".bright_blue().bold(),
                lines.next().unwrap_or_default(),
            );
            self.row(None, &first);

            for continuation in lines {
                self.row(
                    None,
                    &format!("{}{continuation}", " ".repeat(prefix.chars().count())),
                );
            }
        }
    }

    fn footer(&mut self) {
        self.out.push_str(&format!(
            "{}{}\n",
            "─".repeat(self.gutter + 1).bright_black(),
            "╯".bright_black(),
        ));
    }
}
