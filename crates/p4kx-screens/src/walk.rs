// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use p4kx_dataforge::Guid;

use crate::bindings::ParamValues;
use crate::canvas::{Canvas, CanvasStore};
use crate::colour::Palette;
use crate::context::Context;
use crate::elements;
use crate::geometry::Geometry;
use crate::layout::{Frame, Sibling, place};
use crate::limits::Limits;
use crate::look::Look;
use crate::paint::Area;
use crate::rest::{AtRest, RestParams};
use crate::style::preset::PresetStyles;
use crate::style::{CanvasStyles, Modifiers, StyleCache, Subject, TagPaths, Uses};
use crate::text::Shaped;
use crate::transform::Placement;
use crate::variables::Variable;
use crate::walk_instance::Override;
use crate::walk_measure::{MeasureKey, Probe};

pub(crate) struct Walk<'s, 'a, 'f, 't> {
    pub(crate) store: &'s mut CanvasStore<'a, 'f>,
    pub(crate) slots: &'s [String],
    pub(crate) tags: &'s TagPaths,
    pub(crate) brand: Option<&'s str>,
    pub(crate) cx: &'s mut Context<'t>,
    pub(crate) stack: Vec<Guid>,
    pub(crate) widgets: usize,
    pub(crate) limits: Limits,
    pub(crate) budget: u64,
    pub(crate) styles: StyleCache,
    pub(crate) palettes: HashMap<Guid, Palette>,
    pub(crate) sheets: Vec<Rc<CanvasStyles>>,
    pub(crate) above: Vec<Subject>,
    pub(crate) chain: Vec<u64>,
    pub(crate) cache: HashMap<Guid, Rc<CanvasStyles>>,
    pub(crate) pending_sheet: Option<Guid>,
    pub(crate) noted: HashSet<Guid>,
    pub(crate) left_out_noted: HashSet<Guid>,
    pub(crate) params: Vec<ParamValues>,
    pub(crate) resolved: HashMap<(Guid, usize, u64, u64, usize), Rc<Modifiers>>,
    pub(crate) shape: Uses,
    pub(crate) rests: Vec<Rc<AtRest>>,
    pub(crate) variables: &'s [Variable],
    pub(crate) known_variables: HashSet<(String, String)>,
    pub(crate) typed: Vec<RestParams>,
    pub(crate) at_rest: HashMap<(Guid, u64), Rc<AtRest>>,
    pub(crate) measured: HashMap<MeasureKey, [Option<f32>; 2]>,
    pub(crate) shaped: HashMap<MeasureKey, Shaped>,
    pub(crate) measures: usize,
    pub(crate) instances: usize,
    pub(crate) templates: Vec<(Guid, usize)>,
    pub(crate) overrides: Vec<Override>,
    pub(crate) pixel: f32,
    pub(crate) turned: usize,
    pub(crate) presets: Option<Rc<PresetStyles>>,
}

impl<'s, 'a, 'f, 't> Walk<'s, 'a, 'f, 't> {
    pub(crate) fn new(
        store: &'s mut CanvasStore<'a, 'f>,
        slots: &'s [String],
        tags: &'s TagPaths,
        brand: Option<&'s str>,
        cx: &'s mut Context<'t>,
        limits: Limits,
    ) -> Self {
        Self {
            store,
            slots,
            tags,
            brand,
            cx,
            stack: Vec::new(),
            widgets: 0,
            limits,
            budget: limits.evaluations,
            styles: StyleCache::new(),
            palettes: HashMap::new(),
            sheets: Vec::new(),
            above: Vec::new(),
            chain: Vec::new(),
            cache: HashMap::new(),
            pending_sheet: None,
            noted: HashSet::new(),
            left_out_noted: HashSet::new(),
            params: Vec::new(),
            resolved: HashMap::new(),
            shape: Uses::default(),
            rests: Vec::new(),
            variables: &[],
            known_variables: HashSet::new(),
            typed: Vec::new(),
            at_rest: HashMap::new(),
            measured: HashMap::new(),
            shaped: HashMap::new(),
            measures: 0,
            instances: 0,
            templates: Vec::new(),
            overrides: Vec::new(),
            pixel: 1.0,
            turned: 0,
            presets: None,
        }
    }

    /// Evaluates the bindings with the game variables `variables` instead of zero.
    pub(crate) const fn with_variables(mut self, variables: &'s [Variable]) -> Self {
        self.variables = variables;
        self
    }

