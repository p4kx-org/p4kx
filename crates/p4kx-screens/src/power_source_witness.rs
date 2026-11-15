// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::io::{self, Write};

use p4kx_dataforge::{DataForge, Guid, Reference, Step};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin};
use p4kx_gfx::Rgba;
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::canvas::find_canvas;
use crate::colour::{ColorSpec, Palette, PaletteSite, scale_alpha, slot_names};
use crate::look::Look;
use crate::reader::{as_struct, child, items, reference};
use crate::walk::Walk;
use crate::widget::Widget;

#[path = "power_source_inputs.rs"]
mod original;

use self::original::{check_pair, heavy_font, palette_colour, same_widget, tree, widget_source};

#[path = "power_source_witness_tests.rs"]
mod tests;
use crate::{ScreenComposer, Variable, WidgetInput};

const MFD: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/general/m_mfd_screen";
const POWER: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/power/mc_s_power_master";
const DISPLAY: &str = "BuildingBlocks_DisplayWidget";
const FONT_STYLE: &str = "BuildingBlocks_FieldModifierRecordRefTypeFontStyleRecord";
const LABEL_BACKGROUND: [&str; 2] = ["Accent1", "Accent2"];

type Check<T = ()> = Result<T, &'static str>;

#[derive(Debug)]
pub(crate) struct Trace {
    scope: Guid,
    active: Vec<usize>,
    visits: Vec<Visit>,
    pub(crate) background: Option<Rgba>,
}

#[derive(Debug)]
struct Visit {
    input: WidgetInput,
    parent: Option<usize>,
    in_scope: bool,
    kind: VisitKind,
}

#[derive(Debug)]
enum VisitKind {
    Other,
    Parent {
        slot: String,
        drawn: Option<Rgba>,
        palette: Option<PaletteSite>,
        alpha: f32,
        has_override: bool,
    },
    Label {
        font: Option<Guid>,
        black: bool,
    },
}

impl Trace {
    pub(crate) const fn new(scope: Guid) -> Self {
        Self {
            scope,
            active: Vec::new(),
            visits: Vec::new(),
            background: None,
        }
    }

    pub(crate) fn enter(
        &mut self,
        (canvas, widget): (&[Guid], &Widget),
        (look, palette, alpha): (&Look<'_>, &Palette, f32),
    ) {
        let in_scope = canvas.contains(&self.scope);
        let kind = if in_scope {
            observed_kind(widget, look, palette, alpha, self.background.take())
        } else {
            VisitKind::Other
        };
        let next = self.visits.len();
        self.visits.push(Visit {
            input: widget.input,
            parent: self.active.last().copied(),
            in_scope,
            kind,
        });
        self.active.push(next);
    }

    pub(crate) fn leave(&mut self) {
        self.active.pop();
    }
}

impl Walk<'_, '_, '_, '_> {
    pub(crate) fn trace_begin(&mut self) {
        if let Some(trace) = &mut self.cx.power_trace {
            trace.background = None;
        }
    }

    pub(crate) fn trace_painted(&mut self, widget: &Widget, look: &Look<'_>, alpha: f32) {
        if let Some(trace) = &mut self.cx.power_trace {
            trace.enter((&self.stack, widget), (look, &self.cx.palette, alpha));
        }
    }

