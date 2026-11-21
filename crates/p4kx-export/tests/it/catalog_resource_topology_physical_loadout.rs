// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Filter;
use serde_json::{Value, json};

use super::{bindings, catalog_from, catalog_with_filter, common, fixture, proof};
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};

#[test]
fn loadout_positions_keep_the_physical_root_after_filtering_a_same_guid_twin() -> TestResult {
    let database = fixture::physical_identity_database();
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let all = catalog_from(&archive)?;
    let filter = Filter::new(fixture::SELECTED_TWIN);
    let selected = catalog_with_filter(&archive, Some(&filter))?;

    let all_loadouts = proof::rows(&all, "/loadouts")?;
    let [earlier_item, removed, kept] = all_loadouts else {
        return Err(
            "the full catalog includes the earlier Item and both physical Ship roots".into(),
        );
    };
    assert_eq!(
        proof::at(earlier_item, "/entityKind/value")?,
        "item",
        "the earlier Item contributes to the full Loadout array order"
    );
    let guid = json!(common::guid(fixture::SHIP));
    check_twin(&all, removed, &guid, 1, "removed")?;
    check_twin(&all, kept, &guid, 2, "kept")?;

    let [selected_loadout] = proof::rows(&selected, "/loadouts")? else {
        return Err("the filtered catalog retains only the selected physical Ship root".into());
    };
    check_twin(&selected, selected_loadout, &guid, 2, "kept")?;
    assert_eq!(
        bindings(&selected, 2)?,
        bindings(&all, 2)?,
        "the selected root's topology and source cells survive a changed array position"
    );
    proof::closure(&all)?;
    proof::closure(&selected)
}

fn check_twin(
    catalog: &Value,
    loadout: &Value,
    guid: &Value,
    record: u32,
    first_port: &str,
) -> TestResult {
    proof::present(loadout, "entityGuid", guid, Some(true))?;
    let index = proof::present(loadout, "sourceRecordIndex", &json!(record), Some(true))?;
    proof::header(
        catalog,
        index,
        record,
        "recordIndex",
        "selector",
        "archiveEntry",
    )?;
    assert_eq!(
        proof::at(loadout, "/ports/0/name/value")?,
        first_port,
        "the port belongs to this physical twin"
    );
    assert_eq!(
        proof::at(loadout, "/ports/1/name/value")?,
        "second",
        "the authored port order remains the route order"
    );
    let links = bindings(catalog, u64::from(record))?;
    let [link] = links.as_slice() else {
        return Err("each physical twin has one root hardpoint link".into());
    };
    assert_eq!(
        proof::at(link, "/context")?,
        &json!({"loadoutRecordIndex": record, "portOrdinals": []}),
        "the root address uses the physical Record index and an empty port path"
    );
    for (endpoint, ordinal) in [("port1", 0), ("port2", 1)] {
        assert_eq!(
            proof::at(
                link,
                &format!("/{endpoint}/value/candidate/loadoutPortOrdinal")
            )?,
            &json!(ordinal),
            "each local port ordinal still indexes its declaring context in authored order"
        );
    }
    Ok(())
}

#[test]
fn schema_requires_the_physical_loadout_record_index() -> TestResult {
    let schema = serde_json::to_value(p4kx_export::catalog_schema())?;
    let position = proof::at(&schema, "/$defs/LoadoutPosition")?;
    let validator = jsonschema::validator_for(position)?;
    assert!(
        validator.is_valid(&json!({"loadoutRecordIndex": 2, "portOrdinals": [1, 0]})),
        "the required position uses a physical root and ordered local ordinals"
    );
    assert!(
        !validator.is_valid(&json!({"loadoutOrdinal": 2, "portOrdinals": [1, 0]})),
        "an array position alone cannot satisfy the physical reference contract"
    );
    Ok(())
}
