// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use crate::flow_root_fixture::Case;
use crate::health_origin_fixture::TestResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    Field(usize),
    Element(usize),
}

pub(crate) struct Original {
    pub(crate) paths: Vec<Vec<Step>>,
    pub(crate) flow: Vec<Step>,
    pub(crate) target: Vec<Step>,
    pub(crate) id: Vec<Step>,
    pub(crate) property_instance: Vec<Step>,
    pub(crate) variable: Vec<Step>,
    pub(crate) objective_caller: Vec<Step>,
    pub(crate) property_caller: Vec<Step>,
}

impl Original {
    pub(crate) fn ordinal(&self, path: &[Step]) -> TestResult<u64> {
        let index = self
            .paths
            .iter()
            .position(|actual| actual == path)
            .ok_or("actual original path occurrence")?;
        Ok(u64::try_from(index)?)
    }
    pub(crate) fn union(&self) -> TestResult<Vec<u64>> {
        self.paths
            .iter()
            .enumerate()
            .filter(|(_, path)| {
                path.starts_with(&self.flow)
                    || path.starts_with(&self.id)
                    || path.starts_with(&self.variable)
            })
            .map(|(index, _)| Ok(u64::try_from(index)?))
            .collect()
    }
}

pub(crate) fn snapshot(root: &StructValue<'_>, case: Case) -> Original {
    let flow = vec![Step::Field(usize::from(matches!(case, Case::Overlap)))];
    let target = if matches!(case, Case::InsideFlow) {
        vec![Step::Field(0), Step::Field(0)]
    } else {
        vec![Step::Field(0)]
    };
    let mut id = target.clone();
    id.push(Step::Field(0));
    let property_instance = if matches!(case, Case::Overlap) {
        id.clone()
    } else {
        target.clone()
    };
    let mut variable = property_instance.clone();
    variable.push(Step::Field(usize::from(matches!(case, Case::InsideFlow))));
    let trigger_field = usize::from(matches!(case, Case::InsideFlow));
    let mut objective_caller = flow.clone();
    objective_caller.extend([
        Step::Field(trigger_field),
        Step::Element(0),
        Step::Field(0),
        Step::Field(0),
    ]);
    let mut property_caller = flow.clone();
    property_caller.extend([
        Step::Field(trigger_field),
        Step::Element(1),
        Step::Field(0),
        Step::Field(0),
    ]);
    Original {
        paths: enumerate(root),
        flow,
        target,
        id,
        property_instance,
        variable,
        objective_caller,
        property_caller,
    }
}

pub(crate) fn at<'tree, 'source>(
    root: &'tree StructValue<'source>,
    path: &[Step],
) -> TestResult<&'tree Value<'source>> {
    let (first, tail) = path.split_first().ok_or("actual root edge")?;
    let Step::Field(first) = first else {
        return Err("actual root field edge".into());
    };
    let mut value = root
        .fields()
        .get(*first)
        .ok_or("actual root field")?
        .value();
    for edge in tail {
        value = match (edge, value) {
            (Step::Field(index), Value::Struct(owner)) => owner
                .fields()
                .get(*index)
                .ok_or("actual nested field")?
                .value(),
            (Step::Element(index), Value::Array(values)) => {
                values.get(*index).ok_or("actual array slot")?
            }
            _ => return Err("actual source edge kind/shape mismatch".into()),
        };
    }
    Ok(value)
}

pub(crate) fn enumerate(root: &StructValue<'_>) -> Vec<Vec<Step>> {
    let mut work: Vec<_> = root
        .fields()
        .iter()
        .enumerate()
        .rev()
        .map(|(index, field)| (field.value(), vec![Step::Field(index)]))
        .collect();
    let mut paths = Vec::new();
    while let Some((value, path)) = work.pop() {
        paths.push(path.clone());
        match value {
            Value::Struct(value) => {
                for (index, field) in value.fields().iter().enumerate().rev() {
                    let mut child = path.clone();
                    child.push(Step::Field(index));
                    work.push((field.value(), child));
                }
            }
            Value::Array(values) => {
                for (index, value) in values.iter().enumerate().rev() {
                    let mut child = path.clone();
                    child.push(Step::Element(index));
                    work.push((value, child));
                }
            }
            _ => {}
        }
    }
    paths
}
