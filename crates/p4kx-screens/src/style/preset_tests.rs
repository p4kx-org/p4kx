// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_extraction::SourceLocation;
use p4kx_testkit::{
    TestCanvas, TestColour, TestCondition, TestModifier, TestStyle, TestStyleEntry,
    named_tag_record, screens_text_sample,
};

use super::{PresetStyles, TAG_PATH};
use crate::still_inputs::{StillTextInput, TrackedInputs};
use crate::style::TagPaths;

const KIT: u128 = 0xcc01;
const TAG: u128 = 0xcc02;
const BRAND: u128 = 0xcc03;
const OTHER: u128 = 0xcc04;

#[test]
fn only_consumed_preset_fill_keeps_the_original_branch_and_modifier() -> Result<(), Box<dyn Error>>
{
    let entry = TestStyleEntry::new("colour")
        .when(TestCondition::Tag(TAG))
        .set(TestModifier::FontSize(8.0))
        .set(TestModifier::Fill(TestColour::Slot("Accent1", 1.0)));
    let kit = TestCanvas::new(KIT, "fixture_kit", 20.0, 20.0)
        .brand_style(OTHER, Vec::new())
        .brand_style(BRAND, vec![TestStyleEntry::new("empty"), entry]);
    let bytes = screens_text_sample()
        .record(kit.record())
        .record(named_tag_record(TAG, &format!("{TAG_PATH}heading1")))
        .record(TestStyle::new(BRAND, "s_test_hud", Vec::new()).record())
        .record(TestStyle::new(OTHER, "s_other_hud", Vec::new()).record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge
        .records()
        .find(|record| record.name().ok() == Some("BuildingBlocks_Canvas.fixture_kit"))
        .ok_or("standard fixture Canvas")?;
    let canvas = record.tree()?;
    let presets = PresetStyles::read(&canvas, &TagPaths::build(&forge), Some("test"));
    let fill = presets
        .style("Heading1")
        .and_then(|preset| preset.fill.as_ref())
        .ok_or("original branded FillColor")?;
    let location = SourceLocation::DataForge {
        record_index: record.index(),
        field: "brandStyles[1].entries[1].modifiers[1].color".into(),
    };
    assert_eq!(
        fill.source.location(),
        location,
        "the omitted empty entry and preceding brand do not compact the original coordinates"
    );
    let font = StillTextInput {
        font: 0,
        localization_key: None,
        preset_fill: None,
    };
    let tracked = |item| {
        let mut inputs = TrackedInputs::default();
        inputs.text_layout.insert(font.clone());
        inputs.record_text_paint(5..9, &font, Some(fill.source));
        inputs.selected(record.index(), &[], vec![item])
    };
    assert!(
        tracked(6)
            .text
            .iter()
            .any(|input| input.preset_fill == Some(fill.source)),
        "accepted glyph paint contributes the exact preset colour"
    );
    assert!(
        tracked(9)
            .text
            .iter()
            .all(|input| input.preset_fill.is_none()),
        "unaccepted paint and mere text measurement do not contribute the colour"
    );
    Ok(())
}
