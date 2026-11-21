// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceInventory, SourceLocation, SourceUseRole};
use serde_json::Value;

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_origin::{self as origin, ExpectedOrigin, ExpectedUse};

pub(crate) struct Evidence<'read> {
    pub(crate) inventory: &'read SourceInventory,
    pub(crate) encrypted: Option<bool>,
    pub(crate) input_ref: &'read str,
}

impl Evidence<'_> {
    pub(crate) fn value(
        &self,
        cell: &Value,
        ordinal: u64,
        field: &str,
        role: SourceUseRole,
    ) -> TestResult {
        let location = SourceLocation::Subresource {
            namespace: "dataforgeValue".to_owned(),
            indices: vec![0, ordinal],
            field: field.to_owned(),
        };
        if role == SourceUseRole::Attempted {
            closed(cell, self.inventory, None, false, self.input_ref, &[])?;
            let count = self
                .inventory
                .uses()
                .iter()
                .filter(|observed| {
                    observed.source().as_str() == self.input_ref
                        && observed.scope() == InputScope::Payload
                        && observed.role() == SourceUseRole::Attempted
                        && observed.location() == &location
                })
                .count();
            assert_eq!(
                count, 1,
                "the genuine failed read is retained as an observation, not a datum contribution"
            );
            return Ok(());
        }
        closed(
            cell,
            self.inventory,
            self.encrypted,
            true,
            self.input_ref,
            &[(role, location)],
        )
    }
}

pub(crate) fn closed(
    cell: &Value,
    inventory: &SourceInventory,
    encrypted: Option<bool>,
    complete: bool,
    input_ref: &str,
    uses: &[(SourceUseRole, SourceLocation)],
) -> TestResult {
    let expected: Vec<_> = uses
        .iter()
        .map(|(role, location)| ExpectedUse {
            role: *role,
            location: location.clone(),
        })
        .collect();
    origin::verify(
        cell,
        inventory,
        &ExpectedOrigin {
            encrypted,
            complete,
            uses: &expected,
        },
    )
    .map_err(|problem| format!("exact original contribution set: {problem:?}"))?;
    let references = cell
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("closed source reference array")?;
    for reference in references {
        let id = reference.as_str().ok_or("source reference text")?;
        let observed = inventory
            .uses()
            .iter()
            .find(|observed| observed.id().as_str() == id)
            .ok_or("real frozen original use")?;
        assert_eq!(
            observed.source().as_str(),
            input_ref,
            "the use belongs to the actual admitted original input"
        );
    }
    Ok(())
}
