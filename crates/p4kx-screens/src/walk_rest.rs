// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::rc::Rc;

use p4kx_dataforge::Guid;

use crate::bindings::ParamValues;
use crate::canvas::{Canvas, find_canvas};
use crate::elements;
use crate::report::VariableInForce;
use crate::rest::{AtRest, Given, RestParams, same_parameter};
use crate::style::{Modifiers, Subject};
use crate::variables::Variable;
use crate::walk::Walk;
use crate::widget::Widget;

const LAYOUT_FIELDS: [&str; 10] = [
    "Size",
    "Pos",
    "Anchor",
    "Pivot",
    "Scale",
    "Orientation",
    "Padding",
    "Margin",
    "MinSize",
    "MaxSize",
];

/// Names why a bound field is left out: layout fields in a group, any other by its name.
fn left_out_reason(field: &str) -> String {
    if field.ends_with("Behavior")
        || field.starts_with("Flex")
        || LAYOUT_FIELDS.iter().any(|prefix| field.starts_with(prefix))
    {
        "bound layout fields (behaviours, depth, flex) are not evaluated at rest".to_owned()
    } else {
        format!("bound field {field} is not evaluated at rest")
    }
}

/// What a widget hands to the canvas it holds: the localised and the typed parameter values.
pub(crate) struct Handover {
    pub(crate) text: ParamValues,
    pub(crate) typed: RestParams,
    pub(crate) stylesheet: Option<Guid>,
}

impl Walk<'_, '_, '_, '_> {
    /// Evaluates the bindings of `canvas` at rest under the parameters it is entered with and
    /// puts the result in force; the first time a state is made, what it leaves out is listed.
    pub(crate) fn enter_rest(&mut self, canvas: &Canvas) {
        let inputs = self.typed.last().cloned().unwrap_or_default();
        let texts = self.params.last().cloned().unwrap_or_default();
        let key = (canvas.guid, crate::rest::key_of(&inputs, &texts));
        let state = if let Some(known) = self.at_rest.get(&key) {
            Rc::clone(known)
        } else {
            let name = canvas.name();
            let variables: Cow<'_, [Variable]> = if self
                .variables
                .iter()
                .all(|variable| variable.in_namespace(name))
            {
                Cow::Borrowed(self.variables)
            } else {
                Cow::Owned(
                    self.variables
                        .iter()
                        .filter(|variable| variable.in_namespace(name))
                        .cloned()
                        .collect(),
                )
            };
            let forge = self.store.forge();
            let lookup = |text: &str| find_canvas(forge, text).map_err(|error| error.to_string());
            let given = Given {
                inputs: &inputs,
                texts: &texts,
                variables: &variables,
            };
            let state = Rc::new(AtRest::resolving(&canvas.operations, &given, &lookup));
            self.note_rest(canvas, &state);
            self.at_rest.insert(key, Rc::clone(&state));
            state
        };
        self.rests.push(state);
    }

    /// Remembers the paths the game variables of `canvas` are bound to, so that a setting that no
    /// canvas of the screen reads can be told.
    pub(crate) fn note_variables(&mut self, canvas: &Canvas) {
        if self.variables.is_empty() {
            return;
        }
        let name = canvas.name();
        self.known_variables.extend(
            canvas
                .operations
                .variable_names()
                .map(|path| (name.to_owned(), path.to_owned())),
        );
    }

    /// Returns the variables that were set, the last setting of each name, and whether a canvas
    /// of the screen reads it.
    pub(crate) fn variables_in_force(&self) -> Vec<VariableInForce> {
        let mut forced: Vec<VariableInForce> = Vec::new();
        for variable in self.variables {
            let entry = VariableInForce {
                name: variable.name().to_owned(),
                value: variable.value().to_owned(),
                used: self
                    .known_variables
                    .iter()
                    .any(|(canvas, path)| variable.in_namespace(canvas) && variable.matches(path)),
            };
            match forced
                .iter_mut()
                .find(|other| other.name.eq_ignore_ascii_case(&entry.name))
            {
                Some(other) => *other = entry,
                None => forced.push(entry),
            }
        }
        forced
    }

    fn note_rest(&mut self, canvas: &Canvas, state: &AtRest) {
        let name = |at: usize| {
            canvas
                .widgets
                .get(at)
                .map_or("", |widget| widget.name.as_str())
        };
        if self.left_out_noted.insert(canvas.guid) {
            for (widget, field) in state.left_out() {
                let reason = left_out_reason(field);
                self.cx.report.note("Binding", &reason, name(*widget));
            }
        }
        for (widget, field, why) in state.unknown() {
            let reason = format!("the input of the bound field {field} is not evaluated: {why}");
            self.cx.report.note("Binding", &reason, name(*widget));
        }
    }

    /// Returns what tells the state at rest of the canvas being laid out from its other states.
    pub(crate) fn rest_key(&self) -> u64 {
        self.rests.last().map_or(0, |state| state.key())
    }

