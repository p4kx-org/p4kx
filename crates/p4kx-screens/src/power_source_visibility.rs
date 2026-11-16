// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io::Write;

use p4kx_dataforge::{DataForge, Guid};

use super::diagnostics::AuthoredWidgets;
use super::{Check, black, heavy_font, parent_slot};
use crate::WidgetInput;
use crate::canvas::Canvas;
use crate::walk::Walk;
use crate::widget::Widget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Observation {
    LaidOut,
    NotInstantiated,
    BoundInactive,
    StyledInactive,
    AuthoredInactive,
}

impl Walk<'_, '_, '_, '_> {
    pub(crate) fn trace_enter_canvas(&mut self, canvas: &Canvas, roots: &[usize]) {
        if let Some(trace) = &mut self.cx.power_trace {
            trace.visibility.enter(
                canvas,
                roots,
                self.templates.len(),
                self.stack.contains(&trace.scope),
            );
        }
    }

    pub(crate) fn trace_leave_canvas(&mut self) {
        if let Some(trace) = &mut self.cx.power_trace {
            trace.visibility.leave();
        }
    }

    pub(crate) fn trace_site(&mut self, widget: &Widget, instance: bool, observed: Observation) {
        if !instance && let Some(trace) = &mut self.cx.power_trace {
            trace
                .visibility
                .observe(widget, self.templates.len(), observed);
        }
    }

    pub(crate) fn trace_inactive(
        &mut self,
        index: usize,
        widget: &Widget,
        instance: bool,
        styled: Option<bool>,
    ) {
        let gate = if styled == Some(false) {
            Observation::StyledInactive
        } else if self.rests.last().and_then(|state| state.active(index)) == Some(false) {
            Observation::BoundInactive
        } else {
            Observation::AuthoredInactive
        };
        self.trace_site(widget, instance, gate);
    }
}

#[derive(Debug)]
struct Candidate {
    chain: Vec<WidgetInput>,
    fonts: Vec<Guid>,
}

#[derive(Debug)]
struct ScopeCanvas {
    template_depth: usize,
    authored: AuthoredWidgets,
    candidates: Vec<Candidate>,
    observations: HashMap<WidgetInput, Observation>,
}

#[derive(Debug)]
pub(super) struct Visibility {
    active: Vec<Option<usize>>,
    canvases: Vec<ScopeCanvas>,
}

#[derive(Debug, Default)]
pub(super) struct Counts {
    canvases: usize,
    laid_out: usize,
    not_instantiated: usize,
    bound_inactive: usize,
    styled_inactive: usize,
    authored_inactive: usize,
    unexplained: usize,
}

impl Visibility {
    pub(super) const fn new() -> Self {
        Self {
            active: Vec::new(),
            canvases: Vec::new(),
        }
    }

    pub(super) fn enter(
        &mut self,
        canvas: &Canvas,
        roots: &[usize],
        template_depth: usize,
        in_scope: bool,
    ) {
        let scope = in_scope.then(|| {
            let next = self.canvases.len();
            let (candidates, authored) = candidates(canvas, roots);
            self.canvases.push(ScopeCanvas {
                template_depth,
                authored,
                candidates,
                observations: HashMap::new(),
            });
            next
        });
        self.active.push(scope);
    }

    pub(super) fn leave(&mut self) {
        self.active.pop();
    }

    pub(super) fn observe(
        &mut self,
        widget: &Widget,
        template_depth: usize,
        observation: Observation,
    ) {
        if let Some(Some(index)) = self.active.last()
            && let Some(canvas) = self.canvases.get_mut(*index)
            && canvas.template_depth == template_depth
        {
            canvas.observations.insert(widget.input, observation);
        }
    }

    pub(super) fn root_observation(&self, input: WidgetInput) -> Option<Observation> {
        self.canvases.first()?.observations.get(&input).copied()
    }

