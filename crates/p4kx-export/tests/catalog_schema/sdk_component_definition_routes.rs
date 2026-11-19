// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::component_definitions::{
    ComponentDefinitionSnapshot, with_consumable_subtypes, with_consumable_types,
    with_damage_resistance_macros, with_mining_laser_globals, with_turret_health_modifiers,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::{Route, TestResult};

const REMAINING: &str =
    "selective query and typed schema witnessed; installed and full producer parity open";
const TYPES: [&str; 5] = [
    "DamageResistanceMacro",
    "MiningLaserGlobalParams",
    "STurretHealthModifierDef",
    "ConsumableType",
    "ConsumableSubtype",
];

pub(super) fn read() -> TestResult<Vec<Route>> {
    let (_folder, archive) = fixture()?;
    let all = CatalogRecordSelection::All;
    Ok(vec![
        with_damage_resistance_macros(&archive, all, None, |rows| {
            witness("/definitions/damageResistanceMacros", &rows)
        })??,
        with_mining_laser_globals(&archive, all, None, |rows| {
            witness("/definitions/miningLaserGlobals", &rows)
        })??,
        with_turret_health_modifiers(&archive, all, None, |rows| {
            witness("/definitions/turretHealthModifiers", &rows)
        })??,
        with_consumable_types(&archive, all, None, |rows| {
            witness("/definitions/consumableTypes", &rows)
        })??,
        with_consumable_subtypes(
            &archive,
            all,
            CatalogLanguageSelection::Selected(&[]),
            None,
            |rows| witness("/definitions/consumableSubtypes", &rows),
        )??,
    ])
}

fn witness<Row: JsonSchema + Serialize>(
    dataset: &'static str,
    snapshot: &ComponentDefinitionSnapshot<'_, Row>,
) -> TestResult<Route> {
    assert!(
        snapshot.coverage().unreadable_headers.is_empty()
            && snapshot.coverage().unreadable_bodies.is_empty(),
        "{dataset} retains its readable source scope"
    );
    assert!(
        snapshot.definition_discovery().is_complete(),
        "the fixture dependency scope is complete"
    );
    Route::queried_rows(dataset, snapshot.rows(), REMAINING)
}

#[test]
fn component_definition_queries_keep_physical_aliases_and_language_scope() -> TestResult<()> {
    let (_folder, archive) = fixture()?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_damage_resistance_macros(CatalogRecordSelection::All, None, |snapshot| {
        let rows = snapshot.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].source().guid().value(),
            rows[1].source().guid().value()
        );
        assert_ne!(
            rows[0].source().record_index().value(),
            rows[1].source().record_index().value()
        );
        assert_eq!(
            snapshot.requested_languages(),
            [],
            "the query needs no locale input"
        );
        assert_eq!(
            snapshot.available_languages(),
            [],
            "the query opened no locale payload"
        );
    })?;
    parsed.with_consumable_subtypes(
        CatalogRecordSelection::RecordIndex(u32::MAX),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                0,
                "the absent physical index selects no row"
            );
            assert_eq!(snapshot.requested_languages(), &[Language::English]);
            assert_eq!(
                snapshot.available_languages(),
                [],
                "no source owner needs the requested language"
            );
        },
    )?;
    parsed.with_consumable_subtypes(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(snapshot.available_languages(), &[Language::English]);
        assert_eq!(snapshot.public_names().len(), 1);
        assert!(
            snapshot.source_inventory().iter_uses().next().is_some(),
            "selected values retain source uses"
        );
    })?;
    Ok(())
}

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let mut data = DataForgeBuilder::new().version(8);
    for (index, (source_type, id)) in TYPES.into_iter().zip(1..=5_u128).enumerate() {
        let mut definition = TestStruct::new(source_type);
        let mut record = TestRecord::new(
            id,
            source_type,
            source_type,
            format!("libs/foundry/records/fixtures/definition_{index}.xml"),
        );
        if source_type == "ConsumableSubtype" {
            definition = definition.field("consumableName", TestType::Locale);
            record = record.value("consumableName", TestValue::text("@fixture_consumable"));
        }
        data = data.structure(definition).record(record);
    }
    data = data.record(TestRecord::new(
        1,
        TYPES[0],
        "FixtureMacroAlias",
        "libs/foundry/records/fixtures/alias.xml",
    ));
    let archive = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", data.build()))
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"fixture_consumable=Fixture\n",
        ))
        .write_to(&archive)?;
    Ok((folder, Archive::open(archive)?))
}
