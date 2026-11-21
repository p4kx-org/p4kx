// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{StructValue, Value, ValueError};

use crate::loadout_path::PathRead;
use crate::{LoadoutClassName, LoadoutReference, LoadoutReferenceSelection};

/// What a Loadout entry or a port default names to attach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetSpec {
    /// Nothing.
    None,
    /// The Record of this GUID.
    Reference(LoadoutReference),
    /// The Entity of this class name.
    ClassName(LoadoutClassName),
    /// A Reference that cannot be read.
    Invalid(ValueError),
}

pub(crate) fn reference_target(value: &Value<'_>) -> TargetSpec {
    match value {
        Value::Reference(link) => TargetSpec::Reference(LoadoutReference::unbound(link.guid())),
        Value::Error(ValueError::DanglingReference { guid }) => {
            TargetSpec::Reference(LoadoutReference::unbound(*guid))
        }
        Value::Error(error) => TargetSpec::Invalid(*error),
        _ => TargetSpec::None,
    }
}

/// Pairs an accepted Reference with its actual original getter selection.
pub(crate) fn reference_target_at(
    parent: &StructValue<'_>,
    value: &Value<'_>,
    paths: &PathRead<'_>,
) -> TargetSpec {
    let selection = if matches!(
        value,
        Value::Reference(_) | Value::Error(ValueError::DanglingReference { .. })
    ) {
        paths.reference(parent, value)
    } else {
        None
    };
    if paths.stopped() {
        return TargetSpec::None;
    }
    match reference_target(value) {
        TargetSpec::Reference(reference) => {
            TargetSpec::Reference(reference.with_selection(selection))
        }
        other => other,
    }
}

impl TargetSpec {
    pub(crate) const fn reference_selection(&self) -> Option<&LoadoutReferenceSelection> {
        match self {
            Self::Reference(reference) => reference.selection(),
            _ => None,
        }
    }
}