    pub(crate) fn trace_leave(&mut self) {
        if let Some(trace) = &mut self.cx.power_trace {
            trace.leave();
        }
    }
}

fn observed_kind(
    widget: &Widget,
    look: &Look<'_>,
    palette: &Palette,
    alpha: f32,
    drawn: Option<Rgba>,
) -> VisitKind {
    if widget.type_name == DISPLAY
        && widget.name.is_empty()
        && widget.background_enabled
        && let Some(ColorSpec::Slot { name, .. }) = &widget.background
        && LABEL_BACKGROUND.contains(&name.as_str())
    {
        return VisitKind::Parent {
            slot: name.clone(),
            drawn,
            palette: palette.sources.get(name).copied(),
            alpha,
            has_override: look.style.enable_background.is_some() || look.style.background.is_some(),
        };
    }
    if let Some(spec) = &widget.text
        && widget
            .inline
            .iter()
            .any(|entry| black(entry.modifiers.fill.as_ref()))
    {
        return VisitKind::Label {
            font: look.style.text.font_style.or(spec.font_style),
            black: black(look.fill),
        };
    }
    VisitKind::Other
}

fn black(colour: Option<&ColorSpec>) -> bool {
    matches!(colour, Some(ColorSpec::Solid(colour))
        if [colour.red, colour.green, colour.blue] == [0, 0, 0] && colour.alpha != 0)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_rsi_power_source_trace_binds_painted_labels_to_original_parents_and_palette() -> Check {
    let path = real_archive_path().map_err(|_| "missing real Archive")?;
    let key = real_archive_key().map_err(|_| "missing real Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "cannot open real Archive")?;
    let build = archive.build().ok_or("no installed Build manifest")?;
    let watch = ArchiveWatch::new(&archive);
    watch.check().map_err(|_| "Archive changed")?;
    let entry = DataForgeEntry::read_until(&archive, &|| watch.poll().is_err())
        .map_err(|_| "cannot read original DataForge Entry")?;
    let forge = entry.open().map_err(|_| "cannot open original DataForge")?;
    if !matches!(entry.origin_of(&forge), Some(DataForgeOrigin::Archive(_))) {
        return Err("the DataForge origin is unbound");
    }
    let scope = find_canvas(&forge, POWER).map_err(|_| "no unique POWER master canvas")?;
    let variables = [
        Variable::new("powerstate", "1").map_err(|_| "invalid power variable")?,
        Variable::new("landscapecanvasguid", POWER).map_err(|_| "invalid page variable")?,
        Variable::new("mfdviewname", "POWER MANAGEMENT").map_err(|_| "invalid footer variable")?,
    ];
    let powered = compose(&forge, scope, &variables)?;
    let rest = compose(&forge, scope, &[])?;
    let summary = inspect(&forge, &powered)?;
    watch.check().map_err(|_| "Archive changed")?;
    writeln!(
        io::stdout(),
        "RSI POWER build {} change {}: {} source-backed label pairs, {} target bindings ({} with local input), {} at-rest POWER visits",
        build.version(),
        build.change_number(),
        summary.pairs,
        summary.bindings,
        summary.local_inputs,
        rest.visits.iter().filter(|visit| visit.in_scope).count(),
    )
    .map_err(|_| "cannot report the witness")?;
    Ok(())
}

fn compose(forge: &DataForge<'_>, scope: Guid, variables: &[Variable]) -> Check<Trace> {
    let mut composer = ScreenComposer::new(forge).with_variables(variables);
    composer.power_scope = Some(scope);
    let screen = composer
        .compose(MFD, Some("rsi"))
        .map_err(|_| "cannot compose the RSI MFD")?;
    if screen.report().truncated {
        return Err("the composition reached a limit");
    }
    screen.power_trace.ok_or("composition has no source trace")
}

#[derive(Default)]
struct Summary {
    pairs: usize,
    bindings: usize,
    local_inputs: usize,
}

fn inspect(forge: &DataForge<'_>, trace: &Trace) -> Check<Summary> {
    let slots = slot_names(forge);
    let mut seen = BTreeSet::new();
    let mut summary = Summary::default();
    for (index, visit) in trace.visits.iter().enumerate() {
        let VisitKind::Label {
            font: Some(font),
            black: true,
        } = visit.kind
        else {
            continue;
        };
        let Some(parent) = visit.parent.and_then(|at| trace.visits.get(at)) else {
            continue;
        };
        let VisitKind::Parent {
            slot,
            drawn,
            palette,
            alpha,
            has_override,
        } = &parent.kind
        else {
            continue;
        };
        if !heavy_font(forge, font)? {
            continue;
        }
        let original = tree(forge, visit.input.record_index)?;
        check_pair(&original, visit.input, parent.input, slot, font)?;
        if *has_override {
            return Err("the effective cascade now overrides a candidate parent background");
        }
        let expected = palette_colour(
            forge,
            &slots,
            slot,
            palette.ok_or("no original palette slot")?,
        )?;
        let raw_parent = widget_source(&original, parent.input)?;
        let raw_colour = child(raw_parent, "background")
            .and_then(|value| value.field("color"))
            .and_then(ColorSpec::parse);
        let Some(ColorSpec::Slot {
            alpha: slot_alpha, ..
        }) = raw_colour
        else {
            return Err("the original parent no longer names a palette slot");
        };
        if *drawn != Some(scale_alpha(expected, slot_alpha * *alpha)) {
            return Err("the parent paint no longer agrees with its original colour slot");
        }
        summary.pairs += 1;
        count_target_bindings(forge, trace, index, &mut seen, &mut summary)?;
    }
    if summary.pairs == 0 {
        return Err("no original RSI POWER label and parent matched the source signature");
    }
    Ok(summary)
}

fn count_target_bindings(
    forge: &DataForge<'_>,
    trace: &Trace,
    index: usize,
    seen: &mut BTreeSet<(u32, &'static str, usize)>,
    summary: &mut Summary,
) -> Check {
    let mut current = Some(index);
    while let Some(visit) = current.and_then(|at| trace.visits.get(at)) {
        current = visit.parent;
        let input = visit.input;
        if !seen.insert((input.record_index, input.section.field_name(), input.slot)) {
            continue;
        }
        let owner = tree(forge, input.record_index)?;
        let operations = items(owner.root(), "operations");
        for binding in operations.iter().filter_map(as_struct) {
            if !reference(binding, "widget")
                .is_some_and(|value| same_widget(value, owner.guid(), input))
            {
                continue;
            }
            summary.bindings += 1;
            if reference(binding, "input")
                .is_some_and(|value| local_input(value, owner.guid(), operations.len()))
            {
                summary.local_inputs += 1;
            }
        }
    }
    Ok(())
}

fn local_input(input: &Reference<'_>, owner: Guid, operations: usize) -> bool {
    input.guid() == owner
        && matches!(input.location().map(p4kx_dataforge::Location::steps),
            Some([Step::Field("operations"), Step::Element(slot)])
                if usize::try_from(*slot).is_ok_and(|slot| slot < operations))
}
