// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_dataforge::DataForge;
use p4kx_screens::{ScreenComposer, TextSources};
use p4kx_testkit::{
    TestCanvas, TestCondition, TestModifier, TestRecord, TestStyle, TestStyleEntry, TestValue,
    TestWidget, named_tag_record, screens_text_sample,
};

use super::{COMPONENT, FILLED, FOLDER, GHOST, LEFT, TOP, bind, component, parameter, tag_switch};
use crate::support::{BLUE, SCREEN, TestResult, blue_at, guid_text, stage};
use crate::text::fonts;
use crate::text_part::{field, field_entry};

const BUTTON: &str = "BuildingBlocks_ComponentGeneralButton";
const PRIMARY: u128 = SCREEN + 20;
const PROXY: u128 = SCREEN + 21;
const BRAND: u128 = SCREEN + 22;
const SHEET: u128 = SCREEN + 23;

fn template() -> TestRecord {
    let operations = [
        parameter("Integer", "fill", "ParamInput3", TestValue::Int64(432)),
        tag_switch("fill tag", "fill", [(432, FILLED), (111, GHOST)], FILLED),
        bind("String", "root", "PrimaryStateTag", "fill tag"),
        parameter("Integer", "position", "ParamInput4", TestValue::Int64(4)),
        tag_switch("position tag", "position", [(4, TOP), (82, LEFT)], TOP),
        bind("String", "root", "SecondaryStateTag", "position tag"),
        parameter(
            "Boolean",
            "show label",
            "ParamInput0",
            TestValue::Bool(true),
        ),
        bind("Boolean", "label", "IsActive", "show label"),
        parameter("Boolean", "show icon", "ParamInput1", TestValue::Bool(true)),
        bind("Boolean", "icon", "IsActive", "show icon"),
        parameter(
            "Localized",
            "label text",
            "ParamInput0",
            TestValue::text("@fixture_default"),
        ),
        bind("Localized", "label", "ParamInput0", "label text"),
    ];
    TestCanvas::new(COMPONENT, "buttongeneralcomponentstandard", 32.0, 20.0)
        .in_folder(FOLDER)
        .raw()
        .brand_group(BRAND, Some(SHEET), Vec::new())
        .widget(TestWidget::display("root").tag(PROXY))
        .widget(TestWidget::new("BuildingBlocks_WidgetIcon", "icon").parent("root"))
        .widget(field("label", 12.0))
        .record()
        .value("proxyRootNode", TestValue::pointer("root"))
        .value(
            "operations",
            TestValue::Array(operations.into_iter().map(TestValue::Struct).collect()),
        )
}

fn fixture(show_label: bool) -> Vec<u8> {
    let frame = TestStyleEntry::new("default primary frame")
        .when(TestCondition::Tag(PROXY))
        .when(TestCondition::Parent(vec![TestCondition::AnyOfTag(vec![
            PRIMARY,
        ])]))
        .when(TestCondition::Tag(FILLED))
        .when(TestCondition::Tag(TOP))
        .set(TestModifier::EnableBackground(true))
        .set(TestModifier::Background(BLUE));
    let label = field_entry().when(TestCondition::Ancestor(vec![TestCondition::Tag(PRIMARY)]));
    let sheet = TestStyle::new(SHEET, "fixture_primary_style", Vec::new())
        .entry(frame)
        .entry(label);
    screens_text_sample()
        .record(component(BUTTON, "Filled", "Top", show_label))
        .record(template())
        .record(named_tag_record(
            PRIMARY,
            "UI.Style._Standard.ModularKit.Components.General-Button-Primary",
        ))
        .record(named_tag_record(PROXY, "Fixture.Proxy"))
        .record(named_tag_record(FILLED, "FillStyle_Filled"))
        .record(named_tag_record(GHOST, "FillStyle_Ghost"))
        .record(named_tag_record(TOP, "IconPos_Top"))
        .record(named_tag_record(LEFT, "IconPos_Left"))
        .record(TestStyle::new(BRAND, "s_fixture_hud", Vec::new()).record())
        .record(sheet.record())
        .build()
}

#[test]
fn general_button_draws_the_primary_frame_and_respects_bound_label_visibility() -> TestResult {
    let fonts = fonts()?;
    for (show_label, texts) in [(true, 1), (false, 0)] {
        let bytes = fixture(show_label);
        let forge = DataForge::open(&bytes)?;
        let used_label = Cell::new(false);
        let localise = |key: &str| {
            if key == "fixture_button_label" {
                used_label.set(true);
                Some("A".to_owned())
            } else {
                None
            }
        };
        let composed = ScreenComposer::new(&forge)
            .with_texts(TextSources {
                fonts: &fonts,
                localise: Some(&localise),
            })
            .compose(&guid_text(SCREEN), Some("fixture"))?;
        let image = composed.rasterize(&stage())?.image;
        assert!(
            blue_at(&image, 29, 17),
            "the shared Primary style paints the proxy frame in both label states"
        );
        assert_eq!(
            used_label.get(),
            show_label,
            "a visible text field localises the original holder label"
        );
        assert_eq!(composed.report().texts_drawn, texts, "label draw count");
    }
    Ok(())
}
