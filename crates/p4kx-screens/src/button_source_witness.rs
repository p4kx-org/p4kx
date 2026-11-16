// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use p4kx_dataforge::{DataForge, Guid, RecordTree};
use p4kx_extraction::{Archive, ArchiveImages, Language, ScreenTexts, decode_line_breaks};
use p4kx_gfx::{Background, CanvasSize, Localise, RasterOptions, RgbaImage};

use super::{
    Check, DISPLAY, MFD, Trace, Visit, power_variables, same_widget, tree, widget_source,
    with_installed_forge,
};
use crate::button_component::SECONDARY;
use crate::canvas::{CANVAS_TYPE, find_canvas};
use crate::elements::SECONDARY_CANVAS;
use crate::reader::{as_struct, child, flag, items, reference, text};
use crate::{
    Composed, ImageSources, MAX_SVG_BYTES, ScreenComposer, ScreenFontFile, TextSources,
    WidgetInput, WidgetSection,
};

#[path = "button_source_frame.rs"]
mod frame;
#[path = "button_source_witness_tests.rs"]
mod tests;

use self::frame::{VectorRead, VectorReads};

const WIDTH: u32 = 800;
const TEMPLATE_LABEL: &str = "BuildingBlocks_WidgetTextField";
const TEMPLATE_ICON: &str = "BuildingBlocks_WidgetIcon";
const WIDGET_CANVAS: &str = "BuildingBlocks_WidgetCanvas";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_rsi_footer_draws_both_original_secondary_button_frames() -> Check {
    let (build, witness) = with_installed_forge(measure)?;
    let [first, second] = witness.footprints;
    writeln!(
        io::stdout(),
        "RSI footer build {} change {}: two original Secondary buttons, {} and {} visible pixels, SVG archive reads {:?}",
        build.version(),
        build.change_number(),
        first.count,
        second.count,
        witness.vectors,
    )
    .map_err(|_| "cannot report the witness")
}

struct FooterWitness {
    footprints: [Footprint; 2],
    vectors: [VectorRead; 2],
}

fn measure(archive: &Archive, forge: &DataForge<'_>) -> Check<FooterWitness> {
    let wrapper = canvas(forge, MFD)?;
    let content = nested_canvas(forge, &wrapper, 1)?;
    let footer = nested_canvas(forge, &content, 8)?;
    let buttons = original_buttons(&footer)?;
    let template = canvas(forge, SECONDARY_CANVAS)?;
    let proxy = original_proxy(&template)?;
    let proxy_source = widget_source(&template, proxy)?;
    if proxy_source.type_name() != DISPLAY {
        return Err("the original template proxy is not a DisplayWidget");
    }
    if child(proxy_source, "svgFill").and_then(|fill| flag(fill, "renderShape")) != Some(true) {
        return Err("the original template proxy has no enabled SVG fill");
    }
    let reads = VectorReads::default();
    let composed = compose(archive, forge, footer.guid(), &reads)?;
    let trace = composed.power_trace.as_ref().ok_or("no footer trace")?;
    let first = reads.frame(proxy_visit(trace, &footer, buttons[0], &template, proxy)?)?;
    let second = reads.frame(proxy_visit(trace, &footer, buttons[1], &template, proxy)?)?;
    Ok(FooterWitness {
        vectors: [first.vector, second.vector],
        footprints: check_visible_pair(&composed, [first.items, second.items])?,
    })
}

fn canvas<'f>(forge: &'f DataForge<'_>, selector: &str) -> Check<RecordTree<'f>> {
    let guid = find_canvas(forge, selector).map_err(|_| "no unique witness canvas")?;
    forge
        .record_by_guid(guid)
        .ok_or("the selected canvas has no physical Record")?
        .tree()
        .map_err(|_| "cannot read the selected canvas Record")
}