    pub(super) fn counts(&self, forge: &DataForge<'_>) -> Check<Counts> {
        let mut counts = Counts {
            canvases: self.canvases.len(),
            ..Counts::default()
        };
        let mut fonts = HashMap::new();
        for canvas in &self.canvases {
            for candidate in &canvas.candidates {
                if candidate.has_heavy_font(forge, &mut fonts)? {
                    counts.record(candidate.observation(&canvas.observations));
                }
            }
        }
        Ok(counts)
    }
}

impl Candidate {
    fn has_heavy_font(
        &self,
        forge: &DataForge<'_>,
        known: &mut HashMap<Guid, bool>,
    ) -> Check<bool> {
        for &font in &self.fonts {
            let heavy = match known.entry(font) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => *entry.insert(heavy_font(forge, font)?),
            };
            if heavy {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn observation(&self, observations: &HashMap<WidgetInput, Observation>) -> Option<Observation> {
        let (label, ancestors) = self.chain.split_first()?;
        observations.get(label).copied().or_else(|| {
            ancestors
                .iter()
                .filter_map(|ancestor| observations.get(ancestor).copied())
                .find(|observation| *observation != Observation::LaidOut)
        })
    }
}

impl Counts {
    fn record(&mut self, observation: Option<Observation>) {
        match observation {
            Some(Observation::LaidOut) => self.laid_out += 1,
            Some(Observation::NotInstantiated) => self.not_instantiated += 1,
            Some(Observation::BoundInactive) => self.bound_inactive += 1,
            Some(Observation::StyledInactive) => self.styled_inactive += 1,
            Some(Observation::AuthoredInactive) => self.authored_inactive += 1,
            None => self.unexplained += 1,
        }
    }
}

pub(super) fn report(
    out: &mut impl Write,
    forge: &DataForge<'_>,
    powered: &Visibility,
    rest: &Visibility,
) -> Check {
    let mut result = Ok(());
    for (state, visibility) in [("powered", powered), ("at rest", rest)] {
        let mut authored = AuthoredWidgets::default();
        for canvas in &visibility.canvases {
            authored.add(&canvas.authored);
        }
        authored.write(out, state)?;
        let counts = match visibility.counts(forge) {
            Ok(counts) => counts,
            Err(reason) => {
                writeln!(out, "{state} candidate counts unavailable: {reason}")
                    .map_err(|_| "cannot report the candidate census failure")?;
                result = result.and(Err(reason));
                continue;
            }
        };
        writeln!(
            out,
            "{state}: {} entered POWER canvases, {} authored candidates laid out, {} gated by Instantiated, {} gated by IsActive binding, {} gated by IsActive style, {} authored inactive, {} unattributed",
            counts.canvases,
            counts.laid_out,
            counts.not_instantiated,
            counts.bound_inactive,
            counts.styled_inactive,
            counts.authored_inactive,
            counts.unexplained,
        )
        .map_err(|_| "cannot report the candidate census")?;
        if counts.unexplained != 0 {
            result = result.and(Err(
                "an original POWER label candidate has no observed layout or activity gate",
            ));
        }
    }
    result
}

fn candidates(canvas: &Canvas, roots: &[usize]) -> (Vec<Candidate>, AuthoredWidgets) {
    let mut pending: Vec<_> = roots.iter().copied().map(|index| (index, None)).collect();
    let mut parents = HashMap::new();
    let mut candidates = Vec::new();
    let mut authored = AuthoredWidgets::default();
    while let Some((index, parent)) = pending.pop() {
        let Entry::Vacant(entry) = parents.entry(index) else {
            continue;
        };
        entry.insert(parent);
        if let Some(widget) = canvas.widgets.get(index) {
            authored.observe(widget, parent.and_then(|parent| canvas.widgets.get(parent)));
        }
        if let Some(candidate) = candidate(canvas, index, parent, &parents) {
            candidates.push(candidate);
        }
        if let Some(children) = canvas.children.get(index) {
            pending.extend(children.iter().copied().map(|child| (child, Some(index))));
        }
    }
    (candidates, authored)
}

fn candidate(
    canvas: &Canvas,
    index: usize,
    parent: Option<usize>,
    parents: &HashMap<usize, Option<usize>>,
) -> Option<Candidate> {
    let widget = canvas.widgets.get(index)?;
    let parent = canvas.widgets.get(parent?)?;
    parent_slot(parent)?;
    let text = widget.text.as_ref()?;
    if !widget
        .inline
        .iter()
        .any(|entry| black(entry.modifiers.fill.as_ref()))
    {
        return None;
    }
    let fonts = text
        .font_style
        .into_iter()
        .chain(
            widget
                .inline
                .iter()
                .filter_map(|entry| entry.modifiers.text.font_style),
        )
        .collect();
    let chain = std::iter::successors(Some(index), |index| parents.get(index).copied().flatten())
        .filter_map(|index| canvas.widgets.get(index).map(|widget| widget.input))
        .collect();
    Some(Candidate { chain, fonts })
}

#[cfg(test)]
mod tests {
    use p4kx_dataforge::DataForge;
    use p4kx_testkit::{
        DataForgeBuilder, TestCanvas, TestColour, TestDim, TestModifier, TestOp, TestRecord,
        TestStyleEntry, TestValue, TestWidget, entity_schema, screens_schema,
    };

    use super::super::Check;
    use crate::{ScreenComposer, Variable};

    const CANVAS: u128 = 0x5a20;
    const FONT: u128 = CANVAS + 1;

    #[test]
    fn a_never_visited_label_keeps_the_actual_gate_on_its_original_ancestor() -> Check {
        let bytes = database();
        let forge = DataForge::open(&bytes).map_err(|_| "cannot open synthetic DataForge")?;
        let canvas = forge.record(0).ok_or("missing synthetic canvas")?.guid();
        let supplied = [Variable::new("variable", "true").map_err(|_| "invalid test variable")?];
        for (variables, launched) in [(&[][..], false), (&supplied[..], true)] {
            let mut composer = ScreenComposer::new(&forge).with_variables(variables);
            composer.power_scope = Some(canvas);
            let trace = composer
                .compose(&canvas.to_string(), None)
                .map_err(|_| "cannot compose synthetic screen")?
                .power_trace
                .ok_or("no synthetic source trace")?;
            let counts = trace.visibility.counts(&forge)?;
            assert_eq!(counts.canvases, 1, "the canvas was selected in both states");
            assert_eq!(counts.laid_out, usize::from(launched), "original label");
            assert_eq!(
                counts.not_instantiated,
                usize::from(!launched),
                "the original label vanishes through its ancestor's Instantiated binding"
            );
            assert_eq!(
                counts.unexplained, 0,
                "every source candidate is accounted for"
            );
        }
        Ok(())
    }

    fn database() -> Vec<u8> {
        let paint =
            TestStyleEntry::new("black").set(TestModifier::Fill(TestColour::Solid([0, 0, 0, 255])));
        let gate = TestWidget::display("gate").size(TestDim::Fixed(8.0), TestDim::Fixed(8.0));
        let parent = TestWidget::display("panel")
            .parent("gate")
            .background(TestColour::Slot("Accent1", 1.0))
            .size(TestDim::Fixed(8.0), TestDim::Fixed(8.0));
        let label = TestWidget::new("BuildingBlocks_WidgetText", "label")
            .parent("panel")
            .inline_style(&paint)
            .with("fontStyle", TestValue::Reference(FONT))
            .size(TestDim::Fixed(4.0), TestDim::Fixed(4.0));
        let canvas = TestCanvas::new(CANVAS, "visibility", 8.0, 8.0)
            .widget(gate)
            .widget(parent)
            .widget(label)
            .operation("flag", TestOp::BoolVar)
            .operation("bind", TestOp::BoolField("gate", "Instantiated", "flag"));
        let font = TestRecord::new(
            FONT,
            "BuildingBlocks_FontStyle",
            "BuildingBlocks_FontStyle.synthetic",
            "font/synthetic.xml",
        )
        .value(
            "paintFile",
            TestValue::text("UI/fonts/BlenderPro-Heavy.slug"),
        );
        screens_schema(entity_schema(DataForgeBuilder::new()))
            .record(canvas.record())
            .record(font)
            .build()
    }
}
