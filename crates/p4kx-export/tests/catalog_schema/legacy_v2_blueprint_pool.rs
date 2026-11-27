// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::Value;

use super::{Inventory, Result, family_fields, released_schema, schema_at};

const FAMILY: &str = "blueprintPools";
const POOL: &str = "/definitions/blueprintPools/rows/[]";
const WEIGHT: &str = "/projections/blueprintPoolWeights/rows/[]";
const REWARDS: &str = "/declarations/blueprintRewardSources";
const REWARD: &str = "/declarations/blueprintRewardSources/rows/[]";

#[test]
fn every_released_blueprint_pool_field_has_one_canonical_destination() -> Result {
    let old = released_schema()?;
    let inventory = Inventory::collect(&old)?;
    let current = p4kx_export::catalog_schema();
    assert!(
        current["properties"].get(FAMILY).is_none(),
        "the old aggregate is published through independent v3 datasets"
    );

    let mappings = [
        ("/className", POOL, "/className/value"),
        ("/guid", POOL, "/guid/value"),
        ("/recordPath", POOL, "/recordPath/value"),
        ("/entries", POOL, "/blueprintRewards/value"),
        (
            "/entries/[]/blueprintGuid",
            POOL,
            "/blueprintRewards/value/[]/value/blueprintRecord/value/guid/value",
        ),
        (
            "/entries/[]/weight",
            POOL,
            "/blueprintRewards/value/[]/value/weight/value",
        ),
        ("/totalWeight", WEIGHT, "/totalWeight/value"),
        ("/rewardSources", REWARDS, "/rows"),
        ("/rewardSources/[]/recordGuid", REWARD, "/guid/value"),
        ("/rewardSources/[]/recordPath", REWARD, "/recordPath/value"),
        ("/rewardSources/[]/contractId", REWARD, "/contractId/value"),
        (
            "/rewardSources/[]/contractName",
            REWARD,
            "/contractName/value",
        ),
        ("/rewardSources/[]/chance", REWARD, "/chance/value"),
    ];

    let mut classified = BTreeSet::new();
    for (released, owner, field) in mappings {
        assert!(
            classified.insert(released),
            "each released field maps once: {released}"
        );
        let path = format!("{owner}{field}");
        schema_at(&current, &path)
            .map_err(|error| format!("{FAMILY}{released} -> {path}: {error}"))?;
    }
    assert_eq!(
        family_fields(&inventory, FAMILY)?,
        classified,
        "every released pool field needs an explicit migration route"
    );
    verify_types_and_physical_identity(&current)
}

fn verify_types_and_physical_identity(current: &Value) -> Result {
    for (owner, field, definition) in [
        (POOL, "/guid/value", "Guid"),
        (
            POOL,
            "/blueprintRewards/value/[]/value/blueprintRecord/value",
            "CatalogRecordReference",
        ),
        (
            POOL,
            "/blueprintRewards/value/[]/value/weight/value",
            "ResourceNumber",
        ),
        (WEIGHT, "/totalWeight/value", "ResourceFloat32"),
        (REWARD, "/pool/value", "CatalogRecordReference"),
        (REWARD, "/chance/value", "ResourceNumber"),
    ] {
        let path = format!("{owner}{field}");
        let expected = current
            .pointer(&format!("/$defs/{definition}"))
            .ok_or("expected canonical pool field type")?;
        assert_eq!(
            schema_at(current, &path)?,
            expected,
            "{path} preserves {definition}"
        );
    }

    for (owner, field) in [
        (POOL, "/recordIndex/value"),
        (WEIGHT, "/pool/inputRef"),
        (WEIGHT, "/pool/recordIndex/value"),
        (REWARD, "/recordIndex/value"),
        (REWARD, "/sourceOrdinal/value"),
        (REWARD, "/pool/value/targetRecordIndex/value"),
        (REWARD, "/poolStorage/value"),
    ] {
        schema_at(current, &format!("{owner}{field}"))?;
    }
    Ok(())
}