fn nested_canvas<'f>(
    forge: &'f DataForge<'_>,
    owner: &RecordTree<'_>,
    slot: usize,
) -> Check<RecordTree<'f>> {
    let widget = widget_source(owner, scene_input(owner, slot))?;
    let link = reference(widget, "canvas").ok_or("the original canvas widget has no target")?;
    let target = tree(forge, link.record_index())?;
    if widget.type_name() != WIDGET_CANVAS
        || link.location().is_some()
        || target.type_name() != CANVAS_TYPE
        || target.guid() != link.guid()
    {
        return Err("the authored MFD canvas route has changed");
    }
    Ok(target)
}

const fn scene_input(owner: &RecordTree<'_>, slot: usize) -> WidgetInput {
    WidgetInput {
        record_index: owner.record_index(),
        section: WidgetSection::Scene,
        slot,
    }
}

fn original_buttons(footer: &RecordTree<'_>) -> Check<[WidgetInput; 2]> {
    let mut buttons = Vec::new();
    for (slot, widget) in items(footer.root(), "scene").iter().enumerate() {
        let Some(widget) = as_struct(widget).filter(|widget| widget.type_name() == SECONDARY)
        else {
            continue;
        };
        let hidden =
            |field| child(widget, field).and_then(|part| flag(part, "show")) == Some(false);
        if text(widget, "fillStyle") != Some("Filled")
            || text(widget, "iconPosition") != Some("Left")
            || !hidden("labelProperties")
            || !hidden("iconProperties")
        {
            return Err("the footer button's authored default signature changed");
        }
        buttons.push(scene_input(footer, slot));
    }
    buttons
        .try_into()
        .map_err(|_| "the footer has a different Secondary button count")
}

fn original_proxy(template: &RecordTree<'_>) -> Check<WidgetInput> {
    let link = reference(template.root(), "proxyRootNode")
        .ok_or("the template has no proxy declaration")?;
    items(template.root(), "scene")
        .iter()
        .enumerate()
        .filter_map(|(slot, value)| {
            as_struct(value).map(|widget| (scene_input(template, slot), widget))
        })
        .find(|(input, widget)| {
            same_widget(link, template.guid(), *input) && reference(widget, "parent").is_none()
        })
        .map(|(input, _)| input)
        .filter(|input| input.record_index == link.record_index())
        .ok_or("the proxy does not name an original template scene root")
}

fn compose(
    archive: &Archive,
    forge: &DataForge<'_>,
    footer: Guid,
    reads: &VectorReads,
) -> Check<Composed> {
    let vars = power_variables()?;
    let font = ScreenFontFile::read(forge);
    let text = ScreenTexts::load_observed(archive, &font.path, Some(Language::English));
    let fonts = text
        .fonts()
        .ok_or("cannot load the selected MFD font movie")?;
    let ini = text
        .parsed(&mut Vec::new())
        .ok_or("cannot read the selected MFD language")?;
    let lookup = |key: &str| {
        ini.resolve(key)
            .map(|value| decode_line_breaks(value).into_owned())
    };
    let images = ArchiveImages::new(archive);
    let texture = |path: &str, side: u32, pixels: u64| {
        images
            .texture(path, side, pixels)
            .map_err(|error| error.to_string())
    };
    let vector = |path: &str| {
        let result = images.vector(path, MAX_SVG_BYTES as u64);
        reads.record(path, &result);
        result.map_err(|error| error.to_string())
    };
    let mut composer = ScreenComposer::new(forge)
        .with_texts(TextSources {
            fonts,
            localise: Some(&lookup as Localise<'_>),
        })
        .with_images(ImageSources {
            texture: &texture,
            vector: &vector,
        })
        .with_variables(&vars)
        .observing_inputs();
    composer.power_scope = Some(footer);
    let made = composer
        .compose(MFD, Some("rsi"))
        .map_err(|_| "cannot compose the original MFD")?;
    if made.report().truncated || made.report().unknown_variables().next().is_some() {
        return Err("the MFD did not completely evaluate the selected scenario");
    }
    Ok(made)
}

fn proxy_visit<'trace>(
    trace: &'trace Trace,
    footer: &RecordTree<'_>,
    owner: WidgetInput,
    template: &RecordTree<'_>,
    proxy: WidgetInput,
) -> Check<&'trace Visit> {
    let mut holders = trace
        .visits
        .iter()
        .enumerate()
        .filter(|(_, visit)| visit.input == owner);
    let (holder, visit) = holders
        .next()
        .ok_or("the original footer button was not visited")?;
    if !visit.in_scope || holders.next().is_some() {
        return Err("the footer button did not have one owned visit");
    }
    let mut mounted = None;
    for (index, visit) in trace
        .visits
        .iter()
        .enumerate()
        .filter(|(_, visit)| visit.parent == Some(holder))
    {
        if visit.input == proxy {
            if mounted.replace((index, visit)).is_some() {
                return Err("the template proxy was mounted twice below one footer button");
            }
        } else if visit.input.record_index != owner.record_index
            || !reference(widget_source(footer, visit.input)?, "parent").is_some_and(|link| {
                link.record_index() == owner.record_index && same_widget(link, footer.guid(), owner)
            })
        {
            return Err("a non-proxy child does not belong to the original footer button");
        }
    }
    let (root, visit) = mounted.ok_or("the template proxy was not mounted")?;
    for child in trace
        .visits
        .iter()
        .filter(|child| child.parent == Some(root))
    {
        if child.input.record_index != proxy.record_index {
            return Err("a template part has the wrong physical Record");
        }
        let part = widget_source(template, child.input)?;
        if !reference(part, "parent")
            .is_some_and(|parent| same_widget(parent, template.guid(), proxy))
        {
            return Err("the painted part does not name its actual proxy parent");
        }
        if matches!(part.type_name(), TEMPLATE_LABEL | TEMPLATE_ICON) {
            return Err("the authored hidden part was visited");
        }
    }
    Ok(visit)
}

