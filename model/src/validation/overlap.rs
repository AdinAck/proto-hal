//! Utilities for overlap validation.

use std::{iter, ops::Range};

use crate::WideWord;

/// A sibling and the range of bytes or bits it occupies within its parent.
pub(crate) struct Sibling<Id> {
    pub(crate) handle: Id,
    pub(crate) range: Range<WideWord>,
}

/// A region occupied by more than one sibling.
pub(crate) struct Region<Id> {
    pub(crate) range: Range<WideWord>,
    pub(crate) occupants: Vec<Id>,
}

/// Where a sibling starts or ends occupying its parent.
pub enum Bound<Id> {
    Start(Id),
    End(Id),
}

/// Find the regions occupied by more than one of the provided siblings, producing them in order.
///
/// *Note: Regions are delineated by the bounds where siblings start or end, so adjacent regions have unequal sets of
/// occupants.*
pub(crate) fn shared_regions<Id: Copy + PartialEq>(
    siblings: impl IntoIterator<Item = Sibling<Id>>,
) -> impl Iterator<Item = Region<Id>> {
    // 1. the bounds where siblings start or end, in order
    let mut bounds = siblings
        .into_iter()
        .filter(|sibling| !sibling.range.is_empty())
        .flat_map(|sibling| {
            [
                (sibling.range.start, Bound::Start(sibling.handle)),
                (sibling.range.end, Bound::End(sibling.handle)),
            ]
        })
        .collect::<Vec<_>>();

    bounds.sort_unstable_by_key(|&(position, ..)| position);

    let mut bounds = bounds.into_iter().peekable();
    let mut active = Vec::new();

    iter::from_fn(move || {
        while let Some((start, bound)) = bounds.next() {
            // 2. activate the sibling starting at this bound, or deactivate the sibling ending at it
            match bound {
                Bound::Start(handle) => active.push(handle),
                Bound::End(handle) => active.retain(|&sibling| sibling != handle),
            }

            // 3. the region between this bound and the next is shared when more than one sibling is active
            let (end, ..) = *bounds.peek()?;

            if start < end && active.len() > 1 {
                return Some(Region {
                    range: start..end,
                    occupants: active.clone(),
                });
            }
        }

        None
    })
}
