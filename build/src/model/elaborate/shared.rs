//! Shared elaboration machinery: element expansion and head properties.

use syntax::ast::{
    Access, Domain, Field, Indices, Span, Spanned,
};

use crate::model::semantic::Diagnostic;

use super::{
    Context, Modality, expand, side_decl, side_name, side_verdict,
};

impl<'ast, 'src> Context<'ast, 'src> {
    /// The `(name, position)` of every element this definition describes:
    /// one for a plain definition, many for an array.
    #[expect(clippy::too_many_arguments)]
    pub(super) fn scalar_elements(
        &mut self,
        name: &Option<Spanned<&'src str>>,
        array: Option<Span>,
        indices: &Option<Spanned<Indices<'src>>>,
        domain: &Option<Spanned<Domain>>,
        default_step: Result<i64, &'static str>,
        what: &str,
        span: Span,
        series: usize,
    ) -> Option<Vec<(String, u32)>> {
        let name = self.name_of(name, what, span)?;

        match (array, indices) {
            (None, None) => {
                let domain = self.domain_of(domain, what, span)?;

                match &domain.inner {
                    Domain::Value(value) => Some(vec![(name.to_string(), *value)]),
                    Domain::Range(..) => {
                        self.diagnostics
                            .push(Diagnostic::scalar_position(domain.span, what));
                        None
                    }
                    Domain::List(..) => {
                        self.diagnostics
                            .push(Diagnostic::positions_without_array(domain.span));
                        None
                    }
                }
            }
            (array, Some(indices)) => {
                if array.is_none() {
                    self.diagnostics
                        .push(Diagnostic::designators_without_array(indices.span));
                    return None;
                }

                let names = expand::element_names(name, indices, series, &mut self.diagnostics)?;
                let domain = self.domain_of(domain, what, span)?;

                let Domain::List(entries) = &domain.inner else {
                    self.diagnostics.push(Diagnostic::expected_positions(domain.span));
                    return None;
                };

                let positions = expand::scalar_series(
                    entries,
                    names.len(),
                    default_step,
                    domain.span,
                    &mut self.diagnostics,
                )?;

                Some(names.into_iter().zip(positions).collect())
            }
            (Some(array), None) => {
                self.diagnostics.push(Diagnostic::expected_elements(array));
                None
            }
        }
    }

    /// As [`scalar_elements`](Self::scalar_elements), for fields: positions
    /// are `(offset, width)` bit domains.
    pub(super) fn bit_elements(
        &mut self,
        field: &Field<'src>,
        span: Span,
        series: usize,
    ) -> Option<Vec<(String, (u32, u32))>> {
        let name = self.name_of(&field.head.name, "field", span)?;

        match (field.array, &field.head.indices) {
            (None, None) => {
                let domain = self.domain_of(&field.domain, "field", span)?;

                match &domain.inner {
                    Domain::Value(offset) => Some(vec![(name.to_string(), (*offset, 1))]),
                    Domain::Range(range) => {
                        let bits = expand::bit_domain(range, domain.span, &mut self.diagnostics)?;
                        Some(vec![(name.to_string(), bits)])
                    }
                    Domain::List(..) => {
                        self.diagnostics
                            .push(Diagnostic::positions_without_array(domain.span));
                        None
                    }
                }
            }
            (array, Some(indices)) => {
                if array.is_none() {
                    self.diagnostics
                        .push(Diagnostic::designators_without_array(indices.span));
                    return None;
                }

                let names = expand::element_names(name, indices, series, &mut self.diagnostics)?;
                let domain = self.domain_of(&field.domain, "field", span)?;

                let Domain::List(entries) = &domain.inner else {
                    self.diagnostics.push(Diagnostic::expected_positions(domain.span));
                    return None;
                };

                let bits =
                    expand::span_series(entries, names.len(), domain.span, &mut self.diagnostics)?;

                Some(names.into_iter().zip(bits).collect())
            }
            (Some(array), None) => {
                self.diagnostics.push(Diagnostic::expected_elements(array));
                None
            }
        }
    }

    pub(super) fn name_of(
        &mut self,
        name: &Option<Spanned<&'src str>>,
        what: &str,
        span: Span,
    ) -> Option<&'src str> {
        let name = name.as_ref().map(|name| name.inner);

        if name.is_none() {
            self.diagnostics.push(Diagnostic::unnamed(span, what));
        }

        name
    }

    pub(super) fn domain_of<'a>(
        &mut self,
        domain: &'a Option<Spanned<Domain>>,
        what: &str,
        span: Span,
    ) -> Option<&'a Spanned<Domain>> {
        let domain = domain.as_ref();

        if domain.is_none() {
            self.diagnostics.push(Diagnostic::expected_position(span, what));
        }

        domain
    }

    /// Judge an inline variant's declared side against its field's modality:
    /// every side a variant names, the field must have.
    pub(super) fn validate_side(
        &mut self,
        name: &str,
        side: &Spanned<syntax::ast::Side>,
        modality: Modality,
        field_name: &str,
        modality_span: Span,
    ) {
        let Some(kind) = side_verdict(modality, side_decl(side)) else {
            return;
        };

        self.diagnostics.push(Diagnostic::incompatible_variant(
            kind,
            side.span,
            side_name(side_decl(side)),
            name,
            field_name,
            modality.to_string(),
            modality_span,
        ));
    }

    pub(super) fn modality(&mut self, access: &[Spanned<Access>], what: &str, span: Span) -> Option<Modality> {
        let mut read = false;
        let mut write = false;
        let mut store = false;
        let mut volatile_store = false;

        for access in access {
            match access.inner {
                Access::Read => read = true,
                Access::Write => write = true,
                Access::Store => store = true,
                Access::VolatileStore => volatile_store = true,
            }
        }

        match (read, write, store, volatile_store) {
            (true, false, false, false) => Some(Modality::Read),
            (false, true, false, false) => Some(Modality::Write),
            (true, true, false, false) => Some(Modality::ReadWrite),
            (false, false, true, false) => Some(Modality::Store),
            (false, false, false, true) => Some(Modality::VolatileStore),
            (false, false, false, false) => {
                self.diagnostics.push(Diagnostic::expected_modality(span, what));
                None
            }
            _ => {
                self.diagnostics.push(Diagnostic::invalid_modality(span));
                None
            }
        }
    }
}