#[derive(Debug, Clone, Copy)]
struct Footprint {
    x_min: usize,
    y_min: usize,
    x_max: usize,
    y_max: usize,
    count: usize,
}

fn changed_pixels(reference: &RgbaImage, removed: &RgbaImage) -> Check<Footprint> {
    if reference.width() != removed.width()
        || reference.height() != removed.height()
        || reference.width() == 0
    {
        return Err("the comparison rasters have different dimensions");
    }
    let width = reference.width() as usize;
    let mut changed = Footprint {
        x_min: usize::MAX,
        y_min: usize::MAX,
        x_max: 0,
        y_max: 0,
        count: 0,
    };
    for (index, (before, after)) in reference
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(removed.pixels().as_chunks::<4>().0)
        .enumerate()
    {
        if before == after {
            continue;
        }
        let (x, y) = (index % width, index / width);
        changed.x_min = changed.x_min.min(x);
        changed.y_min = changed.y_min.min(y);
        changed.x_max = changed.x_max.max(x);
        changed.y_max = changed.y_max.max(y);
        changed.count += 1;
    }
    if changed.x_min >= changed.x_max || changed.y_min >= changed.y_max {
        return Err("a footer button has no visible area in the final raster");
    }
    Ok(changed)
}

fn check_visible_pair(
    composed: &Composed,
    [first, second]: [Range<usize>; 2],
) -> Check<[Footprint; 2]> {
    if first.start < second.end && second.start < first.end {
        return Err("the two template item ranges overlap");
    }
    let raster = RasterOptions {
        size: CanvasSize::Width(WIDTH),
        background: Background::Transparent,
    };
    let original = composed
        .rasterize(&raster)
        .map_err(|_| "cannot rasterize the original MFD")?
        .image;
    let a = changed_pixels(
        &original,
        &composed.rasterize_without_items(&raster, first)?,
    )?;
    let b = changed_pixels(
        &original,
        &composed.rasterize_without_items(&raster, second)?,
    )?;
    let side_by_side = a.x_max < b.x_min || b.x_max < a.x_min;
    let same_row = a.y_max >= b.y_min && b.y_max >= a.y_min;
    if !side_by_side || !same_row {
        return Err("the visible frame areas do not sit side by side in the footer");
    }
    Ok([a, b])
}
