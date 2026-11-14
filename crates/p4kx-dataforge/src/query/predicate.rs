// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::query::operand::{Operand, OperandError};
use crate::query::reader::Reader;
use crate::query::selection_control::{Control, EvaluationError};
use crate::query::walk::Place;
use crate::query::{Comparison, Condition, Operator, QueryError, QueryErrorKind};
use crate::{DataForge, DataType};

/// A compiled `where` condition, which a Record has to meet; each comparison is a test numbered in condition order.
#[derive(Debug, Clone)]
pub(crate) enum Predicate {
    Test(usize, Operator),
    Not(Box<Self>),
    And(Box<[Self]>),
    Or(Box<[Self]>),
}

/// How one comparison reads and compares the values in an instance of one struct.
#[derive(Debug, Clone)]
pub(crate) struct Test {
    pub(crate) reader: Reader,
    pub(crate) operand: Operand,
}

impl Test {
    /// Returns the test of `comparison` over values of type `leaf` read by `reader`, or `None`
    /// when the literal cannot compare with them.
    pub(crate) fn of(comparison: &Comparison, (reader, leaf): (Reader, DataType)) -> Option<Self> {
        let operand = Operand::of(leaf, comparison.operator, &comparison.literal).ok()?;
        Some(Self { reader, operand })
    }
}

impl Predicate {
    /// Returns the predicate of `condition` and its comparisons in test order.
    pub(crate) fn of(condition: &Condition) -> (Self, Vec<&Comparison>) {
        let mut comparisons = Vec::new();
        let predicate = Self::numbered(condition, &mut comparisons);
        (predicate, comparisons)
    }

    fn numbered<'c>(condition: &'c Condition, comparisons: &mut Vec<&'c Comparison>) -> Self {
        match condition {
            Condition::Compare(comparison) => {
                comparisons.push(comparison);
                Self::Test(comparisons.len().saturating_sub(1), comparison.operator)
            }
            Condition::Not(inner) => Self::Not(Box::new(Self::numbered(inner, comparisons))),
            Condition::And(parts) => Self::And(
                parts
                    .iter()
                    .map(|part| Self::numbered(part, comparisons))
                    .collect(),
            ),
            Condition::Or(parts) => Self::Or(
                parts
                    .iter()
                    .map(|part| Self::numbered(part, comparisons))
                    .collect(),
            ),
        }
    }

    /// Evaluates the condition with Kleene truth values and demanded reading failures.
    pub(super) fn eval<'f>(
        &self,
        forge: &'f DataForge<'f>,
        tests: &[Option<Test>],
        place: Place<'f>,
        control: &Control<'_>,
    ) -> Result<Option<bool>, EvaluationError> {
        if !control.proceed() {
            return Err(EvaluationError::Stopped);
        }
        let result = match self {
            Self::Test(index, operator) => {
                let Some(test) = tests.get(*index).and_then(Option::as_ref) else {
                    return Ok(None);
                };
                test.reader
                    .any(
                        forge,
                        place,
                        &|| control.proceed(),
                        control.observer(),
                        |value| test.operand.compare(*operator, value),
                    )
                    .map_err(|source| EvaluationError::Read {
                        comparison: *index,
                        source,
                    })
            }
            Self::Not(inner) => inner
                .eval(forge, tests, place, control)
                .map(|value| value.map(|holds| !holds)),
            Self::And(parts) => {
                decide(parts, false, |part| part.eval(forge, tests, place, control))
            }
            Self::Or(parts) => decide(parts, true, |part| part.eval(forge, tests, place, control)),
        };
        if control.proceed() {
            result
        } else {
            Err(EvaluationError::Stopped)
        }
    }
}

/// Keeps provisional failures until another part proves the result without them.
fn decide(
    parts: &[Predicate],
    decisive: bool,
    mut eval: impl FnMut(&Predicate) -> Result<Option<bool>, EvaluationError>,
) -> Result<Option<bool>, EvaluationError> {
    let mut result = Some(!decisive);
    let mut failure = None;
    for part in parts {
        match eval(part) {
            Ok(Some(holds)) if holds == decisive => return Ok(Some(decisive)),
            Ok(Some(_)) => {}
            Ok(None) => result = None,
            Err(EvaluationError::Stopped) => return Err(EvaluationError::Stopped),
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    failure.map_or(Ok(result), Err)
}

/// Checks that the literal can compare with one of the `leaves`, the types of the scalars the
/// compared path ends at in the structs it resolves in.
pub(crate) fn check(
    comparison: &Comparison,
    owner: Option<&str>,
    mut leaves: impl Iterator<Item = DataType>,
) -> Result<(), QueryError> {
    let Some(first) = leaves.next() else {
        return Ok(());
    };
    let operand = |leaf| Operand::of(leaf, comparison.operator, &comparison.literal);
    let Err(error) = operand(first) else {
        return Ok(());
    };
    if leaves.any(|leaf| operand(leaf).is_ok()) {
        return Ok(());
    }
    let literal = comparison.literal.span;
    Err(match error {
        OperandError::Incompatible(kind) => QueryError::new(
            QueryErrorKind::IncompatibleComparison {
                name: comparison.path.written(),
                owner: owner.map(str::to_owned),
                data_type: first,
                operator: comparison.operator,
                literal: kind,
            },
            comparison.span(),
        ),
        OperandError::OutOfRange => {
            QueryError::new(QueryErrorKind::NumberOutOfRange(first), literal)
        }
        OperandError::InvalidGuid => QueryError::new(QueryErrorKind::InvalidGuid, literal),
    })
}
