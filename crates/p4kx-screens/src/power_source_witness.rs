// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use p4kx_dataforge::{DataForge, Guid};
use p4kx_extraction::{Archive, ArchiveWatch, Build, DataForgeEntry, DataForgeOrigin};
use p4kx_gfx::Rgba;
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::canvas::find_canvas;
use crate::colour::{ColorSpec, Palette, PaletteSite, scale_alpha, slot_names};
use crate::look::Look;
use crate::reader::child;
use crate::report::Report;
use crate::walk::Walk;
use crate::widget::Widget;

#[path = "power_source_diagnostics.rs"]
mod diagnostics;
#[path = "power_source_inputs.rs"]
mod original;
#[path = "power_source_binding_trace.rs"]
mod source_bindings;
#[path = "power_source_visibility.rs"]
mod visibility;

use self::original::{check_pair, heavy_font, palette_colour, same_widget, tree, widget_source};
pub(crate) use self::visibility::Observation;

#[path = "button_source_witness.rs"]
mod button;
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
    visibility: visibility::Visibility,
    active: Vec<usize>,
    visits: Vec<Visit>,
    first_item: usize,
    vector_requests: Vec<String>,
    pub(crate) background: Option<Rgba>,
}

#[derive(Debug)]
struct Visit {
    input: WidgetInput,
    parent: Option<usize>,
    in_scope: bool,
    kind: VisitKind,
    styled_vector: Option<String>,
    vector_requests: Vec<String>,
    vectors: Option<Vec<(usize, String)>>,
    own_items: Range<usize>,
    last_item: usize,
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
            visibility: visibility::Visibility::new(),
            active: Vec::new(),
            visits: Vec::new(),
            first_item: 0,
            vector_requests: Vec::new(),
            background: None,
        }
    }

    pub(crate) fn record_vector_request(&mut self, path: &str) {
        self.vector_requests.push(path.to_owned());
    }

    pub(crate) fn enter(
        &mut self,
        (canvas, widget): (&[Guid], &Widget),
        (look, palette, alpha): (&Look<'_>, &Palette, f32),
        items: usize,
        vectors: Option<Vec<(usize, String)>>,
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
            styled_vector: look.style.picture.svg_path.clone(),
            vector_requests: std::mem::take(&mut self.vector_requests),
            vectors,
            own_items: self.first_item..items,
            last_item: items,
        });
        self.active.push(next);
    }

    pub(crate) fn leave(&mut self, items: usize) {
        if let Some(visit) = self.active.pop().and_then(|at| self.visits.get_mut(at)) {
            visit.last_item = items;
        }
    }
}

impl Walk<'_, '_, '_, '_> {
    pub(crate) fn trace_begin(&mut self) {
        if let Some(trace) = &mut self.cx.power_trace {
            trace.background = None;
            trace.first_item = self.cx.painter.len();
        }
    }

    pub(crate) fn trace_painted(&mut self, widget: &Widget, look: &Look<'_>, alpha: f32) {
        if let Some(trace) = &mut self.cx.power_trace {
            let items = self.cx.painter.len();
            let vectors = self.cx.inputs.as_ref().map(|inputs| {
                inputs
                    .vectors
                    .range(trace.first_item..items)
                    .map(|(&item, path)| (item, path.clone()))
                    .collect()
            });
            trace.enter(
                (&self.stack, widget),
                (look, &self.cx.palette, alpha),
                items,
                vectors,
            );
        }
    }

    pub(crate) fn trace_leave(&mut self) {
        if let Some(trace) = &mut self.cx.power_trace {
            trace.leave(self.cx.painter.len());
        }
    }
}

fn parent_slot(widget: &Widget) -> Option<&str> {
    if widget.type_name == DISPLAY
        && widget.background_enabled
        && let Some(ColorSpec::Slot { name, .. }) = &widget.background
        && LABEL_BACKGROUND.contains(&name.as_str())
    {
        Some(name)
    } else {
        None
    }
}

