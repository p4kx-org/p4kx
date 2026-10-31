// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use p4kx_dataforge::Guid;

use super::Rest;
use super::fields::{is_evaluated, layout_problem, tag_of};
use super::graph::{Binding, CANVAS_FIELD, FieldKind, Input, Node, Operations};
use super::state::{AtRest, PARAMETER, STATE_TAG, key_of};
use crate::text::is_placeholder;
use crate::variables::Variable;

/// Finds the canvas a game variable names, by GUID or record path.
pub(crate) type CanvasLookup<'a> = &'a dyn Fn(&str) -> Result<Guid, String>;

/// What a canvas is evaluated under: the typed and the localised parameters its widget hands it
/// and the game variables the caller sets.
#[derive(Clone, Copy)]
pub(crate) struct Given<'a> {
    pub(crate) inputs: &'a [(String, Rest)],
    pub(crate) texts: &'a [(String, String)],
    pub(crate) variables: &'a [Variable],
}

/// The field of a text widget that holds its text.
pub(crate) const TEXT_FIELD: &str = "Text";

const MAX_DEPTH: usize = 128;
const GAME_NAMES_CANVAS: &str = "a canvas that only the game names at run time";
const TOO_DEEP: &str = "nested too deep";
const NOT_FINITE: &str = "a number that is not finite";

type Outcome = Result<Rest, String>;

enum Memo {
    Fresh,
    Busy,
    Done(Outcome),
}

struct Eval<'a> {
    nodes: &'a [Node],
    kinds: &'a [String],
    inputs: &'a [(String, Rest)],
    texts: &'a [(String, String)],
    variables: &'a [Variable],
    lookup: CanvasLookup<'a>,
    memo: Vec<Memo>,
}

