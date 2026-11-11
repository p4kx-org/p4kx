// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(crate) struct PropertyLabels {
    pub(crate) count: &'static str,
    pub(crate) checked_ids: &'static str,
    pub(crate) checked_words: &'static str,
    pub(crate) coverage: &'static str,
    pub(crate) id: &'static str,
    pub(crate) position: &'static str,
    pub(crate) word_layout: &'static str,
    pub(crate) scalar_bits: &'static str,
    pub(crate) minimum_bits: &'static str,
    pub(crate) maximum_bits: &'static str,
}

pub(crate) const NODE: PropertyLabels = PropertyLabels {
    count: "nodePropertyCount",
    checked_ids: "nodePropertyCheckedIds",
    checked_words: "nodePropertyCheckedWords",
    coverage: "nodePropertyCoverage",
    id: "nodePropertyId",
    position: "nodePropertyPosition",
    word_layout: "nodePropertyWordLayout",
    scalar_bits: "nodePropertyScalarBits",
    minimum_bits: "nodePropertyMinimumBits",
    maximum_bits: "nodePropertyMaximumBits",
};

pub(crate) const ACTION: PropertyLabels = PropertyLabels {
    count: "actionPropertyCount",
    checked_ids: "actionPropertyCheckedIds",
    checked_words: "actionPropertyCheckedWords",
    coverage: "actionPropertyCoverage",
    id: "actionPropertyId",
    position: "actionPropertyPosition",
    word_layout: "actionPropertyWordLayout",
    scalar_bits: "actionPropertyScalarBits",
    minimum_bits: "actionPropertyMinimumBits",
    maximum_bits: "actionPropertyMaximumBits",
};