fn observed_kind(
    widget: &Widget,
    look: &Look<'_>,
    palette: &Palette,
    alpha: f32,
    drawn: Option<Rgba>,
) -> VisitKind {
    if let Some(name) = parent_slot(widget) {
        return VisitKind::Parent {
            slot: name.to_owned(),
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
    let (build, (summary, census)) = with_installed_forge(|_, forge| {
        let scope = find_canvas(forge, POWER).map_err(|_| "no unique POWER master canvas")?;
        let variables = power_variables()?;
        let (powered, report) = compose(forge, scope, &variables)?;
        let (rest, rest_report) = compose(forge, scope, &[])?;
        if !rest_report.variables.is_empty() {
            return Err("the at-rest composition unexpectedly supplied variables");
        }
        let mut out = io::stdout().lock();
        diagnostics::rendered(&mut out, "powered", forge, &powered, &report)?;
        diagnostics::rendered(&mut out, "at rest", forge, &rest, &rest_report)?;
        let census = visibility::report(&mut out, forge, &powered.visibility, &rest.visibility);
        Ok((inspect(forge, &powered, &variables, &report), census))
    })?;
    let mut stdout = io::stdout().lock();
    writeln!(
        stdout,
        "RSI POWER build {} change {}",
        build.version(),
        build.change_number(),
    )
    .map_err(|_| "cannot report the witness")?;
    let summary = summary?;
    let counts = summary.bindings.counts();
    writeln!(
        stdout,
        "{} source-backed label pairs, {} target bindings ({} to background fields), {} upstream operation occurrences, {} local input edges, {} supplied and {} absent variable declarations, {} source gaps, {} reported omissions",
        summary.pairs,
        counts.sites,
        counts.background_targets,
        counts.graph_nodes,
        counts.local_edges,
        counts.supplied,
        counts.absent,
        counts.source_gaps,
        counts.reported_omissions,
    )
    .map_err(|_| "cannot report the binding evidence")?;
    census
}

fn with_installed_forge<T>(
    run: impl FnOnce(&Archive, &DataForge<'_>) -> Check<T>,
) -> Check<(Build, T)> {
    let path = real_archive_path().map_err(|_| "missing real Archive")?;
    let key = real_archive_key().map_err(|_| "missing real Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "cannot open real Archive")?;
    let build = archive
        .build()
        .ok_or("no installed Build manifest")?
        .clone();
    let watch = ArchiveWatch::new(&archive);
    watch.check().map_err(|_| "Archive changed")?;
    let entry = DataForgeEntry::read_until(&archive, &|| watch.poll().is_err())
        .map_err(|_| "cannot read original DataForge Entry")?;
    let forge = entry.open().map_err(|_| "cannot open original DataForge")?;
    if !matches!(entry.origin_of(&forge), Some(DataForgeOrigin::Archive(_))) {
        return Err("the DataForge origin is unbound");
    }
    let result = run(&archive, &forge)?;
    watch.check().map_err(|_| "Archive changed")?;
    Ok((build, result))
}

fn power_variables() -> Check<[Variable; 3]> {
    Ok([
        Variable::new("powerstate", "1").map_err(|_| "invalid power variable")?,
        Variable::new("landscapecanvasguid", POWER).map_err(|_| "invalid page variable")?,
        Variable::new("mfdviewname", "POWER MANAGEMENT").map_err(|_| "invalid footer variable")?,
    ])
}

fn compose(forge: &DataForge<'_>, scope: Guid, variables: &[Variable]) -> Check<(Trace, Report)> {
    let mut composer = ScreenComposer::new(forge).with_variables(variables);
    composer.power_scope = Some(scope);
    let screen = composer
        .compose(MFD, Some("rsi"))
        .map_err(|_| "cannot compose the RSI MFD")?;
    if screen.report().truncated {
        return Err("the composition reached a limit");
    }
    let report = screen.report().clone();
    let trace = screen
        .power_trace
        .ok_or("composition has no source trace")?;
    Ok((trace, report))
}

#[derive(Default)]
struct Summary {
    pairs: usize,
    bindings: source_bindings::BindingEvidence,
}

fn inspect(
    forge: &DataForge<'_>,
    trace: &Trace,
    variables: &[Variable],
    report: &Report,
) -> Check<Summary> {
    let slots = slot_names(forge);
    let mut summary = Summary::default();
    let bindings = source_bindings::BindingInputs::new(forge, variables, report);
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
            return Err("the effective cascade overrides a candidate parent background");
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
        summary
            .bindings
            .inspect_ancestors(trace, index, &bindings)?;
    }
    if summary.pairs == 0 {
        return Err("no original RSI POWER label and parent matched the source signature");
    }
    Ok(summary)
}
