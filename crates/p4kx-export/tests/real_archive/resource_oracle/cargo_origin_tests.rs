// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::resource_quality_schema_fixture::OverrideEntryTarget;

use super::super::super::carriers::Uses;
use super::super::super::{Failure, Result};
use super::{ALPHA, Written, expected_wire};

const SLOT: &str = "/definitions/resourceTypes/rows/0/defaultCargoContainers/value/0";
const HEADER: &str = concat!(
    "/definitions/resourceTypes/rows/0/defaultCargoContainers/value/0/",
    "value/entityClass/value/targetRecordIndex"
);
const FIELD: &str = "defaultCargoContainers.one_eighthSCU";

struct Mutation {
    carrier: &'static str,
    location: Value,
    role: &'static str,
    stage: &'static str,
}

#[test]
fn actual_producer_cargo_origin_mutations_fail_with_unchanged_canonical_values() -> Result<()> {
    for target in [OverrideEntryTarget::Concrete, OverrideEntryTarget::Base] {
        let written = Written::new(target)?;
        expected_wire(&written)?;
        written.verify(&written.catalog)?;
        let uses = Uses::new(&written.catalog, written.entry.origin())?;
        let owner = uses.record_index(written.alpha()?)?;
        for mutation in mutations(owner) {
            let mut changed = written.catalog.clone();
            replace_location(&mut changed, &mutation)?;
            assert_eq!(
                changed.pointer(ALPHA),
                written.catalog.pointer(ALPHA),
                "the negative control changes original coordinates without changing retained row values or refs"
            );
            let Err(failure) = written.verify(&changed) else {
                return Err(Failure::check("cargo-coordinate-mutation-accepted"));
            };
            assert_eq!(
                failure.stage, mutation.stage,
                "the owning raw oracle rejects this precise cargo coordinate mismatch"
            );
        }
    }
    Ok(())
}

fn mutations(owner: u32) -> [Mutation; 4] {
    [
        Mutation {
            carrier: SLOT,
            location: cargo_location(owner, 1, FIELD),
            role: "payload",
            stage: "bound-cargo-slot-origin",
        },
        Mutation {
            carrier: SLOT,
            location: json!({"type":"subresource", "namespace":"dataforgeCargoSlots",
                "indices":[owner.to_string(), "0"], "field":FIELD}),
            role: "payload",
            stage: "original-subresource-namespace",
        },
        Mutation {
            carrier: SLOT,
            location: cargo_location(owner, 0, "defaultCargoContainers.twoSCU"),
            role: "payload",
            stage: "bound-cargo-slot-origin",
        },
        Mutation {
            carrier: HEADER,
            location: cargo_location(owner, 0, FIELD),
            role: "selector",
            stage: "bound-header-origin",
        },
    ]
}

fn cargo_location(owner: u32, ordinal: u64, field: &str) -> Value {
    json!({"type":"subresource", "namespace":"dataforgeCargoSlot",
        "indices":[owner.to_string(), ordinal.to_string()], "field":field})
}

fn replace_location(catalog: &mut Value, mutation: &Mutation) -> Result<()> {
    let references = catalog
        .pointer(mutation.carrier)
        .and_then(|carrier| carrier.get("sourceUsesRefs"))
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("cargo-test-carrier"))?;
    let [reference] = references.as_slice() else {
        return Err(Failure::check("cargo-test-one-original-use"));
    };
    let id = reference
        .as_str()
        .ok_or_else(|| Failure::check("cargo-test-use-id"))?
        .to_owned();
    replace_use_location(catalog, &id, mutation.role, &mutation.location)
}

pub(super) fn replace_use_location(
    catalog: &mut Value,
    id: &str,
    role: &str,
    replacement: &Value,
) -> Result<()> {
    let original = catalog
        .pointer_mut("/provenance/uses")
        .and_then(Value::as_array_mut)
        .and_then(|rows| {
            rows.iter_mut()
                .find(|row| row.get("id").and_then(Value::as_str) == Some(id))
        })
        .ok_or_else(|| Failure::check("cargo-test-original-use"))?;
    if original.get("role").and_then(Value::as_str) != Some(role) {
        return Err(Failure::check("cargo-test-original-role"));
    }
    let location = original
        .get_mut("location")
        .ok_or_else(|| Failure::check("cargo-test-original-location"))?;
    location.clone_from(replacement);
    Ok(())
}