    /// Resolves the widget `index`, the child at `position`, of which the first `own` of its
    /// siblings are not instances; `None` when it takes no part in the layout.
    fn prepare<'c>(
        &mut self,
        canvas: &'c Canvas,
        (position, index): (usize, usize),
        (own, content): (usize, Area),
        depth: usize,
    ) -> Option<Sibling<'c>> {
        let widget = canvas.widgets.get(index)?;
        if !self.instantiated(index) {
            self.note_gone(canvas, index, widget, "Instantiated");
            return None;
        }
        let mut plan = self.plan_kids(canvas, index, widget);
        let style = self.style_of(canvas, (index, widget), (position, &plan.kids));
        if self.budget == 0 || !self.shown(index, widget, &style) {
            if self.budget > 0 && style.active.is_none() {
                self.note_gone(canvas, index, widget, "IsActive");
            }
            return None;
        }
        let geometry = Geometry::of(widget, &style);
        self.commit_kids(widget, &mut plan);
        let has_children = !plan.kids.is_empty() || self.canvas_of(index, widget, &style).is_some();
        let probe = Probe {
            canvas,
            index,
            widget,
            style: &style,
            geometry: &geometry,
            kids: &plan.kids,
            own: plan.own,
            has_children,
            position,
        };
        let text_key = self
            .measures_text(&probe)
            .then(|| self.measure_key(&probe, content));
        let content = self.content_of(&probe, content, (depth, true));
        Some(Sibling {
            widget,
            index,
            has_children,
            content,
            text_key,
            geometry,
            style,
            own_kids: plan.own,
            kids: plan.kids,
            instance: position >= own,
            position,
        })
    }

    pub(crate) fn siblings(
        &mut self,
        canvas: &Canvas,
        indices: &[usize],
        own: usize,
        frame: &Frame<'_>,
        alpha: f32,
        depth: usize,
    ) {
        if self.widgets.saturating_add(indices.len()) > self.limits.widgets {
            self.cx.report.truncated = true;
            return;
        }
        let mut siblings: Vec<Sibling<'_>> = Vec::with_capacity(indices.len());
        for (position, &index) in indices.iter().enumerate() {
            let at = (position, index);
            if let Some(sibling) = self.prepare(canvas, at, (own, frame.content), depth) {
                siblings.push(sibling);
            }
            if self.budget == 0 {
                self.cx.report.truncated = true;
                break;
            }
        }
        let mut placed_all = place(frame, &siblings);
        placed_all.sort_by_key(|placed| siblings.get(placed.at).map_or(0, |s| s.widget.layer));
        for placed in placed_all {
            let Some(sibling) = siblings.get(placed.at) else {
                continue;
            };
            if placed.auto {
                self.cx.report.note(
                    &sibling.widget.type_name,
                    "automatic size taken as the size of the parent",
                    &sibling.widget.name,
                );
            }
            self.widget(canvas, sibling, placed.area, alpha, depth);
        }
    }

    fn widget(
        &mut self,
        canvas: &Canvas,
        sibling: &Sibling<'_>,
        area: Area,
        alpha: f32,
        depth: usize,
    ) {
        let shaped = sibling.text_key.and_then(|key| self.shaped.remove(&key));
        if depth > self.limits.depth
            || self.widgets >= self.limits.widgets
            || self.cx.painter.is_full()
        {
            self.cx.report.truncated = true;
            return;
        }
        let widget = sibling.widget;
        let geometry = &sibling.geometry;
        self.widgets += 1;
        let style = &*sibling.style;
        self.cx.report.widgets += 1;
        let own = self.alpha_of(sibling.index, widget, style);
        let alpha = if widget.inherits_alpha {
            alpha * own
        } else {
            own
        };
        self.note_approximations(widget, geometry);
        let moved = match Placement::of(area, geometry) {
            Placement::Plain => false,
            Placement::Flat => return,
            Placement::Moved(affine) => {
                if !self.cx.painter.push_transform(&affine) {
                    self.cx.report.note(
                        &widget.type_name,
                        "transform is not finite: not drawn",
                        &widget.name,
                    );
                    return;
                }
                self.turned += 1;
                true
            }
        };
        let font = self.font_of(widget, style);
        let mut look = Look::of(widget, style, self.pixel, font);
        look.bound_text = self.bound_text(canvas, sibling.index);
        look.shaped = shaped;
        if elements::draws_no_background(widget) {
            look.background = None;
        }
        elements::paint(self.cx, widget, &look, area, alpha);
        let clipped = widget.overflow.clips();
        if clipped {
            self.cx.painter.push_clip(area);
        }
        self.descend(canvas, sibling, area, alpha, depth);
        if clipped {
            self.cx.painter.pop_clip();
        }
        if moved {
            self.cx.painter.pop_transform();
            self.turned = self.turned.saturating_sub(1);
        }
    }

    fn descend(
        &mut self,
        canvas: &Canvas,
        sibling: &Sibling<'_>,
        area: Area,
        alpha: f32,
        depth: usize,
    ) {
        let widget = sibling.widget;
        let padding = sibling.geometry.padding;
        let content = area.inset(padding.top, padding.right, padding.bottom, padding.left);
        let place = (sibling.position, sibling.kids.as_slice());
        let subject = self.subject_of(canvas, (sibling.index, widget), place);
        self.push_subject(&subject, sibling.position);
        self.above.push(subject);
        if sibling.instance {
            self.templates.push((canvas.guid, sibling.index));
        }
        let overridden = self.push_override(
            canvas,
            (sibling.index, widget),
            (&sibling.kids, sibling.own_kids),
        );
        let frame = Frame {
            content,
            flex: sibling.geometry.flex.as_ref(),
            centred: false,
        };
        self.siblings(
            canvas,
            &sibling.kids,
            sibling.own_kids,
            &frame,
            alpha,
            depth + 1,
        );
        let chosen = self.canvas_of(sibling.index, widget, &sibling.style);
        if let Some(nested) = elements::nested_canvas(widget, chosen, self.store.forge()) {
            let handover = self.handover(sibling.index, widget);
            self.nested(
                nested,
                handover,
                content,
                widget.canvas_sizing,
                alpha,
                depth + 1,
            );
        } else if elements::is_standard(&widget.type_name) {
            self.cx.report.note(
                &widget.type_name,
                "the Archive has no canvas for this standard widget",
                &widget.name,
            );
        } else if widget.canvas_dangling {
            self.cx.report.missing_canvases.push(format!(
                "{} (a reference that no Record answers)",
                widget.name
            ));
        }
        if overridden {
            self.overrides.pop();
        }
        if sibling.instance {
            self.templates.pop();
        }
        self.above.pop();
        self.chain.pop();
    }
}
