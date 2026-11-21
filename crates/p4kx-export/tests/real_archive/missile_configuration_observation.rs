// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{StructValue, Value as Original};
use serde_json::Value;

use super::TestResult;
use crate::original_item_arena as arena;
use crate::original_item_numbers as numbers;
use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_item_durability::Trace;
use crate::real_archive_verification::print_line;

#[derive(Default)]
pub(super) struct Coverage {
    fields: BTreeMap<String, BTreeMap<&'static str, usize>>,
}

impl Coverage {
    fn observe(&mut self, path: &str, field: &str, state: &'static str) {
        *self
            .fields
            .entry(format!("{path}.{field}"))
            .or_default()
            .entry(state)
            .or_default() += 1;
    }

    pub(super) fn report(self) {
        for (name, states) in self.fields {
            let count = |state| states.get(state).copied().unwrap_or_default();
            print_line(&format!(
                "Missile {name}: {} present, {} absent, {} null, {} unreadable",
                count("present"),
                count("absent"),
                count("null"),
                count("unreadable"),
            ));
        }
    }
}

#[derive(Clone, Copy)]
enum Shape {
    Guidance,
    Targeting,
    Shake,
    Phase,
    Axes,
}

impl Shape {
    const fn numbers(self) -> &'static [&'static str] {
        match self {
            Self::Guidance => &[
                "dumbfireRotationScale",
                "linearSpeed",
                "boostPhaseDuration",
                "terminalPhaseEngagementTime",
                "terminalPhaseEngagementAngle",
                "fuelTankSize",
                "pidIntegralTerm",
                "pidDerivativeTerm",
                "pidProportionalTerm",
            ],
            Self::Targeting => &[
                "trackingSignalMin",
                "lockSignalAmplifier",
                "lockTime",
                "lockingAngle",
                "minRatioForLock",
                "lockIncreaseRate",
                "lockDecreaseRate",
                "lockRangeMin",
                "lockRangeMax",
                "lockResolutionRadius",
                "lockResolutionFalloffPower",
                "signalResilienceMin",
                "signalResilienceMax",
            ],
            Self::Shake => &["randomness", "range", "duration", "frequency"],
            Self::Phase => &["angularSpeed", "maxRotationAccel", "pidAggression"],
            Self::Axes => &["x", "y", "z"],
        }
    }

    const fn controls(self) -> &'static [&'static str] {
        match self {
            Self::Guidance => &["isDumbMissile"],
            Self::Targeting => &["trackingSignalType", "allowDumbFiring", "maxTimesCanMiss"],
            _ => &[],
        }
    }

    const fn owned(self) -> &'static [Owned] {
        match self {
            Self::Guidance => {
                &const {
                    [
                        Owned::new("boostPhase", "boostPhase", Self::Phase),
                        Owned::new("interceptPhase", "interceptPhase", Self::Phase),
                        Owned::new("terminalPhase", "terminalPhase", Self::Phase),
                    ]
                }
            }
            Self::Phase => {
                &const {
                    [
                        Owned::new(
                            "maxLinearAccelerationPositive",
                            "maxLinearAccelerationPositive",
                            Self::Axes,
                        ),
                        Owned::new(
                            "maxLinearAccelerationNegative",
                            "maxLinearAccelerationNegative",
                            Self::Axes,
                        ),
                    ]
                }
            }
            Self::Shake => {
                &const {
                    [
                        Owned::new("offsetAngle", "offsetAngle", Self::Axes),
                        Owned::new("offsetPosition", "offsetPosition", Self::Axes),
                    ]
                }
            }
            _ => &[],
        }
    }
}

#[derive(Clone, Copy)]
struct Owned {
    source: &'static str,
    output: &'static str,
    shape: Shape,
}

impl Owned {
    const fn new(source: &'static str, output: &'static str, shape: Shape) -> Self {
        Self {
            source,
            output,
            shape,
        }
    }
}

