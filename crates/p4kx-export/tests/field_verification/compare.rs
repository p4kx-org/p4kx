// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::units::{self, Normalization, Unit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Class {
    Equal,
    Different,
    ReferenceOnly,
    P4kxOnly,
    Missing,
    Unhandled,
    Derived,
    Uncomparable,
    Excluded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Rule {
    Exact,
    F32RoundTrip,
    Normalized {
        normalization: Normalization,
        reason: String,
    },
    Derived {
        reason: String,
    },
    Curated {
        reason: String,
    },
}

#[derive(Debug, Default)]
pub(crate) struct Summary {
    pub(crate) classes: BTreeMap<Class, u64>,
}

impl Summary {
    pub(crate) fn record(&mut self, class: Class) {
        *self.classes.entry(class).or_default() += 1;
    }

    pub(crate) fn count(&self, class: Class) -> u64 {
        self.classes.get(&class).copied().unwrap_or_default()
    }

    pub(crate) fn compared(&self) -> u64 {
        self.count(Class::Equal) + self.count(Class::Different)
    }

    pub(crate) fn succeeds(&self) -> bool {
        self.compared() > 0 && self.clean()
    }

    pub(crate) fn clean(&self) -> bool {
        self.count(Class::Different) == 0
            && self.count(Class::Unhandled) == 0
            && self.count(Class::Uncomparable) == 0
            && self.count(Class::ReferenceOnly) == 0
            && self.count(Class::P4kxOnly) == 0
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the policy compares the rounding cells of source f32 values"
)]
fn same_f32(left: f64, right: f64) -> bool {
    left.is_finite()
        && right.is_finite()
        && left.abs() <= f64::from(f32::MAX)
        && right.abs() <= f64::from(f32::MAX)
        && (left as f32).to_bits() == (right as f32).to_bits()
}

fn number(value: &Value) -> Option<f64> {
    const EXACT_INTEGER_LIMIT: u64 = 1 << 53;
    if value
        .as_i64()
        .is_some_and(|value| value.unsigned_abs() > EXACT_INTEGER_LIMIT)
        || value
            .as_u64()
            .is_some_and(|value| value > EXACT_INTEGER_LIMIT)
    {
        return None;
    }
    value.as_f64()
}

fn numbers(left: &Value, right: &Value, scale: f64, rounded: bool) -> Class {
    let Some((left, right)) = number(left).zip(number(right)) else {
        return Class::Unhandled;
    };
    let left = left * scale;
    if !left.is_finite() || !right.is_finite() {
        return Class::Uncomparable;
    }
    if (rounded && same_f32(left, right)) || (!rounded && left.to_bits() == right.to_bits()) {
        Class::Equal
    } else {
        Class::Different
    }
}

pub(crate) fn values(
    reference: Option<&Value>,
    catalog: Option<&Value>,
    reference_unit: &Unit,
    catalog_unit: &Unit,
    rule: &Rule,
) -> Class {
    let reference = reference.filter(|value| !value.is_null());
    let catalog = catalog.filter(|value| !value.is_null());
    let (left, right) = match (reference, catalog) {
        (None, None) => return Class::Missing,
        (Some(_), None) => return Class::ReferenceOnly,
        (None, Some(_)) => return Class::P4kxOnly,
        (Some(left), Some(right)) => (left, right),
    };
    if matches!(rule, Rule::Derived { .. } | Rule::Curated { .. }) {
        return Class::Derived;
    }
    let normalization = if let Rule::Normalized { normalization, .. } = rule {
        *normalization
    } else {
        Normalization::None
    };
    let Some(scale) = units::scale(reference_unit, catalog_unit, normalization) else {
        return Class::Uncomparable;
    };
    match rule {
        Rule::Exact if left.is_number() && right.is_number() => {
            if left
                .as_i64()
                .zip(right.as_i64())
                .is_some_and(|(left, right)| left == right)
                || left
                    .as_u64()
                    .zip(right.as_u64())
                    .is_some_and(|(left, right)| left == right)
            {
                Class::Equal
            } else if left.is_f64() || right.is_f64() {
                numbers(left, right, scale, false)
            } else {
                Class::Different
            }
        }
        Rule::Exact if left.is_boolean() || left.is_string() => {
            if left == right {
                Class::Equal
            } else {
                Class::Different
            }
        }
        Rule::F32RoundTrip | Rule::Normalized { .. } => numbers(left, right, scale, true),
        _ => Class::Unhandled,
    }
}
