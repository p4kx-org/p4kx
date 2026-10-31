// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::rc::Rc;

use p4kx_dataforge::Guid;

use crate::canvas::{Canvas, find_canvas};
use crate::colour::Palette;
use crate::fit::fit;
use crate::layout::Frame;
use crate::paint::Area;
use crate::style::{CanvasStyles, Uses};
use crate::walk::Walk;
use crate::walk_rest::Handover;
use crate::widget::CanvasSizing;

const MAX_SURVEYED: usize = 4096;

impl Walk<'_, '_, '_, '_> {
    pub(crate) fn enter(
        &mut self,
        canvas: &Canvas,
        frame: Area,
        alpha: f32,
        depth: usize,
        sizing: CanvasSizing,
    ) {
        self.cx.report.canvases += 1;
        self.stack.push(canvas.guid);
        let sheet = self.sheet(canvas);
        self.note_unsupported(canvas.guid, &sheet);
        self.sheets.push(sheet);
        let over = self.pending_sheet.take();
        if let Some(guid) = over {
            let sheet = self.override_sheet(guid);
            self.note_unsupported(guid, &sheet);
            self.sheets.push(sheet);
        }
        self.enter_rest(canvas);
        let inherited = std::mem::take(&mut self.cx.palette);
        self.cx.palette = Palette::of_canvas(
            self.store.forge(),
            self.slots,
            canvas,
            self.brand,
            &inherited,
            &mut self.palettes,
        );
        for &index in &canvas.unreachable {
            if let Some(widget) = canvas.widgets.get(index) {
                self.cx.report.note(
                    &widget.type_name,
                    "not reachable from a root: its parents form a cycle",
                    &widget.name,
                );
            }
        }
        self.push_chain(&over);
        self.lay_out(canvas, frame, alpha, depth, sizing);
        self.chain.pop();
        self.cx.palette = inherited;
        self.rests.pop();
        if over.is_some() {
            self.sheets.pop();
        }
        self.sheets.pop();
        self.stack.pop();
    }

    /// Lays the root widgets of `canvas` out in `frame`, under the scale the canvas asks for.
    fn lay_out(
        &mut self,
        canvas: &Canvas,
        frame: Area,
        alpha: f32,
        depth: usize,
        sizing: CanvasSizing,
    ) {
        let fitted = fit(canvas, frame, sizing);
        let pixel = self.pixel;
        let scaled = match fitted.scaled {
            Some((affine, scale)) if self.cx.painter.push_transform(&affine) => {
                self.pixel /= scale;
                Some(true)
            }
            Some(_) => None,
            None => Some(false),
        };
        if let Some(scaled) = scaled {
            self.push_chain(&(canvas.guid, self.rest_key()));
            let frame = Frame {
                content: fitted.layout,
                flex: None,
                centred: fitted.centred,
            };
            let roots = &canvas.roots;
            self.siblings(canvas, roots, roots.len(), &frame, alpha, depth);
            self.chain.pop();
            if scaled {
                self.cx.painter.pop_transform();
            }
        } else {
            self.cx.report.note(
                "Canvas",
                "scale of the canvas is not finite: not drawn",
                &canvas.record_path,
            );
        }
        self.pixel = pixel;
    }

    fn sheet(&mut self, canvas: &Canvas) -> Rc<CanvasStyles> {
        if let Some(cached) = self.cache.get(&canvas.guid) {
            return Rc::clone(cached);
        }
        let sheet = Rc::new(CanvasStyles::build(
            self.store.forge(),
            &mut self.styles,
            canvas,
            self.brand,
        ));
        self.cache.insert(canvas.guid, Rc::clone(&sheet));
        sheet
    }

    /// Finds out which selectors that need the shape of the tree the sheets of the screen use,
    /// reading every canvas that can be reached from `root`, so that each widget is made the
    /// same way wherever it is entered from.
    pub(crate) fn survey(&mut self, root: &Rc<Canvas>) {
        let mut seen = HashSet::from([root.guid]);
        let mut todo = vec![Rc::clone(root)];
        let mut uses = Uses::default();
        while let Some(canvas) = todo.pop() {
            let sheet = self.sheet(&canvas);
            uses = uses.or(sheet.uses());
            self.note_variables(&canvas);
            let mut reachable: Vec<Guid> = sheet
                .canvases()
                .chain(canvas.operations.canvas_choices())
                .chain(self.named_canvases(&canvas))
                .collect();
            for widget in &canvas.widgets {
                if let Some(style) = widget.stylesheet {
                    let over = self.override_sheet(style);
                    uses = uses.or(over.uses());
                    reachable.extend(over.canvases());
                }
                reachable.extend(widget.canvas);
                reachable.extend(
                    widget
                        .inline
                        .iter()
                        .filter_map(|entry| entry.modifiers.canvas),
                );
            }
            for guid in reachable {
                if seen.len() >= MAX_SURVEYED {
                    uses = Uses {
                        count: true,
                        child: true,
                    };
                } else if seen.insert(guid)
                    && let Some(nested) = self.store.get(guid)
                {
                    todo.push(nested);
                }
            }
        }
        self.shape = uses;
    }

    /// Returns the canvases that the game variables set by the caller name for the canvas widgets
    /// of `canvas`.
    fn named_canvases(&self, canvas: &Canvas) -> Vec<Guid> {
        let name = canvas.name();
        canvas
            .operations
            .canvas_variables()
            .filter_map(|binding| {
                let variable = self
                    .variables
                    .iter()
                    .rev()
                    .filter(|variable| variable.in_namespace(name))
                    .find(|variable| variable.matches(binding))?;
                find_canvas(self.store.forge(), variable.value()).ok()
            })
            .collect()
    }

    fn note_unsupported(&mut self, sheet_guid: Guid, sheet: &CanvasStyles) {
        if !self.noted.insert(sheet_guid) {
            return;
        }
        for (entry, selector) in sheet.unsupported() {
            self.cx.report.note(
                "StyleEntry",
                &format!("selector {selector} is not evaluated"),
                entry,
            );
        }
    }

    fn override_sheet(&mut self, style: Guid) -> Rc<CanvasStyles> {
        if let Some(cached) = self.cache.get(&style) {
            return Rc::clone(cached);
        }
        let sheet = Rc::new(CanvasStyles::of_style(
            self.store.forge(),
            &mut self.styles,
            style,
        ));
        self.cache.insert(style, Rc::clone(&sheet));
        sheet
    }

    /// Enters the canvas `guid` that a canvas widget names, in the box `frame`.
    pub(crate) fn nested(
        &mut self,
        guid: Guid,
        handover: Handover,
        frame: Area,
        sizing: CanvasSizing,
        alpha: f32,
        depth: usize,
    ) {
        if self.stack.contains(&guid) {
            self.cx
                .report
                .missing_canvases
                .push(format!("{guid} (a canvas that contains itself)"));
            return;
        }
        let Some(canvas) = self.store.get(guid) else {
            self.cx.report.missing_canvases.push(guid.to_string());
            return;
        };
        self.params.push(handover.text);
        self.typed.push(handover.typed);
        self.pending_sheet = handover.stylesheet;
        self.enter(&canvas, frame, alpha, depth, sizing);
        self.typed.pop();
        self.params.pop();
    }
}
