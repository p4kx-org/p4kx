// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

#[path = "catalog_auto_spawn_definitions.rs"]
mod auto_spawn_definitions;
#[path = "catalog_auto_spawn_fixture.rs"]
mod auto_spawn_fixture;
#[path = "catalog_npc_spawn_cycles.rs"]
mod cycles;

#[path = "catalog_npc_spawn_fixture.rs"]
mod npc_fixture;

use npc_fixture::{ARCHETYPE, MISSING, POOL, database};

#[test]
fn npc_definitions_keep_weighted_occurrences_and_provenance() -> TestResult {
    let catalog = export(None)?;
    let pool = at(&catalog, "/definitions/npcClassPools/rows/0")?;
    assert_eq!(at(pool, "/guid/value")?, &json!(guid(POOL)));
    assert_eq!(
        at(pool, "/npcClasses/value/0/value/weight/value/rawBits")?,
        "80000000",
        "negative zero retains its exact Single bits"
    );
    assert_eq!(
        at(pool, "/npcClasses/value/1/value/weight/value/rawBits")?,
        "3fa00000",
        "the second occurrence retains its own weight"
    );
    for (path, expected) in [
        ("/npcClasses/value/0/value/npcClass/state", "null"),
        (
            "/npcClasses/value/1/value/npcClass/value/status",
            "dangling",
        ),
    ] {
        assert_eq!(at(pool, path)?, expected);
    }
    assert_eq!(
        at(pool, "/npcClasses/value/1/value/npcClass/value/guid/value")?,
        &json!(guid(MISSING))
    );
    for path in [
        "/npcClasses/value/0/value/weight/containsEncryptedSource",
        "/npcClasses/value/1/value/npcClass/value/guid/containsEncryptedSource",
    ] {
        assert_eq!(at(pool, path)?, true);
    }
    assert_eq!(
        at(
            &catalog,
            "/definitions/npcClassPools/rows/1/npcClasses/value"
        )?,
        &json!([]),
        "the independent empty pool survives"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {}", error.instance_path(), error).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

#[test]
fn npc_archetype_reuses_pool_identity_across_ordered_branches() -> TestResult {
    let filter = Filter::new("*/npc_fixture/archetypes/*");
    let catalog = export(Some(&filter))?;
    let archetype = at(&catalog, "/definitions/npcArchetypes/rows/0")?;
    assert_eq!(at(archetype, "/guid/value")?, &json!(guid(ARCHETYPE)));
    assert_eq!(at(&catalog, "/definitions/npcClassPools/rows")?, &json!([]));
    for path in [
        "/identity/value/0/value/npcPools/value/0",
        "/identity/value/0/value/npcPools/value/1",
        "/identity/value/1/value/npcPools/value/0",
    ] {
        assert_eq!(
            at(at(archetype, path)?, "/value/guid/value")?,
            &json!(guid(POOL)),
            "each authored occurrence retains the shared pool GUID"
        );
    }
    assert_eq!(
        at(archetype, "/identity/value/0/value/identity/state")?,
        "null"
    );
    assert_eq!(
        at(archetype, "/identity/value/1/value/identity/value/status")?,
        "dangling"
    );
    let references = at(&catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("unresolved reference rows")?;
    assert!(references.iter().any(|row| {
        row["status"] == "excludedByFilter" && row["targetGuid"]["value"] == guid(POOL)
    }));
    Ok(())
}

fn export(filter: Option<&Filter>) -> TestResult<Value> {
    let (_directory, archive) = common::open(&database(), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.filter = filter;
    let built = build_catalog(&archive, &options).map_err(|error| error.to_string())?;
    Ok(serde_json::from_slice(built.json())?)
}
