// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::ops::Range;
use std::path::Path;

use p4kx_dataforge::{DataForge, RecordTree};
use p4kx_extraction::Archive;
use p4kx_gfx::{Background, CanvasSize, RasterOptions};
use p4kx_testkit::real_archive_path;

use super::frame::VectorReads;
use super::{
    CANVAS_TYPE, Check, DISPLAY, TEMPLATE_LABEL, Trace, Visit, WIDTH, canvas, changed_pixels,
    compose_canvas, original_proxy, same_widget, scene_input, tree, widget_source,
    with_installed_forge, with_screen_texts,
};
use crate::button_component::GENERAL;
use crate::canvas::Canvas;
use crate::elements::GENERAL_CANVAS;
use crate::reader::{as_struct, items, reference};
use crate::{TextSources, WidgetInput};

#[path = "button_general_owner.rs"]
mod selection;

use self::selection::Owner;
use super::super::{Observation, TextPaintFailure};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K to installed LIVE and pass --ignored"]
fn a_live_general_button_draws_its_original_primary_frame_and_label() -> Check {
    let path = real_archive_path().map_err(|_| "configure the installed LIVE Archive")?;
    check_live(&path)?;
    let (build, result) = with_installed_forge(|archive, forge| {
        report_census(forge, &mut io::stdout().lock())?;
        with_screen_texts(archive, forge, |texts| measure(archive, forge, texts))
    })?;
    writeln!(
        io::stdout(),
        "LIVE build {} change {}: General button on an MFD canvas={}, visible frame pixels={}, original label pixels={}",
        build.version(),
        build.change_number(),
        result.on_mfd,
        result.frame_pixels,
        result.label_pixels,
    )
    .map_err(|_| "cannot report the General witness")
}

fn check_live(path: &Path) -> Check {
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("Data.p4k"))
        || !path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("LIVE"))
    {
        return Err("the General witness requires the installed LIVE Data.p4k");
    }
    Ok(())
}

fn report_census(forge: &DataForge<'_>, out: &mut impl Write) -> Check {
    let mut counts = BTreeMap::new();
    for definition in forge.structs() {
        let name = definition
            .name()
            .map_err(|_| "unreadable original struct name")?;
        if name.starts_with(GENERAL) {
            let count = counts.entry(name).or_insert(0_u64);
            *count += u64::from(definition.instance_count());
        }
    }
    for (name, count) in counts {
        writeln!(out, "source struct {name}: {count} DataForge instances")
            .map_err(|_| "cannot report the General family census")?;
    }
    Ok(())
}

struct Witness {
    on_mfd: bool,
    frame_pixels: usize,
    label_pixels: usize,
}

fn measure(archive: &Archive, forge: &DataForge<'_>, texts: TextSources<'_>) -> Check<Witness> {
    let localise = texts
        .localise
        .ok_or("no English lookup for the General witness")?;
    let owner = selection::select_owner(forge, localise)?;
    let template = canvas(forge, GENERAL_CANVAS)?;
    let proxy = original_proxy(&template)?;
    if widget_source(&template, proxy)?.type_name() != DISPLAY {
        return Err("the General proxy is not an original DisplayWidget");
    }
    let label = label_source(&template, proxy)?;
    verify_rsi_style(forge, &template)?;
    let reads = VectorReads::default();
    let selector = owner.canvas.guid().to_string();
    let composed = compose_canvas(
        archive,
        forge,
        (&selector, owner.canvas.guid(), &[]),
        &reads,
        texts,
    )?;
    let trace = composed
        .power_trace
        .as_ref()
        .ok_or("no exact General source trace")?;
    let (holder, _) = visit(trace, owner.input, None, || holder_gate(trace, &owner))?;
    let (mounted, proxy_visit) = visit(
        trace,
        proxy,
        Some(holder),
        || "the original General template proxy was not visited",
    )?;
    let (_, label_visit) = visit(
        trace,
        label,
        Some(mounted),
        || "the original General label was not visited",
    )?;
    let (glyphs, actual_key) = composed
        .text_items_in(&label_visit.own_items)
        .ok_or_else(|| {
            label_visit.text_failure.as_ref().map_or(
                "the General label visit has no tracked text paint",
                TextPaintFailure::description,
            )
        })?;
    match actual_key {
        Some(key) if key == owner.label_key => {
            footprints(&composed, owner.on_mfd, proxy_visit, glyphs.clone())
        }
        Some(_) => Err("the General label painted with a different English lookup key"),
        None => Err("the General label painted without a successful English lookup"),
    }
}