impl Eval<'_> {
    fn value(&mut self, at: usize, depth: usize) -> Outcome {
        if depth > MAX_DEPTH {
            return Err(TOO_DEEP.to_owned());
        }
        match self.memo.get(at) {
            None => return Err("an operation that does not exist".to_owned()),
            Some(Memo::Done(done)) => return done.clone(),
            Some(Memo::Busy) => return Err("a cycle".to_owned()),
            Some(Memo::Fresh) => {}
        }
        if let Some(slot) = self.memo.get_mut(at) {
            *slot = Memo::Busy;
        }
        let nodes = self.nodes;
        let outcome = nodes
            .get(at)
            .map_or_else(
                || Err("an operation that does not exist".to_owned()),
                |node| self.node(node, depth),
            )
            .and_then(|value| match value {
                Rest::Num(number) if !number.is_finite() => Err(NOT_FINITE.to_owned()),
                value => Ok(value),
            });
        if let Some(slot) = self.memo.get_mut(at) {
            *slot = match &outcome {
                Err(why) if why == TOO_DEEP => Memo::Fresh,
                _ => Memo::Done(outcome.clone()),
            };
        }
        outcome
    }

    fn input(&mut self, input: Input, depth: usize) -> Outcome {
        input.map_or_else(
            || Err("an input that is not connected".to_owned()),
            |at| self.value(at, depth + 1),
        )
    }

    fn flag(&mut self, input: Input, depth: usize) -> Result<bool, String> {
        match self.input(input, depth)? {
            Rest::Bool(flag) => Ok(flag),
            _ => Err("a value that is not a boolean".to_owned()),
        }
    }

    fn number(&mut self, input: Input, depth: usize) -> Result<f64, String> {
        match self.input(input, depth)? {
            Rest::Num(number) => Ok(number),
            _ => Err("a value that is not a number".to_owned()),
        }
    }

    fn number_or(&mut self, input: Input, constant: f64, depth: usize) -> Result<f64, String> {
        if input.is_some() {
            self.number(input, depth)
        } else {
            Ok(constant)
        }
    }

    fn all(&mut self, inputs: &[Input], wanted: bool, depth: usize) -> Outcome {
        let mut unknown = None;
        for &input in inputs {
            match self.flag(input, depth) {
                Ok(flag) if flag != wanted => return Ok(Rest::Bool(!wanted)),
                Ok(_) => {}
                Err(why) => unknown = Some(why),
            }
        }
        unknown.map_or(Ok(Rest::Bool(wanted)), Err)
    }

    /// Evaluates the canvas the node `input` picks: the branch of a boolean choice, or the canvas
    /// a game variable that the caller sets names.
    fn canvas(&mut self, input: Input, depth: usize) -> Result<Guid, String> {
        let nodes = self.nodes;
        match input.and_then(|at| nodes.get(at)) {
            Some(Node::CanvasChoice { canvases, input }) => {
                let at = usize::from(!self.flag(*input, depth + 1)?);
                canvases
                    .get(at)
                    .copied()
                    .flatten()
                    .ok_or_else(|| "a branch that names no canvas".to_owned())
            }
            Some(node @ Node::Variable { .. }) => match self.node(node, depth + 1)? {
                Rest::Text(named) if !named.trim().is_empty() => (self.lookup)(&named),
                _ => Err(GAME_NAMES_CANVAS.to_owned()),
            },
            _ => Err(GAME_NAMES_CANVAS.to_owned()),
        }
    }

    fn node(&mut self, node: &Node, depth: usize) -> Outcome {
        match node {
            Node::Variable { binding, zero } => self
                .variables
                .iter()
                .rev()
                .find(|variable| variable.matches(binding))
                .map_or_else(|| Ok(zero.clone()), |variable| variable.coerce(zero)),
            Node::Param { parameter, default } => Ok(self
                .inputs
                .iter()
                .find(|(name, given)| name == parameter && fits(given, default))
                .map_or_else(|| default.clone(), |(_, given)| given.clone())),
            Node::TextParam { parameter, default } => Ok(Rest::Text(
                self.texts
                    .iter()
                    .find(|(name, _)| name == parameter)
                    .map_or_else(|| default.clone(), |(_, given)| given.clone()),
            )),
            Node::Invert(input) => Ok(Rest::Bool(!self.flag(*input, depth)?)),
            Node::All(inputs) => self.all(inputs, true, depth),
            Node::Any(inputs) => self.all(inputs, false, depth),
            Node::Compare {
                cmp,
                constant,
                left,
                right,
            } => {
                let left = self.number(*left, depth)?;
                let right = self.number_or(*right, *constant, depth)?;
                Ok(Rest::Bool(cmp.test(left, right)))
            }
            Node::Switch {
                default,
                exceptions,
                input,
            } => {
                let value = self.number(*input, depth)?;
                Ok(Rest::Bool(exceptions.contains(&value) != *default))
            }
            Node::TextEmpty(input) => match self.input(*input, depth)? {
                Rest::Text(text) => Ok(Rest::Bool(text.is_empty())),
                _ => Err("a value that is not a text".to_owned()),
            },
            Node::Unsupported(kind) => Err(format!(
                "{} is not evaluated",
                self.kinds.get(*kind).map_or("an operation", String::as_str)
            )),
            Node::TagChoice { .. } | Node::TagSwitch { .. } => self.tag_node(node, depth),
            other => self.maths(other, depth),
        }
    }

    fn tag_node(&mut self, node: &Node, depth: usize) -> Outcome {
        match node {
            Node::TagChoice {
                tags,
                branches,
                input,
            } => {
                let at = usize::from(!self.flag(*input, depth)?);
                branches.get(at).copied().flatten().map_or_else(
                    || Ok(Rest::Tag(tags.get(at).copied().flatten())),
                    |branch| self.value(branch, depth + 1),
                )
            }
            Node::TagSwitch {
                pairs,
                default,
                input,
            } => {
                let value = self.number(*input, depth)?;
                let tag = pairs
                    .iter()
                    .find(|(first, _)| value.partial_cmp(first).is_some_and(Ordering::is_eq))
                    .map_or(*default, |(_, tag)| *tag);
                Ok(Rest::Tag(tag))
            }
            _ => Err("a node that is not a tag".to_owned()),
        }
    }

    fn maths(&mut self, node: &Node, depth: usize) -> Outcome {
        match node {
            Node::Choose {
                numbers,
                input,
                branches,
                whole,
            } => {
                let at = usize::from(!self.flag(*input, depth)?);
                let constant = numbers.get(at).copied().unwrap_or(0.0);
                let branch = branches.get(at).copied().flatten();
                let value = self.number_or(branch, constant, depth)?;
                Ok(Rest::Num(whole_if(value, *whole)))
            }
            Node::Same(input) => Ok(Rest::Num(self.number(*input, depth)?)),
            Node::Arith {
                op,
                amount,
                left,
                right,
                whole,
            } => {
                let left = self.number(*left, depth)?;
                let right = self.number_or(*right, *amount, depth)?;
                Ok(Rest::Num(whole_if(op.apply(left, right), *whole)))
            }
            Node::Clamp {
                bounds,
                input,
                inputs,
            } => {
                let value = self.number(*input, depth)?;
                let low = self.number_or(
                    inputs.first().copied().flatten(),
                    bounds.first().copied().unwrap_or(0.0),
                    depth,
                )?;
                let high = self.number_or(
                    inputs.get(1).copied().flatten(),
                    bounds.get(1).copied().unwrap_or(0.0),
                    depth,
                )?;
                Ok(Rest::Num(value.max(low).min(high)))
            }
            _ => Err("a node that is not a number".to_owned()),
        }
    }
}

