// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

/// The source requests selected by the composed screen and its output backend.
#[derive(Debug)]
#[non_exhaustive]
pub struct StillInputs {
    /// Original physical position of the selected canvas Record.
    pub canvas_record: u32,
    /// Texture paths of successfully drawn or embedded bitmap inputs.
    pub textures: BTreeSet<String>,
    /// Vector paths of successfully drawn or emitted items.
    pub vectors: BTreeSet<String>,
    /// Font selections and translations supplying accepted painting or layout.
    pub text: BTreeSet<StillTextInput>,
}

/// One selected font and the optional language lookup contributing to screen text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub struct StillTextInput {
    /// The selected font ordinal in the supplied font movie.
    pub font: usize,
    /// The authored lookup key when its translated value was used.
    pub localization_key: Option<Box<str>>,
}

#[derive(Debug, Default)]
pub(crate) struct TrackedInputs {
    pub(crate) textures: BTreeMap<u16, String>,
    pub(crate) vectors: BTreeMap<usize, String>,
    pub(crate) text_items: Vec<(Range<usize>, StillTextInput)>,
    pub(crate) text_layout: BTreeSet<StillTextInput>,
}

impl TrackedInputs {
    pub(crate) fn selected(
        mut self,
        canvas_record: u32,
        bitmaps: &[u16],
        mut items: Vec<usize>,
    ) -> StillInputs {
        items.sort_unstable();
        let text = self
            .text_items
            .into_iter()
            .filter(|(range, _)| {
                items
                    .get(items.partition_point(|item| *item < range.start))
                    .is_some_and(|item| *item < range.end)
            })
            .map(|(_, input)| input)
            .chain(self.text_layout)
            .collect();
        StillInputs {
            canvas_record,
            textures: bitmaps
                .iter()
                .filter_map(|id| self.textures.remove(id))
                .collect(),
            vectors: items
                .iter()
                .filter_map(|item| self.vectors.remove(item))
                .collect(),
            text,
        }
    }
}
