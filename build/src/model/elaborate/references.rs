//! Phase two: references — schema links, resets, and entitlements,
//! applied once every structure exists.

use ::model::{
    Composition, Entitlement,
    decl::{Side, VariantDecl},
    entitlement::EntitlementIndex,
    field::FieldIndex,
};
use std::collections::HashMap;
use syntax::ast::{Space, Span, Spanned};

use crate::model::semantic::Diagnostic;

use super::{
    Context, Correspondence, Enclosing, Link, Pending, PlacedSchema,
    Reference, Target, VariantPath, corresponding, did_you_mean, find_field, find_variant,
    ident, modality_access, side_decl, side_name, side_verdict, variant_names,
};

impl<'ast, 'src> Context<'ast, 'src> {
    /// Resolve schema references: link assuming fields and extend extending
    /// ones. Every schema exists by now, so references may point anywhere in
    /// the device.
    pub(super) fn links(&mut self, composition: &mut Composition) {
        let links = std::mem::take(&mut self.links);

        for link in links {
            match &link.reference {
                Reference::Assume(schema) => self.assume(composition, &link, schema),
                Reference::Extend(schemas) => self.extend(composition, &link, schemas),
            }
        }
    }

    /// The nearest placement of the named schema: search the reference's
    /// scopes from innermost outwards, ending at the device root.
    pub(super) fn placed(&self, path: &[String], name: &str) -> Option<PlacedSchema> {
        (0..path.len() + 1)
            .rev()
            .find_map(|depth| {
                self.schemas
                    .get(&(path[..depth.min(path.len())].to_vec(), name.to_string()))
            })
            .cloned()
    }

    /// The placement names visible from a scope, for suggestions.
    pub(super) fn placements_in_reach(&self, path: &[String]) -> Vec<String> {
        self.schemas
            .keys()
            .filter(|(scope, ..)| path.len() >= scope.len() && path[..scope.len()] == scope[..])
            .map(|(.., name)| name.clone())
            .collect()
    }

    /// Link an assuming field to its placed schema: the field exactly
    /// reflects it, projected onto the field's own modality.
    pub(super) fn assume(
        &mut self,
        composition: &mut Composition,
        link: &Link<'src>,
        schema: &Spanned<syntax::ast::Path<'src>>,
    ) {
        let field_name = &link.chain.2;

        let name = match schema.inner.segments.as_slice() {
            [segment] => segment.inner,
            _ => {
                self.diagnostics.push(Diagnostic::assumed_by_path(schema.span));
                return;
            }
        };

        let Some(placed) = self.placed(&link.path, name) else {
            let suggestion = did_you_mean(name, self.placements_in_reach(&link.path));
            let template_exists = suggestion.is_none()
                && self.templates[schema.span.context].schemas.contains_key(name);

            self.diagnostics.push(Diagnostic::unplaced_schema(
                schema.span,
                name,
                field_name,
                suggestion,
                template_exists,
            ));
            return;
        };

        if !self.compatible(schema.span, "assumed", name, &placed.variants, link) {
            return;
        }

        let Some(field) = find_field(composition, &link.chain) else {
            return;
        };

        let Some(index) = placed.index else {
            return;
        };

        composition.link_field(field, index);

        // the schema's variants become referenceable through the field
        for (variant, ..) in &placed.variants {
            let mut path = link.path.clone();
            path.push(variant.clone());

            self.variants
                .insert(path, (link.chain.clone(), variant.clone()));
        }
    }