fn label_source(template: &RecordTree<'_>, proxy: WidgetInput) -> Check<WidgetInput> {
    let mut labels = items(template.root(), "scene")
        .iter()
        .enumerate()
        .filter_map(|(slot, value)| {
            let widget = as_struct(value)?;
            let parent = reference(widget, "parent")?;
            (widget.type_name() == TEMPLATE_LABEL
                && parent.record_index() == proxy.record_index
                && same_widget(parent, template.guid(), proxy))
            .then(|| scene_input(template, slot))
        });
    let first = labels
        .next()
        .ok_or("the General proxy has no original label part")?;
    if labels.next().is_some() {
        return Err("the General proxy has several original label parts");
    }
    Ok(first)
}

fn verify_rsi_style(forge: &DataForge<'_>, template: &RecordTree<'_>) -> Check {
    let authored = items(template.root(), "brandStyles")
        .iter()
        .filter_map(as_struct)
        .find(|entry| {
            reference(entry, "brandIdentifier")
                .is_some_and(|brand| brand.record_path().to_ascii_lowercase().contains("s_rsi_"))
        })
        .and_then(|entry| reference(entry, "sharedStyles"))
        .ok_or("the first original RSI brand has no shared Style")?;
    let shared = tree(forge, authored.record_index())?;
    if authored.location().is_some()
        || shared.guid() != authored.guid()
        || shared.type_name() != "BuildingBlocks_Style"
    {
        return Err("the original RSI shared Style does not name its actual Record");
    }
    let parsed = Canvas::read(template);
    let selected = parsed
        .pick_brand(Some("rsi"))
        .and_then(|brand| brand.styles().nth(1))
        .ok_or("the composer did not select the original shared Style")?;
    if selected.guid != shared.guid() || selected.record_path != authored.record_path() {
        return Err("the composer's RSI shared Style disagrees with its original Reference");
    }
    Ok(())
}

fn visit(
    trace: &Trace,
    input: WidgetInput,
    parent: Option<usize>,
    missing: impl FnOnce() -> &'static str,
) -> Check<(usize, &Visit)> {
    let mut selected = trace.visits.iter().enumerate().filter(|(_, visit)| {
        visit.input == input && parent.is_none_or(|parent| visit.parent == Some(parent))
    });
    let first = selected.next().ok_or_else(missing)?;
    if !first.1.in_scope || selected.next().is_some() {
        return Err("the General button part does not have one exact mounted occurrence");
    }
    Ok(first)
}

fn holder_gate(trace: &Trace, owner: &Owner<'_>) -> &'static str {
    let chain = owner
        .ancestors
        .iter()
        .rev()
        .copied()
        .chain(std::iter::once(owner.input));
    for input in chain {
        match trace.visibility.root_observation(input) {
            Some(Observation::LaidOut) => {}
            Some(Observation::NotInstantiated) => {
                return "a General owner or ancestor was not instantiated";
            }
            Some(Observation::BoundInactive) => {
                return "a General owner or ancestor has an inactive IsActive binding";
            }
            Some(Observation::StyledInactive) => {
                return "a General owner or ancestor has an inactive IsActive style";
            }
            Some(Observation::AuthoredInactive) => {
                return "a General owner or ancestor is authored inactive";
            }
            None if input == owner.input => {
                return "the original General holder never reached a paint or activity trace";
            }
            None => return "an original General ancestor never reached a paint or activity trace",
        }
    }
    "the original General holder was laid out without a visit"
}

fn footprints(
    composed: &crate::Composed,
    on_mfd: bool,
    frame: &Visit,
    glyphs: Range<usize>,
) -> Check<Witness> {
    let raster = RasterOptions {
        size: CanvasSize::Width(WIDTH),
        background: Background::Transparent,
    };
    let original = composed
        .rasterize(&raster)
        .map_err(|_| "cannot rasterize the original General button")?
        .image;
    let frame = changed_pixels(
        &original,
        &composed.rasterize_without_items(&raster, frame.own_items.clone())?,
    )?;
    let label = changed_pixels(
        &original,
        &composed.rasterize_without_items(&raster, glyphs)?,
    )?;
    Ok(Witness {
        on_mfd,
        frame_pixels: frame.count,
        label_pixels: label.count,
    })
}
