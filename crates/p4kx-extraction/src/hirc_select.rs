// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selects one page of the `HIRC` objects of a soundbank and counts how far the reader decodes
//! them.

use p4kx_audio::{HircKind, HircLayout, HircObject};
use schemars::JsonSchema;
use serde::Serialize;

use crate::bank::Bank;
use crate::hirc_view::ObjectView;

/// How many objects of a whole bank the reader decodes, does not decode, does not support in the
/// bank's version or finds malformed.
#[derive(Debug, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct LayoutCounts {
    /// Objects whose fields the reader read.
    pub decoded: usize,
    /// Objects of a type the reader has no decoder for.
    pub undecoded: usize,
    /// Objects of a type the reader decodes, but not in this bank version.
    pub unsupported: usize,
    /// Objects whose body does not hold the fields of their type.
    pub malformed: usize,
}

impl LayoutCounts {
    /// Counts the objects by [`HircObject::layout`].
    #[must_use]
    pub fn of(objects: &[HircObject]) -> Self {
        let mut counts = Self::default();
        for object in objects {
            let count = match object.layout() {
                HircLayout::Decoded => &mut counts.decoded,
                HircLayout::Undecoded => &mut counts.undecoded,
                HircLayout::Unsupported => &mut counts.unsupported,
                HircLayout::Malformed => &mut counts.malformed,
            };
            *count = count.saturating_add(1);
        }
        counts
    }
}

/// One page of the objects that pass the filters.
#[derive(Debug)]
pub struct ObjectPage<'a> {
    /// How many objects pass the filters, on this page and beyond it.
    pub matched: usize,
    /// How many matching objects precede the page.
    pub offset: usize,
    /// The objects of the page in bank order.
    pub shown: Vec<ObjectView<'a>>,
}

impl ObjectPage<'_> {
    /// Returns the offset of the page after this one, `None` on the last page.
    #[must_use]
    pub fn next_offset(&self) -> Option<usize> {
        let next = self.offset.saturating_add(self.shown.len());
        (next < self.matched).then_some(next)
    }
}

/// Returns the page of at most `limit` objects from the `offset`-th match on, among the objects
/// of `kind` (any if `None`) and with `id` (any if `None`), in bank order.
#[must_use]
pub fn select(
    objects: &[HircObject],
    kind: Option<HircKind>,
    id: Option<u32>,
    offset: usize,
    limit: usize,
) -> ObjectPage<'_> {
    let mut matched = 0_usize;
    let mut shown = Vec::new();
    let chosen = objects.iter().enumerate().filter(|(_, object)| {
        kind.is_none_or(|kind| object.kind == kind) && id.is_none_or(|id| object.id == id)
    });
    for (index, object) in chosen {
        if matched >= offset && shown.len() < limit {
            shown.push(ObjectView::new(index, object));
        }
        matched = matched.saturating_add(1);
    }
    ObjectPage {
        matched,
        offset,
        shown,
    }
}

/// Selects one page with checked physical source fields from the owning bank.
#[must_use]
pub fn select_with_sources(
    bank: &Bank,
    kind: Option<HircKind>,
    id: Option<u32>,
    offset: usize,
    limit: usize,
) -> ObjectPage<'_> {
    let mut page = select(bank.soundbank().objects(), kind, id, offset, limit);
    for object in &mut page.shown {
        object.attach_source_fields(bank);
    }
    page
}

#[cfg(test)]
mod tests {
    use p4kx_audio::Soundbank;
    use p4kx_testkit::{TestBank, TestHircObject};

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn bank_bytes() -> Vec<u8> {
        TestBank::new(1)
            .object(TestHircObject::sound(10, 100, 2, 4096))
            .object(TestHircObject::event(11, &[12]))
            .object(TestHircObject::sound(12, 101, 2, 4096))
            .object(TestHircObject::raw(0xFE, 13, [0; 4]))
            .bytes()
    }

    #[test]
    fn a_page_holds_the_matches_in_bank_order_with_their_bank_index() -> TestResult {
        let bank = Soundbank::parse(bank_bytes())?;
        let page = select(bank.objects(), Some(HircKind::Sound), None, 0, 10);
        let found: Vec<(usize, u32)> = page.shown.iter().map(|o| (o.index, o.id)).collect();
        assert_eq!(found, [(0, 10), (2, 12)], "the Sounds and their positions");
        assert_eq!((page.matched, page.next_offset()), (2, None), "one page");
        Ok(())
    }

    #[test]
    fn paging_continues_at_the_next_offset() -> TestResult {
        let bank = Soundbank::parse(bank_bytes())?;
        let first = select(bank.objects(), None, None, 0, 3);
        assert_eq!(
            (first.shown.len(), first.next_offset()),
            (3, Some(3)),
            "first"
        );
        let rest = select(bank.objects(), None, None, 3, 3);
        assert_eq!((rest.shown.len(), rest.next_offset()), (1, None), "last");
        let beyond = select(bank.objects(), None, None, 9, 3);
        assert_eq!((beyond.shown.len(), beyond.matched), (0, 4), "past the end");
        Ok(())
    }

    #[test]
    fn the_id_filter_narrows_the_kind_filter() -> TestResult {
        let bank = Soundbank::parse(bank_bytes())?;
        let page = select(bank.objects(), Some(HircKind::Event), Some(12), 0, 10);
        assert_eq!(page.matched, 0, "object 12 is no Event");
        assert_eq!(
            select(bank.objects(), None, Some(12), 0, 10).matched,
            1,
            "any type"
        );
        Ok(())
    }

    #[test]
    fn objects_the_reader_does_not_decode_are_counted_and_listed() -> TestResult {
        let bank = Soundbank::parse(bank_bytes())?;
        let counts = LayoutCounts::of(bank.objects());
        assert_eq!(
            counts,
            LayoutCounts {
                decoded: 3,
                undecoded: 1,
                unsupported: 0,
                malformed: 0
            },
            "three decoded, one raw"
        );
        let page = select(bank.objects(), None, Some(13), 0, 10);
        assert_eq!(page.shown.len(), 1, "the raw object is listed");
        Ok(())
    }
}
