// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value as Original};
use serde_json::Value;

use super::{Check, Link, link, owned, single};
use crate::original_item_arena as arena;
use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};

const GRAPH: &str = "SQedVisualGraphParams";
const STATE: &str = "SQedVisualGraphStateParams";
const ANIMATION: &str = "SQedVisualGraphAnimationParams";
const EFFECT: &str = "SQedVisualGraphEffectParams";
const TRANSITION: &str = "SQedVisualGraphTransitionParams";
const TIME: &str = "SQedVisualGraphTransitionTypeParams_TimeBased";
const VALUE: &str = "SQedVisualGraphTransitionTypeParams_ValueBased";

#[derive(Default)]
pub(super) struct Coverage {
    pub(super) states: usize,
    pub(super) transitions: usize,
    timings: bool,
    tracked_values: bool,
}

impl Coverage {
    pub(super) const fn complete(&self) -> bool {
        self.states > 0 && self.transitions > 0 && self.timings && self.tracked_values
    }
}

pub(super) fn representative(original: Option<&Original<'_>>) -> bool {
    let Some(Original::Struct(graph)) = original else {
        return false;
    };
    let mut timing = false;
    let mut value = false;
    for transition in slots(graph, "states").flat_map(|state| slots(state, "transitions")) {
        for key in ["animationTransitionType", "effectTransitionType"] {
            match transition.field(key) {
                Some(Original::Struct(selector)) if selector.type_name() == TIME => timing = true,
                Some(Original::Struct(selector)) if selector.type_name() == VALUE => value = true,
                _ => {}
            }
        }
    }
    timing && value
}

fn slots<'read, 'source>(
    source: &'read StructValue<'source>,
    field: &str,
) -> impl Iterator<Item = &'read StructValue<'source>> {
    match source.field(field) {
        Some(Original::Array(values)) => values.as_ref(),
        _ => &[],
    }
    .iter()
    .filter_map(|value| match value {
        Original::Struct(value) => Some(value),
        _ => None,
    })
}

pub(super) fn compare<'source>(
    original: Option<&Original<'source>>,
    row: &Value,
    trace: &Trace<'_, 'source>,
    coverage: &mut Coverage,
) -> Check {
    owned(original, row, trace, GRAPH, |graph, row| {
        link(
            graph.field("effectStrengthLink"),
            cell(row, "effectStrengthLink")?,
            trace,
            Link::Record,
        )?;
        ordered(
            graph.field("states"),
            cell(row, "states")?,
            trace,
            STATE,
            |source, row| {
                coverage.states += 1;
                compare_state(source, row, trace, coverage)
            },
        )
    })
}

fn compare_state<'source>(
    state: &StructValue<'source>,
    row: &Value,
    trace: &Trace<'_, 'source>,
    coverage: &mut Coverage,
) -> Check {
    owned(
        state.field("animationParams"),
        cell(row, "animationParams")?,
        trace,
        ANIMATION,
        |source, row| {
            original_label(source, row, "fragmentName", trace)?;
            original_label(source, row, "fragmentTag", trace)?;
            if let Some(raw @ Original::Bool(expected)) = source.field("loop") {
                let output = cell(row, "loop")?;
                require_state(output, "present")?;
                assert_eq!(
                    output.get("value").and_then(Value::as_bool),
                    Some(*expected),
                    "original loop boolean"
                );
                trace.check(output, raw, "payload")?;
            } else {
                require_state(cell(row, "loop")?, "absent")?;
            }
            single(source, row, "playbackBias", "playbackBias", trace)
        },
    )?;
    owned(
        state.field("effectParams"),
        cell(row, "effectParams")?,
        trace,
        EFFECT,
        |source, row| {
            for name in ["entityEffectTag", "entityEffectTrigger"] {
                link(source.field(name), cell(row, name)?, trace, Link::Record)?;
            }
            single(source, row, "strength", "strength", trace)
        },
    )?;
    ordered(
        state.field("transitions"),
        cell(row, "transitions")?,
        trace,
        TRANSITION,
        |source, row| {
            coverage.transitions += 1;
            original_label(source, row, "state", trace)?;
            for key in ["animationTransitionType", "effectTransitionType"] {
                compare_selector(source.field(key), cell(row, key)?, trace, coverage)?;
            }
            Ok(())
        },
    )
}

fn compare_selector<'source>(
    original: Option<&Original<'source>>,
    row: &Value,
    trace: &Trace<'_, 'source>,
    coverage: &mut Coverage,
) -> Check {
    let Some(Original::Struct(selector)) = original else {
        return owned(original, row, trace, "", |_, _| {
            Err("original transition body".into())
        });
    };
    let concrete = selector.type_name();
    owned(original, row, trace, concrete, |source, row| {
        let kind = match concrete {
            TIME => {
                coverage.timings = true;
                single(source, row, "seconds", "seconds", trace)?;
                "timeBased"
            }
            VALUE => {
                coverage.tracked_values = true;
                original_label(source, row, "context", trace)?;
                original_label(source, row, "variable", trace)?;
                "valueBased"
            }
            _ => {
                arena::compare(
                    trace,
                    source.fields().iter(),
                    cell(row, "additionalFields")?,
                )?;
                "authored"
            }
        };
        assert!(
            proof::text(row, "/kind")? == kind,
            "original concrete transition retains its shape"
        );
        Ok(())
    })
}

fn original_label<'source>(
    source: &StructValue<'source>,
    row: &Value,
    key: &str,
    trace: &Trace<'_, 'source>,
) -> Check {
    let output = cell(row, key)?;
    match source.field(key) {
        Some(raw @ (Original::String(text) | Original::Enum(text))) => {
            require_state(output, "present")?;
            assert!(
                proof::text(output, "/value")? == *text,
                "exact original QIG label"
            );
            trace.check(output, raw, "payload")
        }
        None => require_state(output, "absent"),
        _ => Err("original QIG label storage".into()),
    }
}

fn ordered<'source>(
    original: Option<&Original<'source>>,
    row: &Value,
    trace: &Trace<'_, 'source>,
    nominal: &str,
    mut compare: impl FnMut(&StructValue<'source>, &Value) -> Check,
) -> Check {
    let Some(raw) = original else {
        return require_state(row, "absent");
    };
    let Original::Array(slots) = raw else {
        return Err("original ordered visual graph slots".into());
    };
    require_state(row, "present")?;
    trace.check(row, raw, "selector")?;
    trace.part(row, raw, "$target", "selector")?;
    let output = proof::rows(row, "/value")?;
    assert_eq!(
        output.len(),
        slots.len(),
        "every original positioned graph slot survives"
    );
    for (raw, export) in slots.iter().zip(output) {
        let Original::Struct(value) = raw else {
            return Err("original visual graph array Class".into());
        };
        assert!(
            value.type_name() == nominal,
            "ordered Class uses its exact nominal type"
        );
        require_state(export, "present")?;
        trace.part(export, raw, "$type", "selector")?;
        let payload = cell(export, "value")?;
        arena::source_type(trace, raw, value, payload)?;
        compare(value, payload)?;
    }
    Ok(())
}
