// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, Value};
use p4kx_screens::{Composed, ScreenComposer};
use p4kx_testkit::{TestCanvas, TestDim, TestWidget};

use crate::support::{self, GREEN, SCREEN, TestResult, empty_at, green_at};

const CLIPS: u16 = 25;

#[test]
fn every_independent_source_clip_still_clips_after_twenty_four_masks() -> TestResult {
    let mut canvas = TestCanvas::new(SCREEN, "all-clips", 100.0, 50.0);
    let mut names = Vec::new();
    for index in 0..CLIPS {
        let clip = format!("clip-{index}");
        let fill = format!("fill-{index}");
        canvas = canvas.widget(window(&clip, 20.0)).widget(
            TestWidget::display(&fill)
                .parent(&clip)
                .size(TestDim::Fixed(60.0), TestDim::Fixed(10.0))
                .background(GREEN),
        );
        names.extend([clip, fill]);
    }
    let composed = compose_source(&canvas, &names)?;
    let raster = composed.rasterize(&support::stage())?;
    assert_eq!(raster.skipped, [], "all admitted masks and fills render");
    assert!(
        green_at(&raster.image, 10, 5),
        "inside the final original clip"
    );
    assert!(
        empty_at(&raster.image, 30, 5),
        "the final original clip still hides its overflow"
    );
    assert_eq!(composed.scene().masks.len(), usize::from(CLIPS));
    assert!(
        composed.report().is_complete(),
        "all original clipping is represented"
    );
    assert_complete_svg(&composed)?;
    Ok(())
}

#[test]
fn the_twenty_fifth_nested_source_clip_keeps_its_narrower_boundary() -> TestResult {
    let mut canvas = TestCanvas::new(SCREEN, "nested-clips", 100.0, 50.0);
    let mut parent = None;
    let mut names = Vec::new();
    for index in 0..CLIPS {
        let name = format!("clip-{index}");
        let mut widget = window(&name, 50.0 - f32::from(index));
        if let Some(parent) = &parent {
            widget = widget.parent(parent);
        }
        canvas = canvas.widget(widget);
        names.push(name.clone());
        parent = Some(name);
    }
    let parent = parent.ok_or("the final original clip")?;
    canvas = canvas.widget(
        TestWidget::display("fill")
            .parent(parent)
            .size(TestDim::Fixed(100.0), TestDim::Fixed(10.0))
            .background(GREEN),
    );
    names.push("fill".to_owned());
    let composed = compose_source(&canvas, &names)?;
    let raster = composed.rasterize(&support::stage())?;
    assert_eq!(
        raster.skipped,
        [],
        "all required ancestor masks fit the raster budget"
    );
    assert!(green_at(&raster.image, 25, 5), "inside every original clip");
    assert!(
        empty_at(&raster.image, 26, 5),
        "only the final original clip excludes this pixel"
    );
    let masks = &composed.scene().masks;
    assert_eq!(masks.len(), usize::from(CLIPS));
    for (index, mask) in masks.iter().enumerate() {
        assert_eq!(
            mask.parent,
            index.checked_sub(1),
            "the exact original clip chain"
        );
    }
    assert_complete_svg(&composed)?;
    assert!(composed.report().is_complete());
    Ok(())
}

fn window(name: &str, width: f32) -> TestWidget {
    TestWidget::display(name)
        .size(TestDim::Fixed(width), TestDim::Fixed(40.0))
        .overflow("Clip")
}

fn compose_source(
    canvas: &TestCanvas,
    names: &[String],
) -> Result<Composed, Box<dyn std::error::Error>> {
    let bytes = support::database([canvas.record()]);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("the original canvas record")?;
    let tree = record.tree()?;
    assert_eq!(tree.error_count(), 0, "every original widget is readable");
    let Some(Value::Array(widgets)) = tree.root().field("scene") else {
        return Err("the original widget array".into());
    };
    assert_eq!(
        widgets.len(),
        names.len(),
        "all original widget slots exist"
    );
    let mut clips = 0;
    for (widget, name) in widgets.iter().zip(names) {
        let Value::Struct(widget) = widget else {
            return Err("a readable original widget".into());
        };
        assert!(
            matches!(widget.field("name"), Some(Value::String(actual)) if actual == name),
            "the exact original widget order and names"
        );
        if name.starts_with("clip-") {
            let Some(Value::Struct(overflow)) = widget.field("overflow") else {
                return Err("the original clipping declaration".into());
            };
            assert!(
                matches!(overflow.field("overflow"), Some(Value::String("Clip"))),
                "the original declaration requests clipping"
            );
            clips += 1;
        }
    }
    assert_eq!(
        clips,
        usize::from(CLIPS),
        "every original clipping declaration is present"
    );
    Ok(ScreenComposer::new(&forge).compose(&support::guid_text(SCREEN), None)?)
}

fn assert_complete_svg(composed: &Composed) -> TestResult {
    let mut output = Vec::new();
    let report = composed.write_svg(&support::stage(), None, &mut output)?;
    assert_eq!(
        report.skipped,
        [],
        "SVG preserves every admitted clipped item"
    );
    let text = String::from_utf8(output)?;
    let document = roxmltree::Document::parse(&text)?;
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("mask"))
            .count(),
        usize::from(CLIPS),
        "every original clip has a written SVG mask"
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("mask") == Some("url(#m24)")),
        "SVG content uses the final original clip"
    );
    Ok(())
}
