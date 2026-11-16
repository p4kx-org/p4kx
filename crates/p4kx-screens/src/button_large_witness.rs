// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::fs;
use std::io::{self, Write};
use std::ops::Range;

use p4kx_dataforge::{DataForge, RecordTree, Value};
use p4kx_extraction::Archive;
use p4kx_gfx::{Background, CanvasSize, Localise, RasterOptions};
use p4kx_testkit::real_archive_path;

use super::frame::VectorReads;
use super::general::{check_live, label_source, selection, verify_rsi_style, visit};
use super::{
    CANVAS_TYPE, Check, DISPLAY, WIDTH, canvas, changed_pixels, compose_canvas, same_widget,
    scene_input, with_installed_forge, with_screen_texts,
};
use crate::elements::LARGE_CANVAS;
use crate::picture::CUSTOM_SHAPE;
use crate::reader::{as_struct, items, reference};
use crate::{Composed, TextSources, WidgetInput};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_live_large_button_draws_its_original_frame_and_label() -> Check {
    let path = real_archive_path().map_err(|_| "configure the installed LIVE Archive")?;
    check_live(&path)?;
    let path = env::var_os("P4KX_PRIVATE_GENERAL_LARGE_OWNER")
        .ok_or("configure the private file of exact Large owner Canvas paths")?;
    let selectors = fs::read_to_string(path).map_err(|_| "cannot read the private Large owners")?;
    let (build, (frame_pixels, label_pixels)) = with_installed_forge(|archive, forge| {
        with_screen_texts(archive, forge, |texts| {
            let localise = texts.localise.ok_or("missing English lookup")?;
            let (owner, holder, key) = select_owner(forge, &selectors, localise)?;
            measure(archive, forge, &owner, holder, &key, texts)
        })
    })?;
    writeln!(
        io::stdout(),
        "LIVE build {} change {}: Large button, original frame pixels={}, original label pixels={}",
        build.version(),
        build.change_number(),
        frame_pixels,
        label_pixels,
    )
    .map_err(|_| "cannot report the Large witness")
}

fn select_owner<'f>(
    forge: &'f DataForge<'_>,
    selectors: &str,
    localise: Localise<'_>,
) -> Check<(RecordTree<'f>, WidgetInput, String)> {
    for selector in selectors
        .lines()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        let owner = owner_canvas(forge, selector)?;
        if let Ok((holder, key)) = selection::large_owner(&owner, localise) {
            return Ok((owner, holder, key));
        }
    }
    Err("no nominated Large owner has an original label and statically active ancestry")
}

fn owner_canvas<'f>(forge: &'f DataForge<'_>, selector: &str) -> Check<RecordTree<'f>> {
    let mut records = forge.records_at_path(selector);
    let record = records
        .next()
        .ok_or("the exact private Large owner Canvas is absent")?;
    if records.next().is_some() || record.type_name().ok() != Some(CANVAS_TYPE) {
        return Err("the private Large selector does not name one original Canvas");
    }
    record
        .tree()
        .map_err(|_| "cannot read the original Large owner Canvas")
}

fn original_root(template: &RecordTree<'_>) -> Check<WidgetInput> {
    if !matches!(
        template.root().field("proxyRootNode"),
        None | Some(Value::Null)
    ) {
        return Err("the Large source template has an explicit proxy");
    }
    let mut roots = items(template.root(), "scene")
        .iter()
        .enumerate()
        .filter_map(|(slot, value)| {
            let widget = as_struct(value)?;
            matches!(widget.field("parent"), None | Some(Value::Null)).then_some((slot, widget))
        });
    let (slot, root) = roots
        .next()
        .ok_or("the Large source template has no original root")?;
    if roots.next().is_some() || root.type_name() != DISPLAY {
        return Err("the Large source template has no unique original DisplayWidget root");
    }
    Ok(scene_input(template, slot))
}

fn original_shapes(template: &RecordTree<'_>, root: WidgetInput) -> Check<[WidgetInput; 2]> {
    let mut shapes = items(template.root(), "scene")
        .iter()
        .enumerate()
        .filter_map(|(slot, value)| {
            let widget = as_struct(value)?;
            let parent = reference(widget, "parent")?;
            (widget.type_name() == CUSTOM_SHAPE
                && parent.record_index() == root.record_index
                && same_widget(parent, template.guid(), root))
            .then(|| scene_input(template, slot))
        });
    let first = shapes
        .next()
        .ok_or("the Large root has no original frame shape")?;
    let second = shapes
        .next()
        .ok_or("the Large root has no second original frame shape")?;
    if shapes.next().is_some() {
        return Err("the Large root has more than two original frame shapes");
    }
    Ok([first, second])
}

fn measure(
    archive: &Archive,
    forge: &DataForge<'_>,
    owner: &RecordTree<'_>,
    holder: WidgetInput,
    label_key: &str,
    texts: TextSources<'_>,
) -> Check<(usize, usize)> {
    let template = canvas(forge, LARGE_CANVAS)?;
    let root = original_root(&template)?;
    let label = label_source(&template, root)?;
    let [first, second] = original_shapes(&template, root)?;
    verify_rsi_style(forge, &template)?;

    let composed = compose_canvas(
        archive,
        forge,
        (&owner.guid().to_string(), owner.guid(), &[]),
        &VectorReads::default(),
        texts,
    )?;
    let trace = composed
        .power_trace
        .as_ref()
        .ok_or("no Large source trace")?;
    let (holder, _) = visit(
        trace,
        holder,
        None,
        || "the original Large holder was not visited",
    )?;
    let (mounted, _) = visit(
        trace,
        root,
        Some(holder),
        || "the original Large root was not mounted",
    )?;
    let (_, label_visit) = visit(
        trace,
        label,
        Some(mounted),
        || "the original Large label was not visited",
    )?;
    let (_, first) = visit(
        trace,
        first,
        Some(mounted),
        || "the first original Large frame shape was not visited",
    )?;
    let (_, second) = visit(
        trace,
        second,
        Some(mounted),
        || "the second original Large frame shape was not visited",
    )?;
    let (glyphs, key) = composed
        .text_items_in(&label_visit.own_items)
        .ok_or("the original Large label submitted no tracked glyphs")?;
    if key != Some(label_key) {
        return Err("the Large label used a different English lookup key");
    }
    footprints(
        &composed,
        [&first.own_items, &second.own_items],
        glyphs.clone(),
    )
}

fn footprints(
    composed: &Composed,
    frame: [&Range<usize>; 2],
    glyphs: Range<usize>,
) -> Check<(usize, usize)> {
    let raster = RasterOptions {
        size: CanvasSize::Width(WIDTH),
        background: Background::Transparent,
    };
    let original = composed
        .rasterize(&raster)
        .map_err(|_| "cannot rasterize the original Large button")?
        .image;
    let label = changed_pixels(
        &original,
        &composed.rasterize_without_items(&raster, glyphs)?,
    )?;
    let mut frame_pixels = 0;
    for items in frame {
        let removed = composed.rasterize_without_items(&raster, items.clone())?;
        if original != removed {
            let shape = changed_pixels(&original, &removed)?;
            frame_pixels = frame_pixels.max(shape.count);
        }
    }
    if frame_pixels == 0 {
        return Err("no original Large frame shape contributes visible paint");
    }
    Ok((frame_pixels, label.count))
}
