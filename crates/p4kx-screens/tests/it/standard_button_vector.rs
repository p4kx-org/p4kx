// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::error::Error;

use p4kx_dataforge::{DataForge, Value};
use p4kx_gfx::RgbaImage;
use p4kx_screens::{Composed, ImageSources, ScreenComposer};
use p4kx_testkit::{
    TestCanvas, TestCondition, TestDim, TestInstance, TestModifier, TestRecord, TestStyle,
    TestStyleEntry, TestValue, TestWidget, named_tag_record,
};

use super::{COMPONENT, FILLED, FOLDER, GHOST, LEFT, TOP, button, template_operations};
use crate::images::texture;
use crate::support::{
    SCREEN, TestResult, blue_at, database, empty_at, green_at, guid_text, red_at, stage,
};

const PROXY_TAG: u128 = SCREEN + 20;
const CLASS_TAG: u128 = SCREEN + 21;
const ALTERNATE_CLASS_TAG: u128 = SCREEN + 22;
const BRAND: u128 = SCREEN + 23;
const BRAND_SHEET: u128 = SCREEN + 24;
const SECONDARY_CLASS: &str = "UI.Style._Standard.ModularKit.Components.General-Button-Secondary";
const UNRELATED_CLASS: &str = "Fixture.UnrelatedComponent";
const FILLED_SVG: &str = "UI/Test/secondary-filled.svg";
const GHOST_SVG: &str = "UI/Test/secondary-ghost.svg";

fn svg_fill(path: &str, render_shape: bool) -> TestInstance {
    TestInstance::new("BuildingBlocks_SvgFill")
        .value("svgPath", TestValue::text(path))
        .value("renderShape", TestValue::Bool(render_shape))
        .value("scalingBehavior", TestValue::text("Contain"))
}

fn proxy_template(render_shape: bool) -> TestRecord {
    TestCanvas::new(
        COMPONENT,
        "buttongeneralsecondarycomponentstandard",
        32.0,
        20.0,
    )
    .in_folder(FOLDER)
    .raw()
    .brand_group(BRAND, Some(BRAND_SHEET), Vec::new())
    .widget(
        TestWidget::display("root")
            .tag(PROXY_TAG)
            .size(TestDim::Fixed(32.0), TestDim::Fixed(20.0))
            .with("svgFill", TestValue::Struct(svg_fill("", render_shape))),
    )
    .widget(TestWidget::display("icon").parent("root"))
    .widget(TestWidget::display("label").parent("root"))
    .record()
    .value("proxyRootNode", TestValue::pointer("root"))
    .value("operations", template_operations())
}

fn frame_rule(name: &str, state: u128, path: &'static str) -> TestStyleEntry {
    TestStyleEntry::new(name)
        .when(TestCondition::Tag(PROXY_TAG))
        .when(TestCondition::Parent(vec![TestCondition::AnyOfTag(vec![
            ALTERNATE_CLASS_TAG,
            CLASS_TAG,
        ])]))
        .when(TestCondition::Tag(state))
        .set(TestModifier::SvgPath(path))
}

fn fixture(fill: &str, class_name: &str, render_shape: bool) -> Vec<u8> {
    let sheet = TestStyle::new(BRAND_SHEET, "fixture_component_style", Vec::new())
        .entry(
            frame_rule("filled frame", FILLED, FILLED_SVG)
                .set(TestModifier::Enumerated("SvgScalingBehavior", "StretchXY")),
        )
        .entry(frame_rule("ghost frame", GHOST, GHOST_SVG));
    database([
        button(fill, "Left", false),
        proxy_template(render_shape),
        named_tag_record(FILLED, "FillStyle_Filled"),
        named_tag_record(GHOST, "FillStyle_Ghost"),
        named_tag_record(LEFT, "IconPos_Left"),
        named_tag_record(TOP, "IconPos_Top"),
        named_tag_record(PROXY_TAG, "Fixture.Proxy"),
        named_tag_record(CLASS_TAG, class_name),
        named_tag_record(ALTERNATE_CLASS_TAG, "Fixture.AlternateComponent"),
        TestStyle::new(BRAND, "s_fixture_hud", Vec::new()).record(),
        sheet.record(),
    ])
}

fn assert_proxy_source(forge: &DataForge<'_>, render_shape: bool) -> TestResult {
    let tree = forge
        .record(1)
        .ok_or("no original template Record")?
        .tree()?;
    let Some(Value::Array(scene)) = tree.field("scene") else {
        return Err("no original template scene".into());
    };
    let Some(Value::Struct(proxy)) = scene.first() else {
        return Err("no original proxy widget".into());
    };
    assert_eq!(
        proxy.type_name(),
        "BuildingBlocks_DisplayWidget",
        "the original fill is declared on the base proxy type",
    );
    let Some(Value::Struct(svg)) = proxy.field("svgFill") else {
        return Err("the proxy lacks its original svgFill".into());
    };
    assert!(
        matches!(svg.field("renderShape"), Some(Value::Bool(value)) if *value == render_shape),
        "the nested field owns the authored visibility"
    );
    assert!(
        matches!(svg.field("svgPath"), Some(Value::String(""))),
        "the source path is empty and the stylesheet supplies it"
    );
    assert!(
        matches!(svg.field("scalingBehavior"), Some(Value::String("Contain"))),
        "Contain is the original geometry fallback"
    );
    Ok(())
}

