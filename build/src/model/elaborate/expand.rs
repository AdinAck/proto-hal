//! Array expansion: element designators → names, position lists → concrete
//! positions.

use syntax::ast::{Indices, ListEntry, NumRange, Span, Spanned};

use crate::model::semantic::Diagnostic;

/// Produce the element names of an array: the base name suffixed with each
/// designator.
///
/// `series` is the ordinal of the enclosing array element this definition is
/// elaborating within — `[0..=31, ...]` designators continue across those
/// elements, each picking up where the previous left off.
pub(super) fn element_names(
    base: &str,
    indices: &Spanned<Indices>,
    series: usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<String>> {
    match &indices.inner {
        Indices::Names(names) => Some(
            names
                .iter()
                .map(|name| format!("{base}{}", name.inner))
                .collect(),
        ),
        Indices::Range(range) => {
            let start = range.start.inner;
            let end = bound(range, indices.span, diagnostics)?;

            Some(
                (start..=end)
                    .step_by(range.step.as_deref().copied().unwrap_or(1) as _)
                    .map(|i| format!("{base}{i}"))
                    .collect(),
            )
        }
        Indices::Series(range) => {
            let start = range.start.inner;
            let end = bound(range, indices.span, diagnostics)?;
            let step = range.step.as_deref().copied().unwrap_or(1);
            let shift = (end - start + step) * series as u32;

            Some(
                (start + shift..=end + shift)
                    .step_by(step as _)
                    .map(|i| format!("{base}{i}"))
                    .collect(),
            )
        }
    }
}

/// Expand a scalar position list — addresses, offsets, or variant values —
/// to exactly `count` concrete values.
///
/// `default_step` is what a bare `...` steps by in this position, or an
/// explanation of why it may not appear at all.
pub(super) fn scalar_series(
    entries: &[Spanned<ListEntry>],
    count: usize,
    default_step: Result<i64, &'static str>,
    list_span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<u32>> {
    let mut out = Vec::with_capacity(count);

    for (i, entry) in entries.iter().enumerate() {
        match &entry.inner {
            ListEntry::Value(value) => out.push(*value),
            ListEntry::Range(..) => {
                diagnostics.push(Diagnostic::scalar_entry(entry.span));
                return None;
            }
            ListEntry::Rest(rest) => {
                if i != entries.len() - 1 {
                    diagnostics.push(Diagnostic::unsupported_rest(entry.span));
                    return None;
                }

                let step = step_of(rest, default_step, entry.span, diagnostics)?;

                let Some(&previous) = out.last() else {
                    diagnostics.push(Diagnostic::dangling_rest(entry.span));
                    return None;
                };

                let mut previous = previous as i64;

                while out.len() < count {
                    previous += step;
                    out.push(in_range(previous, entry.span, diagnostics)?);
                }
            }
        }
    }

    if out.len() != count {
        diagnostics.push(Diagnostic::count_mismatch(
            list_span,
            "positions",
            count,
            out.len(),
        ));
        return None;
    }

    Some(out)
}

/// Expand a bit domain list to exactly `count` concrete `(offset, width)`
/// pairs. A bare `...` packs elements adjacently: each new element begins
/// where the previous ended.
pub(super) fn span_series(
    entries: &[Spanned<ListEntry>],
    count: usize,
    list_span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<(u32, u32)>> {
    let mut out: Vec<(u32, u32)> = Vec::with_capacity(count);

    for (i, entry) in entries.iter().enumerate() {
        match &entry.inner {
            ListEntry::Value(offset) => out.push((*offset, 1)),
            ListEntry::Range(range) => out.push(bit_domain(range, entry.span, diagnostics)?),
            ListEntry::Rest(rest) => {
                if i != entries.len() - 1 {
                    diagnostics.push(Diagnostic::unsupported_rest(entry.span));
                    return None;
                }

                let Some(&(offset, width)) = out.last() else {
                    diagnostics.push(Diagnostic::dangling_rest(entry.span));
                    return None;
                };

                let step = step_of(rest, Ok(width as i64), entry.span, diagnostics)?;

                let mut offset = offset as i64;

                while out.len() < count {
                    offset += step;
                    out.push((in_range(offset, entry.span, diagnostics)?, width));
                }
            }
        }
    }

    if out.len() != count {
        diagnostics.push(Diagnostic::count_mismatch(
            list_span,
            "bit domains",
            count,
            out.len(),
        ));
        return None;
    }

    Some(out)
}

/// Interpret a range as a bit domain: `(offset, width)`.
pub(super) fn bit_domain(
    range: &NumRange,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(u32, u32)> {
    let start = range.start.inner;
    let end = bound(range, span, diagnostics)?;

    if let Some(step) = range.step {
        diagnostics.push(Diagnostic::non_contiguous_domain(step.span));
        return None;
    }

    Some((start, end - start + 1))
}

/// The inclusive upper bound of a range, validated to be nonempty.
fn bound(range: &NumRange, span: Span, diagnostics: &mut Vec<Diagnostic>) -> Option<u32> {
    let start = range.start.inner;
    let end = if range.inclusive {
        range.end.inner
    } else {
        let Some(end) = range
            .end
            .inner
            .checked_sub(range.step.as_deref().copied().unwrap_or(1))
        else {
            diagnostics.push(Diagnostic::empty_range(span));
            return None;
        };
        end
    };

    if end < start {
        diagnostics.push(Diagnostic::empty_range(span));
        return None;
    }

    Some(end)
}

/// The step a `...` advances by: its explicit stride, or the positional
/// default.
fn step_of(
    rest: &syntax::ast::Rest,
    default: Result<i64, &'static str>,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<i64> {
    if let Some(stride) = &rest.stride {
        let magnitude = stride.inner.magnitude as i64;

        return Some(if stride.inner.negative {
            -magnitude
        } else {
            magnitude
        });
    }

    match default {
        Ok(step) => Some(step),
        Err(explanation) => {
            diagnostics.push(Diagnostic::no_default_stride(span, explanation));
            None
        }
    }
}

fn in_range(value: i64, span: Span, diagnostics: &mut Vec<Diagnostic>) -> Option<u32> {
    u32::try_from(value).ok().or_else(|| {
        diagnostics.push(Diagnostic::out_of_range(span, value));
        None
    })
}