    /// Extend a field from each referenced schema, in reference order. A
    /// placed schema takes precedence (nearest scope first); a reference no
    /// placement satisfies falls back to schema templates — extension
    /// requires no placement, the definitions land in the extending field.
    ///
    /// Each schema is judged independently: every side its variants name,
    /// the field must have.
    pub(super) fn extend(
        &mut self,
        composition: &mut Composition,
        link: &Link<'src>,
        schemas: &[Spanned<syntax::ast::Path<'src>>],
    ) {
        let Some(field) = find_field(composition, &link.chain) else {
            return;
        };

        for schema in schemas {
            let placed = match schema.inner.segments.as_slice() {
                [segment] => self
                    .placed(&link.path, segment.inner)
                    .map(|placed| (segment.inner, placed)),
                _ => None,
            };

            match placed {
                Some((name, placed)) => {
                    if !self.compatible(schema.span, "extended", name, &placed.variants, link) {
                        continue;
                    }

                    let Some(index) = placed.index else {
                        continue;
                    };

                    composition.extend_field(field, index, modality_access(link.modality));
                }
                None => {
                    // no placement in reach — the schema may come straight
                    // from a template
                    let Some(template) = self.resolve_template(
                        schema,
                        "schema",
                        |templates, name| templates.schemas.get(name).copied(),
                        |templates| {
                            templates.schemas.keys().map(|name| name.to_string()).collect()
                        },
                    ) else {
                        continue;
                    };

                    let Some((members, ..)) = self.schema_shape(template, schema.span) else {
                        continue;
                    };

                    let name = schema.inner.segments.last().unwrap().inner;

                    let variants = members
                        .iter()
                        .map(|declared| {
                            (
                                declared.name.clone(),
                                declared
                                    .side
                                    .as_ref()
                                    .map(|side| (side_decl(side), side.span)),
                            )
                        })
                        .collect::<Vec<_>>();

                    if !self.compatible(schema.span, "extended", name, &variants, link) {
                        continue;
                    }

                    composition.extend_field_with(
                        field,
                        modality_access(link.modality),
                        members
                            .into_iter()
                            .map(|declared| VariantDecl {
                                variant: declared.variant,
                                side: declared.side.as_ref().map(side_decl),
                            })
                            .collect(),
                    );
                }
            }
        }
    }

    /// Judge a schema's variants against a referencing field's modality:
    /// every side a variant names, the field must have. Each offending
    /// variant is reported; a compatible schema returns `true`.
    pub(super) fn compatible(
        &mut self,
        reference: Span,
        relation: &str,
        schema_name: &str,
        variants: &[(String, Option<(Side, Span)>)],
        link: &Link<'src>,
    ) -> bool {
        let mut compatible = true;

        for (name, side) in variants {
            let Some((side, side_span)) = side else {
                continue;
            };

            let Some(kind) = side_verdict(link.modality, *side) else {
                continue;
            };

            compatible = false;

            self.diagnostics.push(Diagnostic::incompatible_schema_variant(
                kind,
                reference,
                relation,
                side_name(*side),
                name,
                schema_name,
                &link.chain.2,
                link.modality.to_string(),
                *side_span,
                link.modality_span,
            ));
        }

        compatible
    }

    /// Resolve field resets given by variant name. Runs after [`links`], so
    /// variants a field gains from its schema are in place.
    ///
    /// [`links`]: Self::links
    pub(super) fn resets(&mut self, composition: &mut Composition) {
        let resets = std::mem::take(&mut self.resets);

        for (chain, variant, span) in resets {
            let Some((field, index)) = find_variant(composition, &chain, &variant) else {
                self.diagnostics.push(Diagnostic::unknown_variant(
                    span,
                    &chain.2,
                    &variant,
                    did_you_mean(&variant, variant_names(composition, &chain)),
                ));
                continue;
            };

            let bits = composition.get_variant(index).bits;
            composition.set_field_reset(field, bits);
        }
    }

    /// Resolve and register every pending entitlement. Every structure exists
    /// by now, so references may point anywhere in the device.
    pub(super) fn apply(&mut self, composition: &mut Composition) {
        let pending = std::mem::take(&mut self.pending);
        let tree = self.path_tree();

        for Pending {
            target,
            space,
            arrays,
        } in pending
        {
            let Some(spaces) = self.resolve_space(composition, &space, &arrays, &tree) else {
                continue;
            };

            let index = match &target {
                Target::Peripheral(peripheral) => {
                    EntitlementIndex::Peripheral(ident(peripheral).into())
                }
                Target::Field(chain) => {
                    let Some(field) = find_field(composition, chain) else {
                        continue;
                    };
                    EntitlementIndex::Field(field)
                }
                Target::Write(chain) => {
                    let Some(field) = find_field(composition, chain) else {
                        continue;
                    };
                    EntitlementIndex::Write(field)
                }
                Target::HardwareWrite(chain) => {
                    let Some(field) = find_field(composition, chain) else {
                        continue;
                    };
                    EntitlementIndex::HardwareWrite(field)
                }
                Target::Variant(chain, variant) => {
                    let Some((field, variant)) = find_variant(composition, chain, variant) else {
                        continue;
                    };
                    EntitlementIndex::Variant(field, variant)
                }
            };

            composition.entitle(index, spaces);
        }
    }

    /// Every valid entitlement-path prefix, mapped to the segments that may
    /// follow it — for narrowing unresolved paths to their failing portion,
    /// and for suggestions.
    pub(super) fn path_tree(&self) -> HashMap<VariantPath, Vec<String>> {
        let mut tree: HashMap<VariantPath, Vec<String>> = HashMap::new();

        for path in self.variants.keys() {
            for depth in 0..path.len() {
                let children = tree.entry(path[..depth].to_vec()).or_default();

                if !children.contains(&path[depth]) {
                    children.push(path[depth].clone());
                }
            }
        }

        tree
    }

    /// Report the failing portion of an unresolved entitlement path: the
    /// deepest valid prefix survives, the segment after it is the problem.
    ///
    /// The path arrives with its correspondences already resolved — the
    /// judged names are the written ones.
    pub(super) fn missing_path(
        &mut self,
        span: Span,
        path: &[(String, Span)],
        member: Option<(&'src str, Span)>,
        tree: &HashMap<VariantPath, Vec<String>>,
    ) {
        // the written segments with their spans: the path, plus the set
        // member when one names the variant
        let segments = path
            .iter()
            .map(|(segment, span)| (segment.as_str(), *span))
            .chain(member)
            .collect::<Vec<_>>();

        let written = segments
            .iter()
            .map(|(segment, ..)| segment.to_string())
            .collect::<Vec<_>>();

        let mut failing = None;

        for depth in 1..=written.len() {
            if !tree.contains_key(&written[..depth]) {
                failing = Some(depth - 1);
                break;
            }
        }

        match failing {
            Some(depth) => {
                let (segment, span) = segments[depth];
                let candidates = tree.get(&written[..depth]).cloned().unwrap_or_default();

                self.diagnostics.push(Diagnostic::unknown_path_segment(
                    span,
                    segment,
                    (depth > 0).then(|| written[..depth].join(".")),
                    did_you_mean(segment, candidates),
                ));
            }
            None => {
                // every segment is a valid prefix — the path stops short of
                // a variant
                self.diagnostics.push(Diagnostic::incomplete_path(
                    span,
                    tree.get(&written).cloned().unwrap_or_default(),
                ));
            }
        }
    }

    /// Resolve a requirement space's paths into model entitlements.
    ///
    /// The space belongs to one element of the writing definition; `arrays`
    /// are the array definitions enclosing that element, which the space's
    /// correspondences bind against.
    pub(super) fn resolve_space(
        &mut self,
        composition: &Composition,
        space: &Spanned<Space<'src>>,
        arrays: &[Enclosing],
        tree: &HashMap<VariantPath, Vec<String>>,
    ) -> Option<Vec<Vec<Entitlement>>> {
        let mut resolved = Vec::with_capacity(space.inner.patterns.len());

        for pattern in &space.inner.patterns {
            let mut entitlements = Vec::with_capacity(pattern.inner.entitlements.len());
            // `&` conjoins distinct fields; a repeat is a contradiction
            let mut fields: Vec<(FieldIndex, Span)> = Vec::new();

            for entitled in &pattern.inner.entitlements {
                let segments = match corresponding(entitled, arrays) {
                    Ok(segments) => segments,
                    Err(failure) => {
                        // a structural failure repeats identically for every
                        // element of the writer — the first speaks for all
                        if arrays.iter().all(|array| array.ordinal == 0) {
                            self.diagnostics.push(match failure {
                                Correspondence::OutsideArray(span) => {
                                    Diagnostic::correspondence_outside_array(span)
                                }
                                Correspondence::Nested(span) => {
                                    Diagnostic::nested_correspondence(
                                        span,
                                        arrays.iter().map(|array| array.span).collect(),
                                    )
                                }
                                Correspondence::Several { first, second } => {
                                    Diagnostic::several_correspondences(second, first)
                                }
                                Correspondence::Length { span, targets } => {
                                    Diagnostic::correspondence_length(
                                        span,
                                        targets,
                                        arrays[0].length,
                                        arrays[0].span,
                                    )
                                }
                            });
                        }

                        return None;
                    }
                };

                let prefix = segments
                    .iter()
                    .map(|(segment, ..)| segment.clone())
                    .collect::<Vec<_>>();

                // the variants the field must be among: the set, or the
                // path's final segment
                let members = match &entitled.inner.set {
                        Some(set) => set
                            .iter()
                            .map(|member| {
                                let mut key = prefix.clone();
                                key.push(member.inner.to_string());
                                (key, Some((member.inner, member.span)))
                            })
                            .collect(),
                        None => vec![(prefix, None)],
                    };

                let mut entitled_field = None;

                for (key, member) in members {
                    let span = member.map(|(.., span)| span).unwrap_or(entitled.span);

                    let Some((chain, variant)) = self.variants.get(&key).cloned() else {
                        self.missing_path(entitled.span, &segments, member, tree);
                        return None;
                    };

                    let Some((field, variant)) = find_variant(composition, &chain, &variant)
                    else {
                        self.diagnostics
                            .push(Diagnostic::unlocated_entitlement(span));
                        return None;
                    };

                    entitled_field = Some(field);
                    entitlements.push(Entitlement::new(field, variant));
                }

                if let Some(field) = entitled_field {
                    // the entitled field's written name: the final segment,
                    // or the one before the variant
                    let name = match &entitled.inner.set {
                        Some(..) => segments.last(),
                        None => segments.iter().rev().nth(1),
                    }
                    .map(|(segment, ..)| segment.clone())
                    .unwrap_or_default();

                    if let Some((.., first)) = fields.iter().find(|(other, ..)| *other == field) {
                        self.diagnostics
                            .push(Diagnostic::repeated_field(entitled.span, &name, *first));
                        return None;
                    }

                    fields.push((field, entitled.span));
                }
            }

            resolved.push(entitlements);
        }

        Some(resolved)
    }

    /// Detect and forbid cycles among statewise requirements.
    pub(super) fn detect_cycles(&mut self) {
        let mut adjacency: HashMap<&VariantPath, Vec<(&VariantPath, Span)>> = HashMap::new();

        for (source, target, span) in &self.edges {
            adjacency.entry(source).or_default().push((target, *span));
        }

        #[derive(Clone, Copy, PartialEq)]
        enum State {
            Visiting,
            Done,
        }

        let mut states: HashMap<&VariantPath, State> = HashMap::new();
        let mut cycles = Vec::new();

        for start in self.edges.iter().map(|(source, ..)| source) {
            if states.contains_key(start) {
                continue;
            }

            let mut stack: Vec<(&VariantPath, usize)> = vec![(start, 0)];
            let mut trail: Vec<&VariantPath> = vec![start];
            states.insert(start, State::Visiting);

            while let Some(&(node, index)) = stack.last() {
                let next = adjacency
                    .get(node)
                    .and_then(|edges| edges.get(index).copied());

                match next {
                    Some((target, span)) => {
                        stack.last_mut().unwrap().1 += 1;

                        match states.get(target) {
                            Some(State::Visiting) => {
                                let position =
                                    trail.iter().position(|node| *node == target).unwrap_or(0);

                                let cycle = trail[position..]
                                    .iter()
                                    .chain(std::iter::once(&target))
                                    .map(|path| path.join("."))
                                    .collect::<Vec<_>>()
                                    .join(" → ");

                                cycles.push((span, cycle));
                            }
                            Some(State::Done) => {}
                            None => {
                                states.insert(target, State::Visiting);
                                stack.push((target, 0));
                                trail.push(target);
                            }
                        }
                    }
                    None => {
                        states.insert(node, State::Done);
                        stack.pop();
                        trail.pop();
                    }
                }
            }
        }

        for (span, cycle) in cycles {
            self.diagnostics.push(Diagnostic::entitlement_cycle(span, cycle));
        }
    }
}