    /// Returns whether the bindings leave widget `index` of the canvas being laid out in place.
    pub(crate) fn instantiated(&self, index: usize) -> bool {
        self.rests
            .last()
            .and_then(|state| state.instantiated(index))
            .unwrap_or(true)
    }

    /// Lists the root widget `index` of `canvas` as left out when a binding takes it out of the
    /// screen, so that an empty picture says why.
    pub(crate) fn note_gone(
        &mut self,
        canvas: &Canvas,
        index: usize,
        widget: &Widget,
        field: &str,
    ) {
        let bound_off = field == "Instantiated"
            || self
                .rests
                .last()
                .is_some_and(|state| state.active(index) == Some(false));
        if bound_off && canvas.roots.contains(&index) {
            let reason = if self.variables.is_empty() {
                format!(
                    "not drawn at rest: {field} is bound to game variables, which are zero at rest"
                )
            } else {
                format!("not drawn: {field} is bound to game variables, whose values leave it off")
            };
            self.cx.report.note("Binding", &reason, &widget.name);
        }
    }

    /// Returns whether widget `index` is shown: a style entry beats the bound value of
    /// `IsActive`, which beats the field of the widget.
    pub(crate) fn shown(&self, index: usize, widget: &Widget, style: &Modifiers) -> bool {
        style
            .active
            .or_else(|| self.rests.last().and_then(|state| state.active(index)))
            .unwrap_or(widget.active)
    }

    /// Returns the canvas the canvas widget `index` holds, `None` for any other widget: a style
    /// entry beats the bound `CanvasReferenceRecord`, which beats the field of the widget.
    pub(crate) fn canvas_of(
        &self,
        index: usize,
        widget: &Widget,
        style: &Modifiers,
    ) -> Option<Guid> {
        if !elements::is_canvas_widget(widget) {
            return None;
        }
        style
            .canvas
            .or_else(|| self.rests.last().and_then(|state| state.canvas(index)))
            .or(widget.canvas)
    }

    /// Returns the own alpha of widget `index`, in the same order of precedence.
    pub(crate) fn alpha_of(&self, index: usize, widget: &Widget, style: &Modifiers) -> f32 {
        style
            .alpha
            .or_else(|| self.rests.last().and_then(|state| state.alpha(index)))
            .unwrap_or(widget.alpha)
    }

    /// Returns the values widget `index` hands to its nested canvas: the bound parameters over
    /// the ones the widget names, the localised ones included.
    pub(crate) fn handover(&self, index: usize, widget: &Widget) -> Handover {
        let bound = self
            .rests
            .last()
            .map_or(&[][..], |state| state.typed(index));
        let typed = if bound.is_empty() {
            Rc::clone(&widget.typed)
        } else {
            widget
                .typed
                .iter()
                .filter(|(name, value)| {
                    !bound
                        .iter()
                        .any(|(other, other_value)| same_parameter(name, value, other, other_value))
                })
                .chain(bound)
                .cloned()
                .collect()
        };
        let named = self
            .rests
            .last()
            .map_or(&[][..], |state| state.localised(index));
        let text = if named.is_empty() {
            Rc::clone(&widget.params)
        } else {
            widget
                .params
                .iter()
                .filter(|(name, _)| !named.iter().any(|(other, _)| other == name))
                .chain(named)
                .cloned()
                .collect()
        };
        Handover {
            text,
            typed,
            stylesheet: widget.stylesheet,
        }
    }

    /// Returns widget `index` as the selectors see it, at `position` among its siblings and with
    /// the widgets `kids` as children where an entry looks at them.
    pub(crate) fn subject_of(
        &self,
        canvas: &Canvas,
        (index, widget): (usize, &Widget),
        (position, kids): (usize, &[usize]),
    ) -> Subject {
        let subject = self.subject_at(index, widget, position);
        if !self.shape.child {
            return subject;
        }
        let kids = kids
            .iter()
            .filter_map(|&kid| Some((kid, canvas.widgets.get(kid)?)))
            .enumerate()
            .map(|(at, (kid, widget))| self.subject_at(kid, widget, at))
            .collect();
        subject.with_kids(kids)
    }

    /// Returns widget `index` with its own tags, those of its standard look and the state tags
    /// the bindings of its canvas give it.
    fn subject_at(&self, index: usize, widget: &Widget, position: usize) -> Subject {
        let mut subject = Subject::of(&widget.type_name, &widget.tags).at(position);
        subject
            .tags
            .extend(elements::standard_tags(widget, self.tags));
        if let Some(state) = self.rests.last() {
            subject.tags.extend_from_slice(state.state_tags(index));
        }
        subject
    }

    /// Returns what the place of a widget adds to the key of a resolved style: its position among
    /// its siblings where an entry counts them, else nothing.
    pub(crate) const fn position_key(&self, position: usize) -> usize {
        if self.shape.count {
            position.saturating_add(1)
        } else {
            0
        }
    }
}
