// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{
    InputScope, SourceInventory, SourceLocation, SourceUseDescription, SourceUseRole,
};
use serde_json::{Value, json};

pub(crate) struct ExpectedUse {
    pub(crate) role: SourceUseRole,
    pub(crate) location: SourceLocation,
}

pub(crate) struct ExpectedOrigin<'expected> {
    pub(crate) encrypted: Option<bool>,
    pub(crate) complete: bool,
    pub(crate) uses: &'expected [ExpectedUse],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceSetProblem {
    Marker,
    Completeness,
    ReferenceShape,
    DuplicateId,
    UnknownUse,
    Scope,
    Contributions,
}

/// Checks serialized-to-snapshot closure; original admission is a separate fixture proof.
pub(crate) fn verify(
    cell: &Value,
    inventory: &SourceInventory,
    expected: &ExpectedOrigin<'_>,
) -> Result<(), SourceSetProblem> {
    if cell.get("containsEncryptedSource") != Some(&json!(expected.encrypted)) {
        return Err(SourceSetProblem::Marker);
    }
    if cell.get("sourceSetComplete").and_then(Value::as_bool) != Some(expected.complete) {
        return Err(SourceSetProblem::Completeness);
    }
    let actual = uses(cell, inventory)?;
    if actual.len() != expected.uses.len() {
        return Err(SourceSetProblem::Contributions);
    }
    for demanded in expected.uses {
        let count = actual
            .iter()
            .filter(|observed| {
                observed.role() == demanded.role && observed.location() == &demanded.location
            })
            .count();
        if count != 1 {
            return Err(SourceSetProblem::Contributions);
        }
    }
    Ok(())
}

pub(crate) fn value_use(ordinal: u64, field: &str, role: SourceUseRole) -> ExpectedUse {
    ExpectedUse {
        role,
        location: SourceLocation::Subresource {
            namespace: "dataforgeValue".to_owned(),
            indices: vec![0, ordinal],
            field: field.to_owned(),
        },
    }
}

fn uses<'inventory>(
    cell: &Value,
    inventory: &'inventory SourceInventory,
) -> Result<Vec<&'inventory SourceUseDescription>, SourceSetProblem> {
    let references = cell
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or(SourceSetProblem::ReferenceShape)?;
    let mut seen = HashSet::new();
    let mut actual = Vec::new();
    for reference in references {
        let id = reference.as_str().ok_or(SourceSetProblem::ReferenceShape)?;
        if !seen.insert(id) {
            return Err(SourceSetProblem::DuplicateId);
        }
        let observed = inventory
            .uses()
            .iter()
            .find(|observed| observed.id().as_str() == id)
            .ok_or(SourceSetProblem::UnknownUse)?;
        if observed.scope() != InputScope::Payload {
            return Err(SourceSetProblem::Scope);
        }
        actual.push(observed);
    }
    Ok(actual)
}
