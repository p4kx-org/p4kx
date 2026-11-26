// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::mission_definitions::{
    MissionDefinitionFamily, MissionDefinitionRows, MissionDefinitionSelection,
    MissionDefinitionSnapshot, MissionItemDefinition, MissionModuleDeclaration,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogOptions, Language};
use p4kx_extraction::{Archive, DataForgeEntry, InputScope, PhysicalSourceKind};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde::Serialize;
use serde_json::Value;

use super::{Route, TestResult};

const SCENARIOS: &str = "/definitions/missionScenarios";
const MODULES: &str = "/definitions/missionModuleDeclarations";
const ITEMS: &str = "/definitions/missionItems";
const ALTERNATIVES: &str = "MissionStringVariants";
const ALTERNATIVE: &str = "MissionStringVariant";
const MODULE_BASE: &str = "ModuleDeclarationType_Base";
const MODULE_LOCATION: &str = "ModuleDeclarationType_Location";
const ITEM_KEY: &str = "FixtureMissionItem";

pub(super) fn read() -> TestResult<[Route; 3]> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database().build()).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"FixtureMissionItem=Original mission item\n",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        p4kx_export::build_catalog_from(
            &archive,
            &entry,
            CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
        )?
        .json(),
    )?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?
    .with_provenance_mode(full_proof::MODE);
    let parsed = session.parsed(None)?;
    let read = |family| {
        parsed.with_mission_definitions(
            MissionDefinitionSelection::new(family, CatalogRecordSelection::All),
            None,
            |snapshot| witness(&catalog, &snapshot),
        )
    };
    parsed.with_mission_definitions(
        MissionDefinitionSelection::new(
            MissionDefinitionFamily::Modules,
            CatalogRecordSelection::RecordIndex(0),
        ),
        None,
        assert_wrong_owner,
    )??;
    Ok([
        read(MissionDefinitionFamily::Scenarios)??,
        read(MissionDefinitionFamily::Modules)??,
        read(MissionDefinitionFamily::Items)??,
    ])
}

fn assert_wrong_owner(snapshot: MissionDefinitionSnapshot<'_>) -> TestResult {
    let MissionDefinitionRows::Modules(rows) = snapshot.rows else {
        return Err("the selected module family".into());
    };
    assert!(
        rows.is_empty(),
        "an index in another family admits no module"
    );
    assert_no_locale(&snapshot.source_inventory);
    Ok(())
}

fn witness(catalog: &Value, snapshot: &MissionDefinitionSnapshot<'_>) -> TestResult<Route> {
    match &snapshot.rows {
        MissionDefinitionRows::Scenarios(rows) => {
            let [row] = rows.as_slice() else {
                return Err("one physical mission scenario".into());
            };
            assert_eq!(
                row.header().record_index().value(),
                Some(&0),
                "physical scenario index"
            );
            assert!(
                !row.authored_fields().roots().is_empty(),
                "raw scenario member is available"
            );
            assert_no_locale(&snapshot.source_inventory);
            parity(catalog, SCENARIOS, rows)
        }
        MissionDefinitionRows::Modules(rows) => {
            let [row] = rows.as_slice() else {
                return Err("one physical module declaration".into());
            };
            check_module(row, &snapshot.source_inventory)?;
            parity(catalog, MODULES, rows)
        }
        MissionDefinitionRows::Items(rows) => {
            let [row] = rows.as_slice() else {
                return Err("one physical mission item".into());
            };
            check_item(row, snapshot, catalog)?;
            parity(catalog, ITEMS, rows)
        }
        _ => Err("unexpected original mission definition family".into()),
    }
}

fn check_module(
    row: &MissionModuleDeclaration<'_>,
    inventory: &p4kx_extraction::SourceInventory,
) -> TestResult {
    let module = row.module();
    assert_eq!(
        module.value().copied(),
        Some("FixtureModule"),
        "authored module name"
    );
    assert_eq!(
        module.origin().contains_encrypted_source(),
        Some(true),
        "encrypted owner"
    );
    assert!(
        module.origin().source_set_complete()
            && inventory.iter_uses().any(|used| {
                used.scope() == InputScope::Payload
                    && module.origin().source_uses_refs().contains(used.id())
            }),
        "the module name resolves in its own exact source inventory"
    );
    let location = row.module_type().value().ok_or("authored module type")?;
    let persistence = location
        .is_persistent()
        .ok_or("authored location persistence")?;
    assert_eq!(persistence.value(), Some(&true), "typed location variant");
    assert_no_locale(inventory);
    Ok(())
}

