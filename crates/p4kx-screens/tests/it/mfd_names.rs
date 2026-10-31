// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! What names the page in the tab bar of an MFD and which labels a filled field shows (ADR-0306).

use p4kx_dataforge::DataForge;
use p4kx_gfx::RgbaImage;
use p4kx_screens::{EDITOR_LABEL, ScreenComposer, TextSources};
use p4kx_testkit::{
    TestCanvas, TestDim, TestInstance, TestOp, TestValue, TestWidget, screens_text_sample,
    text_localisation,
};

use crate::mfd_pages::{Failure, variables};
use crate::support::{SCREEN, TestResult, guid_text, near, pixel, stage};
use crate::text::fonts;
use crate::text_part::field_entry;

const CLEAR: [u8; 4] = [0, 0, 0, 0];

fn labelled_field(name: &'static str, label: &str) -> TestWidget {
    TestWidget::new("BuildingBlocks_WidgetTextField", name)
        .parent("root")
        .size(TestDim::Fixed(20.0), TestDim::Fixed(20.0))
        .with(
            "labelProperties",
            TestValue::Struct(
                TestInstance::new("BuildingBlocks_ComponentLabelProperties")
                    .value("label", TestValue::text(label)),
            ),
        )
}

fn text_of(canvas: &TestCanvas, settings: &[&str]) -> Result<RgbaImage, Failure> {
    localised_text_of(canvas, settings, None)
}

fn localised_text_of(
    canvas: &TestCanvas,
    settings: &[&str],
    localise: Option<p4kx_gfx::Localise<'_>>,
) -> Result<RgbaImage, Failure> {
    let bytes = screens_text_sample().record(canvas.record()).build();
    let forge = DataForge::open(&bytes)?;
    let fonts = fonts()?;
    let composed = ScreenComposer::new(&forge)
        .with_variables(&variables(settings)?)
        .with_texts(TextSources {
            fonts: &fonts,
            localise,
        })
        .compose(&guid_text(SCREEN), None)?;
    Ok(composed.rasterize(&stage())?.image)
}

fn editor_label() -> String {
    format!("@{EDITOR_LABEL}")
}

fn titled(label: &str, source: TestOp) -> TestCanvas {
    TestCanvas::new(SCREEN, "title", 20.0, 20.0)
        .embedded(field_entry())
        .operation("source", source)
        .widget(TestWidget::display("root"))
        .widget(labelled_field("title", label))
        .operation("bind", TestOp::LocalField("title", "ParamInput0", "source"))
}

#[test]
fn a_localised_variable_gives_the_text_a_field_draws_in_place_of_its_label() -> TestResult {
    let canvas = titled(&editor_label(), TestOp::NamedLocalVar("mfdviewname"));
    assert_eq!(
        pixel(&text_of(&canvas, &[])?, 2, 4),
        CLEAR,
        "the editor label of a field that is filled in shows nothing yet"
    );
    let named = text_of(&canvas, &["mfdviewname=A"])?;
    assert!(near(pixel(&named, 2, 4), [255, 0, 0, 255], 2), "the name");
    Ok(())
}

#[test]
fn the_default_of_a_localised_parameter_is_the_text_unless_it_is_a_placeholder() -> TestResult {
    let named = text_of(
        &titled(&editor_label(), TestOp::TextParam("ParamInput0", "A")),
        &[],
    )?;
    assert!(
        near(pixel(&named, 2, 4), [255, 0, 0, 255], 2),
        "the default"
    );
    let kept = text_of(
        &titled("A", TestOp::TextParam("ParamInput0", "@LOC_PLACEHOLDER")),
        &[],
    )?;
    assert!(
        near(pixel(&kept, 2, 4), [255, 0, 0, 255], 2),
        "a placeholder leaves a real label as it is"
    );
    Ok(())
}

#[test]
fn a_localisation_key_named_by_the_variable_is_localised() -> TestResult {
    let canvas = titled(&editor_label(), TestOp::NamedLocalVar("mfdviewname"));
    let literal = localised_text_of(&canvas, &["mfdviewname=A"], Some(&text_localisation))?;
    let key = localised_text_of(
        &canvas,
        &["mfdviewname=@test_hello"],
        Some(&text_localisation),
    )?;
    let unresolved = localised_text_of(&canvas, &["mfdviewname=@test_hello"], None)?;
    assert_ne!(key.pixels(), literal.pixels(), "the key is looked up");
    assert_ne!(key.pixels(), unresolved.pixels(), "and not drawn as it is");
    Ok(())
}