const BLOCKS: [Owned; 3] = [
    Owned::new("GCSParams", "guidance", Shape::Guidance),
    Owned::new("targetingParams", "targeting", Shape::Targeting),
    Owned::new("shakeParams", "shake", Shape::Shake),
];

pub(super) fn compare<'source>(
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
    coverage: &mut Coverage,
) -> TestResult {
    for block in BLOCKS {
        compare_owned(trace, source, row, "root", block, coverage)?;
    }
    Ok(())
}

fn compare_owned<'source>(
    trace: &Trace<'_, 'source>,
    owner: &StructValue<'source>,
    row: &Value,
    path: &str,
    member: Owned,
    coverage: &mut Coverage,
) -> TestResult<bool> {
    let value = cell(row, member.output)?;
    let raw = owner.field(member.source);
    if let Some(raw @ Original::Struct(source)) = raw {
        require_state(value, "present")?;
        trace.part(value, raw, "$type", "selector")?;
        coverage.observe(path, member.output, "present");
        let output = cell(value, "value")?;
        arena::source_type(trace, raw, source, output)?;
        let path = if path == "root" {
            member.output.to_owned()
        } else {
            format!("{path}.{}", member.output)
        };
        compare_block(trace, source, output, &path, member.shape, coverage)?;
        Ok(true)
    } else {
        nonpresent(trace, raw, value, path, member.output, coverage)
    }
}

fn compare_block<'source>(
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
    path: &str,
    shape: Shape,
    coverage: &mut Coverage,
) -> TestResult {
    let mut consumed = BTreeSet::new();
    for &name in shape.numbers().iter().chain(shape.controls()) {
        if compare_scalar(
            trace,
            source.field(name),
            cell(row, name)?,
            path,
            name,
            coverage,
        )? {
            consumed.insert(name);
        }
    }
    for &member in shape.owned() {
        if compare_owned(trace, source, row, path, member, coverage)? {
            consumed.insert(member.source);
        }
    }
    arena::additional(trace, source, row, consumed)?;
    Ok(())
}

fn compare_scalar<'source>(
    trace: &Trace<'_, 'source>,
    raw: Option<&Original<'source>>,
    output: &Value,
    path: &str,
    name: &str,
    coverage: &mut Coverage,
) -> TestResult<bool> {
    let expected = match raw {
        Some(Original::Single(value)) => numbers::expected(&Original::Single(*value))?.map(|value| {
            serde_json::json!({"rawBits": value.get("rawBits"), "finite": value.get("finite")})
        }),
        Some(Original::Enum(value)) => Some(serde_json::json!(value)),
        Some(Original::Bool(value)) => Some(serde_json::json!(value)),
        Some(Original::Int32(value)) => Some(serde_json::json!(value)),
        _ => None,
    };
    if let (Some(raw), Some(expected)) = (raw, expected) {
        require_state(output, "present")?;
        assert!(
            numbers::equal(&expected, cell(output, "value")?),
            "original missile scalar matches at {path}.{name}"
        );
        trace.check(output, raw, "payload")?;
        coverage.observe(path, name, "present");
        return Ok(true);
    }
    nonpresent(trace, raw, output, path, name, coverage)
}

fn nonpresent<'source>(
    trace: &Trace<'_, 'source>,
    raw: Option<&Original<'source>>,
    output: &Value,
    path: &str,
    name: &str,
    coverage: &mut Coverage,
) -> TestResult<bool> {
    let state = match raw {
        None => "absent",
        Some(Original::Null) => "null",
        Some(Original::Error(_)) => "unreadable",
        _ => return Err(format!("unrecognized source storage at {path}.{name}").into()),
    };
    require_state(output, state)?;
    assert!(
        cell(output, "value")?.is_null(),
        "nonpresent missile cells have null payload"
    );
    if let Some(raw) = raw {
        if state == "unreadable" {
            trace.attempted_value(output, raw)?;
        } else {
            trace.check(output, raw, "selector")?;
        }
    }
    coverage.observe(path, name, state);
    Ok(state == "null")
}