/// Returns whether the value `given` for a parameter may stand where `default` is expected: the
/// same kind, or a tag for a string parameter, which is how a nested canvas receives one.
fn fits(given: &Rest, default: &Rest) -> bool {
    std::mem::discriminant(given) == std::mem::discriminant(default)
        || matches!((given, default), (Rest::Tag(_), Rest::Text(_)))
}

const fn whole_if(value: f64, whole: bool) -> f64 {
    if whole { value.trunc() } else { value }
}

impl AtRest {
    /// Evaluates the field bindings of `operations` with the game variables at the values of
    /// `variables`, else at zero, and the component parameters at `inputs`, else at their
    /// defaults. No game variable names a canvas.
    #[cfg(test)]
    pub(crate) fn of(
        operations: &Operations,
        inputs: &[(String, Rest)],
        variables: &[Variable],
    ) -> Self {
        let given = Given {
            inputs,
            texts: &[],
            variables,
        };
        Self::resolving(operations, &given, &|_| Err(GAME_NAMES_CANVAS.to_owned()))
    }

    /// Takes the text a localised field of a text or of a canvas parameter has at rest: a value
    /// that is a text of its own, not a placeholder the game fills in.
    fn bind_text(&mut self, eval: &mut Eval<'_>, binding: &Binding) {
        if !(binding.field.starts_with(PARAMETER) || binding.field == TEXT_FIELD) {
            return;
        }
        self.text_bound.insert(binding.widget);
        if let Ok(Rest::Text(text)) = eval.input(binding.input, 0)
            && !is_placeholder(&text)
        {
            self.localise(binding.widget, &binding.field, text);
        }
    }

    /// Evaluates as [`AtRest::of`] under `given`, with the canvases that game variables name
    /// found by `lookup`.
    pub(crate) fn resolving(
        operations: &Operations,
        given: &Given<'_>,
        lookup: CanvasLookup<'_>,
    ) -> Self {
        let Given {
            inputs,
            texts,
            variables,
        } = *given;
        let mut found = Self {
            key: key_of(inputs, texts),
            ..Self::default()
        };
        if operations.bindings.is_empty() {
            return found;
        }
        let mut eval = Eval {
            nodes: &operations.nodes,
            kinds: &operations.kinds,
            inputs,
            texts,
            variables,
            lookup,
            memo: operations.nodes.iter().map(|_| Memo::Fresh).collect(),
        };
        for binding in &operations.bindings {
            if binding.kind == FieldKind::Localised {
                found.bind_text(&mut eval, binding);
                continue;
            }
            if binding.kind == FieldKind::Text && binding.field == CANVAS_FIELD {
                match eval.canvas(binding.input, 0) {
                    Ok(canvas) => {
                        found.canvases.insert(binding.widget, canvas);
                    }
                    Err(why) => found
                        .unknown
                        .push((binding.widget, binding.field.clone(), why)),
                }
                continue;
            }
            if binding.kind == FieldKind::Text && binding.field.ends_with(STATE_TAG) {
                match eval.input(binding.input, 0).and_then(tag_of) {
                    Ok(tag) => found.tag(binding.widget, tag),
                    Err(why) => found
                        .unknown
                        .push((binding.widget, binding.field.clone(), why)),
                }
                continue;
            }
            if !is_evaluated(binding.kind, &binding.field) {
                found.left_out.push((binding.widget, binding.field.clone()));
                continue;
            }
            match eval.input(binding.input, 0).and_then(|value| {
                layout_problem(&binding.field, &value).map_or(Ok(value), |why| Err(why.to_owned()))
            }) {
                Ok(value) => found.set(binding.widget, &binding.field, value),
                Err(why) => found
                    .unknown
                    .push((binding.widget, binding.field.clone(), why)),
            }
        }
        found
    }
}