fn check_item(
    row: &MissionItemDefinition<'_>,
    snapshot: &MissionDefinitionSnapshot<'_>,
    catalog: &Value,
) -> TestResult {
    let alternatives = row
        .string_variants()
        .value()
        .and_then(|value| value.variants().value())
        .ok_or("authored mission item alternatives")?;
    let [alternative] = alternatives.as_slice() else {
        return Err("one original text alternative".into());
    };
    let reference = alternative
        .value()
        .and_then(|row| row.string().value())
        .ok_or("protected reference")?;
    let resolved = snapshot
        .game_text(reference)
        .ok_or("the selected protected text")?;
    assert_eq!(
        resolved.key().value().copied(),
        Some(ITEM_KEY),
        "authored localization key"
    );
    assert_eq!(
        catalog.pointer("/texts/gameTexts"),
        Some(&serde_json::to_value(&snapshot.game_texts)?),
        "the selected protected texts match the canonical text table"
    );
    assert_eq!(
        snapshot.available_languages,
        [Language::English],
        "mission items open the requested available locale"
    );
    Ok(())
}

fn assert_no_locale(inventory: &p4kx_extraction::SourceInventory) {
    let locale = Language::English.global_ini_path();
    assert!(
        inventory.iter_sources().all(|source| match source.kind() {
            PhysicalSourceKind::ArchiveEntry(entry) => entry
                .path()
                .value()
                .is_none_or(|path| path.as_str() != locale),
            _ => true,
        }),
        "a nonlocalized family leaves the installed locale payload outside its result"
    );
}

fn parity<Row: schemars::JsonSchema + Serialize>(
    catalog: &Value,
    path: &'static str,
    rows: &[Row],
) -> TestResult<Route> {
    let expected = catalog
        .pointer(&format!("{path}/rows"))
        .ok_or("canonical typed rows")?;
    assert_eq!(
        &serde_json::to_value(rows)?,
        expected,
        "exact producer parity for {path}"
    );
    Route::queried_rows(path, rows, None)
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("MissionScenario").field("track_progress", TestType::Bool))
        .structure(TestStruct::new(MODULE_BASE))
        .structure(
            TestStruct::new(MODULE_LOCATION)
                .parent(MODULE_BASE)
                .field("isPersistent", TestType::Bool),
        )
        .structure(
            TestStruct::new("ModuleDeclaration")
                .field("module", TestType::String)
                .field(
                    "moduleType",
                    TestType::StrongPointer(MODULE_BASE.to_owned()),
                ),
        )
        .structure(TestStruct::new(ALTERNATIVE).field("string", TestType::Locale))
        .structure(
            TestStruct::new(ALTERNATIVES)
                .array("variants", TestType::Class(ALTERNATIVE.to_owned())),
        )
        .structure(
            TestStruct::new("MissionItem")
                .field("weighting", TestType::Single)
                .field("stringVariants", TestType::Class(ALTERNATIVES.to_owned())),
        )
        .record(
            TestRecord::new(
                1,
                "MissionScenario",
                "MissionScenario.Fixture",
                "libs/foundry/records/fixture/scenario.xml",
            )
            .value("track_progress", TestValue::Bool(false)),
        )
        .record(module_record())
        .record(item_record())
}

fn module_record() -> TestRecord {
    let location = TestInstance::new(MODULE_LOCATION).value("isPersistent", TestValue::Bool(true));
    TestRecord::new(
        2,
        "ModuleDeclaration",
        "ModuleDeclaration.Fixture",
        "libs/foundry/records/fixture/module.xml",
    )
    .value("module", TestValue::text("FixtureModule"))
    .value("moduleType", TestValue::Struct(location))
}

fn item_record() -> TestRecord {
    let alternative = TestInstance::new(ALTERNATIVE).value("string", TestValue::text(ITEM_KEY));
    let alternatives = TestInstance::new(ALTERNATIVES).value(
        "variants",
        TestValue::Array(vec![TestValue::Struct(alternative)]),
    );
    TestRecord::new(
        3,
        "MissionItem",
        "MissionItem.Fixture",
        "libs/foundry/records/fixture/item.xml",
    )
    .value("weighting", TestValue::Single(1.0))
    .value("stringVariants", TestValue::Struct(alternatives))
}
