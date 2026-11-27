// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::{Cell, RefCell};
use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_screens::{Composed, ImageSources, ScreenComposer, TextSources};
use p4kx_testkit::{
    TestCanvas, TestCondition, TestDim, TestInstance, TestRecord, TestValue, TestWidget,
    named_tag_record, screens_text_sample,
};

use super::{COMPONENT, FOLDER, bind, parameter};
use crate::images::texture;
use crate::support::{SCREEN, TestResult, guid_text};
use crate::text::fonts;
use crate::text_part::{field, field_entry};

const LARGE: &str = "BuildingBlocks_ComponentGeneralButtonLarge";
const LARGE_TAG: u128 = SCREEN + 25;
const FRAME_SVG: &str = "UI/Test/large-frame.svg";
const EDGE_SVG: &str = "UI/Test/large-edge.svg";
const WRAP_SVG: &str = r#"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="blue"/></svg>"#;

fn holder(show: bool, canvas: Option<u128>) -> TestRecord {
    let label = TestInstance::new("BuildingBlocks_ComponentLabelProperties")
        .value("show", TestValue::Bool(show))
        .value("label", TestValue::text("@fixture_large_label"))
        .value("scale", TestValue::Single(1.0));
    let icon = TestInstance::new("BuildingBlocks_ComponentIconProperties")
        .value("show", TestValue::Bool(false));
    TestCanvas::new(SCREEN, "large_buttons", 40.0, 20.0)
        .widget(
            TestWidget::new(LARGE, "large button")
                .size(TestDim::Fixed(32.0), TestDim::Fixed(20.0))
                .with("sizingMethod", TestValue::text("Size"))
                .with(
                    "canvas",
                    canvas.map_or(TestValue::Null, TestValue::Reference),
                )
                .with("labelProperties", TestValue::Struct(label))
                .with("iconProperties", TestValue::Struct(icon)),
        )
        .record()
}

fn shape(name: &str, path: &str) -> TestWidget {
    TestWidget::new("BuildingBlocks_WidgetCustomShape", name.to_owned())
        .parent("root")
        .size(TestDim::Fixed(24.0), TestDim::Fixed(16.0))
        .with("svgPath", TestValue::text(path))
        .with("renderShape", TestValue::Bool(true))
}

fn template(invalid_proxy: bool) -> TestRecord {
    let label_style =
        field_entry().when(TestCondition::Ancestor(vec![TestCondition::Tag(LARGE_TAG)]));
    let operations = [
        parameter(
            "Localized",
            "text",
            "ParamInput0",
            TestValue::text("@fixture_default"),
        ),
        bind("Localized", "label", "ParamInput0", "text"),
        parameter(
            "Boolean",
            "label visibility",
            "ParamInput0",
            TestValue::Bool(true),
        ),
        bind("Boolean", "label", "IsActive", "label visibility"),
        parameter(
            "Boolean",
            "icon visibility",
            "ParamInput1",
            TestValue::Bool(true),
        ),
        bind("Boolean", "icon", "IsActive", "icon visibility"),
        parameter(
            "Number",
            "label scale",
            "ParamInput5",
            TestValue::Single(1.0),
        ),
        bind("Number", "label", "ScaleX", "label scale"),
        bind("Number", "label", "ScaleY", "label scale"),
    ];
    let mut record = TestCanvas::new(COMPONENT, "buttongenerallargecomponentstandard", 32.0, 20.0)
        .in_folder(FOLDER)
        .raw()
        .embedded(label_style)
        .widget(TestWidget::display("root"))
        .widget(field("label", 12.0).with("layer", TestValue::Int32(10)))
        .widget(shape("frame", FRAME_SVG))
        .widget(shape("edge", EDGE_SVG))
        .widget(TestWidget::new("BuildingBlocks_WidgetIcon", "icon").parent("root"))
        .record()
        .value(
            "operations",
            TestValue::Array(operations.into_iter().map(TestValue::Struct).collect()),
        );
    if invalid_proxy {
        record = record.value("proxyRootNode", TestValue::pointer("label"));
    }
    record
}

struct Rendered {
    composed: Composed,
    vectors: Vec<String>,
    localized: bool,
}

fn compose(
    show: bool,
    target: Option<u128>,
    invalid_proxy: bool,
) -> Result<Rendered, Box<dyn Error>> {
    let bytes = screens_text_sample()
        .record(holder(show, target))
        .record(template(invalid_proxy))
        .record(named_tag_record(
            LARGE_TAG,
            "UI.Style._Standard.ModularKit.Components.general-button-large",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let fonts = fonts()?;
    let localized = Cell::new(false);
    let requests = RefCell::new(Vec::new());
    let localise = |key: &str| {
        if key == "fixture_large_label" {
            localized.set(true);
            Some("A".to_owned())
        } else {
            None
        }
    };
    let vector = |path: &str| {
        requests.borrow_mut().push(path.to_owned());
        Ok(WRAP_SVG.as_bytes().to_vec())
    };
    let composed = ScreenComposer::new(&forge)
        .with_images(ImageSources {
            texture: &texture,
            vector: &vector,
        })
        .with_texts(TextSources {
            fonts: &fonts,
            localise: Some(&localise),
        })
        .compose(&guid_text(SCREEN), None)?;
    Ok(Rendered {
        composed,
        vectors: requests.into_inner(),
        localized: localized.get(),
    })
}

#[test]
fn large_button_draws_the_original_root_shapes_and_respects_label_visibility() -> TestResult {
    for show in [true, false] {
        let actual = compose(show, None, false)?;
        assert_eq!(
            actual.vectors,
            [FRAME_SVG, EDGE_SVG],
            "both original frame children follow the root"
        );
        assert_eq!(
            actual.composed.report().images_drawn,
            2,
            "the two SVG children draw: {:?}",
            actual.composed.report().unsupported
        );
        assert_eq!(
            actual.localized, show,
            "only a visible label uses the holder's localization"
        );
        assert_eq!(
            actual.composed.report().texts_drawn,
            usize::from(show),
            "the Large class styles the label"
        );
    }
    Ok(())
}

#[test]
fn an_invalid_explicit_large_canvas_or_proxy_cannot_select_an_implicit_root() -> TestResult {
    for (target, invalid_proxy, reason) in [
        (
            Some(SCREEN + 100),
            false,
            "the explicit button canvas is outside the standard template route",
        ),
        (
            None,
            true,
            "the Large button template has no valid original scene root",
        ),
    ] {
        let actual = compose(true, target, invalid_proxy)?;
        assert!(
            actual.vectors.is_empty(),
            "no vector use comes from a replacement root"
        );
        assert_eq!(
            actual.composed.report().canvases,
            1,
            "only the holder canvas enters"
        );
        assert!(
            actual
                .composed
                .report()
                .unsupported
                .iter()
                .any(|entry| entry.reason == reason),
            "the source refuses this specific route: {reason}"
        );
    }
    Ok(())
}