fn compose_frame(
    state: &str,
    class_name: &str,
    render_shape: bool,
) -> Result<(Composed, Vec<String>), Box<dyn Error>> {
    let bytes = fixture(state, class_name, render_shape);
    let forge = DataForge::open(&bytes)?;
    assert_proxy_source(&forge, render_shape)?;
    let reads = RefCell::new(Vec::new());
    let vector = |path: &str| {
        let fill = match path {
            FILLED_SVG => "blue",
            GHOST_SVG => "lime",
            _ => return Err("unexpected vector Entry".to_owned()),
        };
        reads.borrow_mut().push(path.to_owned());
        Ok(format!(
            r#"<svg viewBox="0 0 10 20"><rect width="10" height="20" fill="{fill}"/></svg>"#
        )
        .into_bytes())
    };
    let composed = ScreenComposer::new(&forge)
        .with_images(ImageSources {
            texture: &texture,
            vector: &vector,
        })
        .compose(&guid_text(SCREEN), Some("fixture"))?;
    Ok((composed, reads.into_inner()))
}

#[test]
fn secondary_proxy_draws_the_class_gated_svg_with_the_selected_fit() -> TestResult {
    let (filled, reads) = compose_frame("Filled", SECONDARY_CLASS, true)?;
    assert_eq!(
        reads,
        [FILLED_SVG],
        "the Filled rule selects its distinct SVG"
    );
    let image = filled.rasterize(&stage())?.image;
    assert!(blue_at(&image, 5, 10), "the proxy uses the Filled vector");
    assert!(blue_at(&image, 29, 10), "StretchXY fills the wide proxy");
    assert!(
        empty_at(&image, 36, 10),
        "the vector stays inside the proxy"
    );

    let (ghost, reads) = compose_frame("Ghost", SECONDARY_CLASS, true)?;
    assert_eq!(
        reads,
        [GHOST_SVG],
        "the Ghost rule selects its distinct SVG"
    );
    let image = ghost.rasterize(&stage())?.image;
    assert!(green_at(&image, 5, 10), "the proxy uses the Ghost vector");
    assert!(
        empty_at(&image, 29, 10),
        "authored Contain preserves the aspect ratio"
    );
    Ok(())
}

#[test]
fn secondary_proxy_requires_its_holder_class_and_original_render_flag() -> TestResult {
    let (unmatched, reads) = compose_frame("Filled", UNRELATED_CLASS, true)?;
    assert_eq!(
        reads.as_slice(),
        &[] as &[String],
        "neither parent alternative matches"
    );
    assert!(empty_at(&unmatched.rasterize(&stage())?.image, 5, 10));

    let (hidden, _) = compose_frame("Filled", SECONDARY_CLASS, false)?;
    assert_eq!(
        hidden.report().images_drawn,
        0,
        "renderShape disables the proxy"
    );
    assert!(empty_at(&hidden.rasterize(&stage())?.image, 5, 10));
    Ok(())
}

fn sliced_frame(edge: Option<f32>) -> Result<RgbaImage, Box<dyn Error>> {
    const SVG: &str = r#"<svg viewBox="0 0 10 20"><rect width="10" height="20" fill="blue"/><rect width="4" height="20" fill="red"/></svg>"#;
    let rect = TestInstance::new("BuildingBlocks_TRBL")
        .value("top", TestValue::Single(0.25))
        .value("right", TestValue::Single(0.75))
        .value("bottom", TestValue::Single(0.75))
        .value("left", TestValue::Single(0.25));
    let fill = svg_fill(FILLED_SVG, true)
        .value("enableNineSliceRect", TestValue::Bool(true))
        .value("nineSliceScale", TestValue::Single(1.0))
        .value("nineSliceRect", TestValue::Struct(rect));
    let mut style = TestStyleEntry::new("stretch")
        .set(TestModifier::Enumerated("SvgScalingBehavior", "StretchXY"));
    if let Some(edge) = edge {
        style = style.set(TestModifier::Number("NineSliceLeft", edge));
    }
    let canvas = TestCanvas::new(SCREEN, "sliced display", 32.0, 20.0)
        .raw()
        .embedded(style)
        .widget(
            TestWidget::display("fill")
                .size(TestDim::Fixed(32.0), TestDim::Fixed(20.0))
                .with("svgFill", TestValue::Struct(fill)),
        );
    let bytes = database([canvas.record()]);
    let forge = DataForge::open(&bytes)?;
    let screen = ScreenComposer::new(&forge)
        .with_images(ImageSources {
            texture: &texture,
            vector: &|_| Ok(SVG.as_bytes().to_vec()),
        })
        .compose(&guid_text(SCREEN), None)?;
    Ok(screen.rasterize(&stage())?.image)
}

#[test]
fn display_vector_uses_the_styled_nine_slice_edge() -> TestResult {
    let authored = sliced_frame(None)?;
    let overridden = sliced_frame(Some(0.5))?;
    assert!(
        red_at(&authored, 7, 10),
        "the authored edge stretches red across the sample"
    );
    assert!(
        blue_at(&overridden, 7, 10),
        "the wider styled cap retains the red bar's width"
    );
    assert!(
        red_at(&overridden, 2, 10),
        "the red cap itself is preserved"
    );
    Ok(())
}
