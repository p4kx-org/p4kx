// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::DataForge;
use p4kx_testkit::{
    DataForgeBuilder, TestCanvas, TestColour, TestDim, TestStyle, TestValue, TestWidget,
    entity_schema, screens_schema,
};

use super::{Check, VisitKind, palette_colour, slot_names};
use crate::ScreenComposer;
use crate::colour::{ColorSpec, Palette};

const ROOT: u128 = 0x5a00_0000_0000_4000_8000_0000_0000_0010;
const PAGE: u128 = ROOT + 1;
const PALETTE: u128 = ROOT + 2;

#[test]
fn source_trace_retains_the_actual_nested_parent_and_painted_palette_slot() -> Check {
    let bytes = database();
    let forge = DataForge::open(&bytes).map_err(|_| "cannot open synthetic DataForge")?;
    let root = forge.record(0).ok_or("missing synthetic root canvas")?;
    let nested = forge.record(1).ok_or("missing synthetic page canvas")?;
    let mut composer = ScreenComposer::new(&forge);
    composer.power_scope = Some(nested.guid());
    let composition = composer
        .compose(&root.guid().to_string(), None)
        .map_err(|_| "cannot compose synthetic MFD")?;
    let visits = composition
        .power_trace
        .ok_or("no synthetic source trace")?
        .visits;
    let [holder, parent, label] = visits.as_slice() else {
        return Err("expected the canvas holder, parent and child");
    };
    assert!(
        !holder.in_scope,
        "the selecting widget is outside the entered canvas"
    );
    assert!(
        parent.in_scope && label.in_scope,
        "both nested widgets are inside the entered canvas"
    );
    assert_eq!(
        parent.parent,
        Some(0),
        "the visited parent is below the holder"
    );
    assert_eq!(
        label.parent,
        Some(1),
        "the visited child is below that parent"
    );
    assert_eq!(
        parent.input.record_index,
        nested.index(),
        "the parent belongs to the physical page"
    );
    let VisitKind::Parent {
        slot,
        drawn: Some(drawn),
        palette: Some(palette),
        ..
    } = &parent.kind
    else {
        return Err("the original palette slot did not paint the selected parent");
    };
    assert_eq!(
        palette.record_index, 2,
        "the original style supplied the winning colour"
    );
    assert_eq!(palette.slot, 1, "the original slot within colorStyles");
    assert_eq!(
        *drawn,
        palette_colour(&forge, &slot_names(&forge), slot, *palette)?,
        "the actual painter emitted the original palette colour",
    );
    Ok(())
}

#[test]
fn a_duplicate_name_keeps_the_ordinal_of_its_last_resolved_colour() -> Check {
    let bytes = database();
    let forge = DataForge::open(&bytes).map_err(|_| "cannot open synthetic DataForge")?;
    let style = forge.record(2).ok_or("missing synthetic style Record")?;
    let original = style
        .tree()
        .map_err(|_| "cannot read synthetic style Record")?;
    let slots = ["Base", "Accent1", "Accent1"].map(str::to_owned);
    let palette = Palette::from_style(&forge, &original, &slots);
    let name = "Accent1";
    let site = palette
        .sources
        .get(name)
        .copied()
        .ok_or("missing winning palette source")?;
    assert_eq!(
        site.record_index,
        style.index(),
        "the physical style Record"
    );
    assert_eq!(
        site.slot, 2,
        "the last resolved occurrence supplies the colour"
    );
    let spec = ColorSpec::Slot {
        name: name.to_owned(),
        alpha: 1.0,
    };
    let colour = palette
        .resolve(&spec, 1.0)
        .map_err(|_| "cannot resolve duplicate palette name")?;
    assert_eq!(
        [colour.red, colour.green, colour.blue, colour.alpha],
        [22, 34, 45, 255],
        "the duplicate keeps the last resolved colour",
    );
    assert_eq!(
        colour,
        palette_colour(&forge, &slots, name, site)?,
        "the original ordinal supplies exactly the resolved colour",
    );
    Ok(())
}

fn database() -> Vec<u8> {
    let holder = TestWidget::new("BuildingBlocks_WidgetCanvas", "holder")
        .with("canvas", TestValue::Reference(PAGE))
        .size(TestDim::Fixed(8.0), TestDim::Fixed(8.0));
    let root = TestCanvas::new(ROOT, "trace_root", 8.0, 8.0).widget(holder);
    let parent = TestWidget::display("")
        .size(TestDim::Fixed(8.0), TestDim::Fixed(8.0))
        .background(TestColour::Slot("Accent1", 1.0));
    let label = TestWidget::display("label")
        .parent("")
        .size(TestDim::Fixed(2.0), TestDim::Fixed(2.0));
    let nested = TestCanvas::new(PAGE, "trace_page", 8.0, 8.0)
        .shared_style(PALETTE)
        .widget(parent)
        .widget(label);
    let palette = TestStyle::new(
        PALETTE,
        "trace_palette",
        vec![
            TestColour::Solid([0, 0, 0, 255]),
            TestColour::Solid([184, 211, 239, 255]),
            TestColour::Solid([22, 34, 45, 255]),
        ],
    );
    screens_schema(entity_schema(DataForgeBuilder::new()))
        .record(root.record())
        .record(nested.record())
        .record(palette.record())
        .build()
}
