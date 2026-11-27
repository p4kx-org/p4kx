// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::DataForge;
use p4kx_extraction::{ConversionError, UnknownEntries};
use p4kx_gfx::{Background, CanvasSize, RasterOptions};

use super::super::VisitKind;
use super::frame::{VectorRead, VectorReads};
use super::{Check, Visit, changed_pixels};
use crate::{ScreenComposer, WidgetInput, WidgetSection};

fn clipped_fixture() -> Vec<u8> {
    use p4kx_testkit::{
        DataForgeBuilder, TestCanvas, TestColour, TestDim, TestWidget, entity_schema,
        screens_schema,
    };

    const ROOT: u128 = 0x5a00_0000_0000_4000_8000_0000_0000_0020;
    let fixed = TestDim::Fixed;
    let strip = |name, red| {
        let colour = if red {
            [255, 0, 0, 255]
        } else {
            [0, 255, 0, 255]
        };
        TestWidget::display(name)
            .size(fixed(12.0), fixed(3.0))
            .background(TestColour::Solid(colour))
    };
    let canvas = TestCanvas::new(ROOT, "clipped_trace", 12.0, 8.0)
        .raw()
        .widget(
            TestWidget::display("clip")
                .size(fixed(8.0), fixed(8.0))
                .overflow("Clip"),
        )
        .widget(TestWidget::display("parent").parent("clip"))
        .widget(strip("removed", true).parent("parent"))
        .widget(strip("kept", false).parent("clip").offset(0.0, 4.0));
    screens_schema(entity_schema(DataForgeBuilder::new()))
        .record(canvas.record())
        .build()
}

#[test]
fn the_traced_subtree_can_be_removed_without_affecting_a_siblings_clip() -> Check {
    let bytes = clipped_fixture();
    let forge = DataForge::open(&bytes).map_err(|_| "cannot read the synthetic canvas")?;
    let root = forge.record(0).ok_or("no synthetic canvas")?.guid();
    let mut composer = ScreenComposer::new(&forge);
    composer.power_scope = Some(root);
    let rendered = composer
        .compose(&root.to_string(), None)
        .map_err(|_| "cannot compose synthetic canvas")?;
    let trace = rendered.power_trace.as_ref().ok_or("no synthetic trace")?;
    let parent = trace
        .visits
        .iter()
        .find(|visit| visit.input.slot == 1)
        .ok_or("missing traced parent")?;
    assert!(
        parent.own_items.is_empty(),
        "the parent itself paints nothing"
    );
    let raster = RasterOptions {
        size: CanvasSize::Stage,
        background: Background::Transparent,
    };
    let baseline = rendered
        .rasterize(&raster)
        .map_err(|_| "cannot rasterize the synthetic canvas")?
        .image;
    let after =
        rendered.rasterize_without_items(&raster, parent.own_items.start..parent.last_item)?;
    let difference = changed_pixels(&baseline, &after)?;
    assert_eq!(
        difference.count, 24,
        "only the clipped red strip is removed"
    );
    let row = after
        .pixels()
        .as_chunks::<4>()
        .0
        .get(48..60)
        .ok_or("missing synthetic raster row")?;
    assert_eq!(
        row[6],
        [0, 255, 0, 255],
        "the visible part of the surviving strip remains"
    );
    assert_eq!(
        row[10],
        [0, 0, 0, 0],
        "the surviving strip retains its clip"
    );
    Ok(())
}

const STYLED_SVG: &str = "ui/test/frame.svg";

fn proxy_visit_with_box() -> Visit {
    Visit {
        input: WidgetInput {
            record_index: 0,
            section: WidgetSection::Scene,
            slot: 0,
        },
        parent: None,
        in_scope: true,
        kind: VisitKind::Other,
        styled_vector: Some(STYLED_SVG.to_owned()),
        vector_requests: vec![STYLED_SVG.to_owned()],
        vectors: Some(Vec::new()),
        own_items: 2..4,
        text_failure: None,
        last_item: 4,
    }
}

#[test]
fn only_an_exact_archive_miss_allows_the_proxys_box_to_witness_its_frame() -> Check {
    let reads = VectorReads::default();
    let mut visit = proxy_visit_with_box();
    assert!(
        reads.frame(&visit).is_err(),
        "a draw request without an observed Archive result proves no absence"
    );
    let miss = Err(UnknownEntries(vec![STYLED_SVG.to_owned()]).into());
    reads.record(&STYLED_SVG.to_ascii_uppercase(), &miss);
    let frame = reads.frame(&visit)?;
    assert_eq!(frame.vector, VectorRead::Absent, "the file was absent");
    assert_eq!(frame.items, 2..4, "both original box items prove the frame");

    visit.vectors = Some(vec![(3, "ui/test/substitute.svg".to_owned())]);
    assert!(
        reads.frame(&visit).is_err(),
        "any emitted SVG invalidates the absent-file witness"
    );
    visit.vectors = Some(Vec::new());
    reads.record(
        STYLED_SVG,
        &Err(ConversionError::NoSuchFile {
            path: STYLED_SVG.into(),
        }),
    );
    assert!(
        reads.frame(&visit).is_err(),
        "errors other than the Archive entry lookup failure cannot prove absence"
    );
    Ok(())
}

#[test]
fn a_present_svg_must_prove_the_exact_painted_item() -> Check {
    let reads = VectorReads::default();
    let mut visit = proxy_visit_with_box();
    reads.record(STYLED_SVG, &Ok(Vec::new()));
    assert!(
        reads.frame(&visit).is_err(),
        "a successful read cannot excuse a failure to paint the vector"
    );
    visit.own_items.end = 5;
    visit.vectors = Some(vec![(4, "ui/test/substitute.svg".to_owned())]);
    assert!(
        reads.frame(&visit).is_err(),
        "a substituted vector cannot prove the styled SVG was painted"
    );
    visit.vectors = Some(vec![(4, STYLED_SVG.to_owned())]);
    let frame = reads.frame(&visit)?;
    assert_eq!(frame.vector, VectorRead::Present, "the file was read");
    assert_eq!(
        frame.items,
        4..5,
        "only the original vector proves the frame"
    );
    Ok(())
}
