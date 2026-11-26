// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{Loadout, read_loadouts_from};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestPort, entity_schema};

use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::sdk_full_proof as full_proof;

const BASE_PATH: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Prefix.xml";
const PATCH_PATH: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/Prefix.xml";

const BASE: &str = r#"<Vehicle><Parts>
    <Part name="first" class="ItemPort"><ItemPort flags="uneditable"/></Part>
    <Part name="second" class="ItemPort"><ItemPort id="settings" flags="invisible"/></Part>
</Parts><Modifications>
    <Modification name="Protected" patchFile="Modifications/Prefix.xml"/>
</Modifications></Vehicle>"#;

const PATCH: &str =
    r#"<Modifications><Attributes id="settings" flags="uneditable"/></Modifications>"#;

#[test]
fn port_prefix_cache_stays_with_its_physical_vehicle_call() -> TestResult {
    let fixture = fixture()?;
    let markers = read_loadouts_from(
        &fixture.archive,
        &fixture.entry,
        CatalogRecordSelection::All,
        None,
    )?;
    let full = full_proof::from_entry(
        &fixture.archive,
        &fixture.entry,
        full_proof::NO_LANGUAGES,
        None,
    )?
    .read_loadouts(CatalogRecordSelection::All, None)?;
    let ([modified, unmodified], [modified_full, unmodified_full]) =
        (markers.rows.as_slice(), full.rows.as_slice())
    else {
        return Err("two physical callers sharing one base XML".into());
    };
    assert_call(modified, modified_full, 0, true)?;
    assert_call(unmodified, unmodified_full, 1, false)
}

fn assert_call(markers: &Loadout, full: &Loadout, record: u32, encrypted: bool) -> TestResult {
    assert_eq!(markers.record_index(), record, "physical caller identity");
    assert_eq!(
        full.record_index(),
        record,
        "the same physical caller in Full"
    );
    let ([mn, mp], [fn_, fp]) = (markers.ports(), full.ports()) else {
        return Err("two ports in deliberate reverse Part order".into());
    };
    for (name, marked, proved) in [("second", mn, fn_), ("first", mp, fp)] {
        assert_eq!(
            marked.name().value().map(String::as_str),
            Some(name),
            "Record port order"
        );
        let marked = marked.source_visibility();
        let proved = proved.source_visibility();
        assert_eq!(
            marked.value(),
            proved.value(),
            "interpreted visibility for {name}"
        );
        assert_eq!(
            marked.state(),
            proved.state(),
            "source-value state for {name}"
        );
        assert_eq!(
            marked.origin().contains_encrypted_source(),
            Some(encrypted),
            "only the actual caller's selected documents set the marker for {name}"
        );
        assert_eq!(
            marked.origin().contains_encrypted_source(),
            proved.origin().contains_encrypted_source(),
            "Full and Markers read the same physical prefix for {name}"
        );
        assert_eq!(
            marked.origin().source_set_complete(),
            proved.origin().source_set_complete(),
            "original completeness for {name}"
        );
        assert!(
            !proved.origin().source_uses_refs().is_empty(),
            "Full retains exact sources"
        );
        assert!(
            marked.origin().source_uses_refs().is_empty(),
            "marker profile excludes source references"
        );
    }
    Ok(())
}

fn fixture() -> TestResult<Fixture> {
    let mut builder = entity_schema(DataForgeBuilder::new());
    for (guid, modification) in [(1, "Protected"), (2, "")] {
        builder = builder.record(
            TestEntity::new(
                guid,
                format!("Prefix{guid}"),
                format!("libs/foundry/records/entities/spaceships/prefix{guid}.xml"),
            )
            .vehicle(BASE_PATH, modification)
            .port(TestPort::new("second"))
            .port(TestPort::new("first"))
            .record(),
        );
    }
    Fixture::open(
        &builder.build(),
        Input::Plain,
        vec![
            TestEntry::stored(format!("Data/{BASE_PATH}"), BASE),
            TestEntry::stored(format!("Data/{PATCH_PATH}"), PATCH).encrypted(),
        ],
    )
}
