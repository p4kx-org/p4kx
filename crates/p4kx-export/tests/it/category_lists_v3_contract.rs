// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::{CatalogPublicationMode, Kind};
use p4kx_testkit::{TestRecord, TestValue};
use serde_json::{Value, json};

use crate::blueprint_category_v3_fixture;
use crate::category_lists_v3_fixture as fixture;
use crate::category_lists_v3_source as source;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn category_lists_v3_preserves_original_slots_and_owns_a_type_wide_appended_kind() -> TestResult {
    let graph = fixture::graph();
    source::ordered(&graph, true)?;
    let (_, catalog) = fixture::diagnostic(&graph)?;
    assert_slots(&catalog)?;
    assert_foreign_source_reference()?;
    let clean = fixture::clean();
    source::ordered(&clean, false)?;
    source::header(
        &clean,
        4,
        fixture::SECOND,
        "BlueprintCategoryDatabaseRecord.Secondary",
        fixture::SECOND_FILE,
        "SecondaryDomain",
    )?;
    let (built, catalog) = fixture::build(
        &clean,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )?;
    fixture::count(&built, 2, 1)?;
    assert_eq!(
        blueprint_category_v3_fixture::rows(&catalog)?.len(),
        2,
        "lists never inflate category definitions"
    );
    assert_eq!(
        fixture::rows(&catalog)?.len(),
        2,
        "type-wide selection includes the list outside the old category directory"
    );
    assert_header(&catalog)?;
    assert_kind_order();
    Ok(())
}

fn assert_foreign_source_reference() -> TestResult {
    let database = fixture::schema()
        .record(fixture::listing(
            fixture::LIST,
            "Primary",
            fixture::LIST_FILE,
            TestValue::Array(vec![TestValue::Reference(fixture::OTHER)]),
        ))
        .record(TestRecord::new(
            fixture::OTHER,
            "Other",
            "Other.Foreign",
            "synthetic/foreign.XML",
        ));
    source::primary(&database)?;
    assert_eq!(
        source::slots(&database, 0)?,
        [source::reference(
            fixture::OTHER,
            1,
            "Other",
            "synthetic/foreign"
        )],
        "the declared typed Reference genuinely resolves to the original foreign physical Record"
    );
    let (_, catalog) = fixture::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )?;
    let row = fixture::row(&catalog, 0)?;
    assert_eq!(
        fixture::at(row, "/categories/value/0/value/status")?,
        &json!("present"),
        "raw list export adds no unapproved category-family health join"
    );
    assert_eq!(
        fixture::at(row, "/categories/value/0/value/targetType/value")?,
        &json!("Other"),
        "a foreign source target is not rewritten as a category"
    );
    assert_eq!(
        fixture::at(&catalog, "/exportHealth/status")?,
        &json!("complete"),
        "a format-valid foreign reference alone is not a fabricated source error"
    );
    Ok(())
}

fn assert_slots(catalog: &Value) -> TestResult {
    let row = fixture::row(catalog, 0)?;
    let values = fixture::at(row, "/categories/value")?
        .as_array()
        .ok_or("ordered category slots")?;
    assert_eq!(
        values.len(),
        6,
        "nulls, repetitions and failures do not disappear"
    );
    for (ordinal, guid, status, target) in [
        (0, fixture::BETA, "present", Some(3)),
        (2, fixture::ALPHA, "present", Some(1)),
        (3, fixture::BETA, "present", Some(3)),
        (4, fixture::MISSING, "dangling", None),
        (5, fixture::OTHER, "present", Some(2)),
    ] {
        assert_reference(
            values.get(ordinal).ok_or("original ordinal")?,
            guid,
            status,
            target,
        )?;
    }
    let null = values.get(1).ok_or("original null ordinal")?;
    assert_eq!(
        fixture::at(null, "/state")?,
        &json!("null"),
        "a null slot is not a dangling or dropped member"
    );
    assert_eq!(
        fixture::at(null, "/value")?,
        &Value::Null,
        "a null slot cannot fabricate a reference"
    );
    assert_eq!(
        fixture::at(row, "/categories/value/5/value/targetType/value")?,
        &json!("Other"),
        "the canonical format reader's foreign target is not relabeled"
    );
    Ok(())
}

fn assert_reference(slot: &Value, guid: u128, status: &str, target: Option<u32>) -> TestResult {
    assert_eq!(
        fixture::at(slot, "/state")?,
        &json!("present"),
        "a retained Reference has its own source state"
    );
    assert_eq!(
        fixture::at(slot, "/value/guid/value")?,
        &json!(common::guid(guid)),
        "authored GUID survives at its own ordinal"
    );
    assert_eq!(
        fixture::at(slot, "/value/status")?,
        &json!(status),
        "actual format resolution remains independent of family/materialization"
    );
    assert_eq!(
        fixture::at(slot, "/value/targetRecordIndex/value")?,
        &json!(target),
        "the original physical target or absence survives"
    );
    Ok(())
}

fn assert_header(catalog: &Value) -> TestResult {
    let row = fixture::row(catalog, 0)?;
    let mut fields = BTreeSet::from(blueprint_category_v3_fixture::FIELDS);
    fields.insert("categories");
    assert_eq!(
        row.as_object()
            .ok_or("list row")?
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        fields,
        "only the shared source header and authored list are exported"
    );
    for (field, expected) in [
        ("guid", json!(common::guid(fixture::LIST))),
        (
            "recordName",
            json!("BlueprintCategoryDatabaseRecord.Primary"),
        ),
        ("className", json!("Primary")),
        (
            "recordPath",
            json!("synthetic/category_lists/included/Primary"),
        ),
        ("recordIndex", json!(0)),
        ("sourceType", json!(fixture::TYPE)),
        ("fileName", json!(fixture::LIST_FILE)),
        ("metadataTag", json!("PrimaryDomain")),
    ] {
        assert_eq!(
            fixture::at(row, &format!("/{field}/value"))?,
            &expected,
            "physical source metadata survives unchanged"
        );
    }
    assert_eq!(
        fixture::at(catalog, "/schemaVersion")?,
        &json!(3),
        "only catalog schema 3 is produced"
    );
    assert!(
        catalog.get(fixture::KIND).is_none(),
        "no flat duplicate list is emitted"
    );
    Ok(())
}

const EXPECTED_KINDS: [&str; 79] = [
    "manufacturers",
    "items",
    "ships",
    "commodities",
    "blueprints",
    "resourceGroups",
    "blueprintCategories",
    "gameplayProperties",
    "qualityDistributions",
    "qualityQuantizations",
    "blueprintPools",
    "dismantleProcesses",
    "paints",
    "loadouts",
    "starmapObjects",
    "contracts",
    "mineableElements",
    "refiningProcesses",
    "mineableCompositions",
    "miningGlobalParams",
    "harvestableProviderPresets",
    "subHarvestableMultiConfigs",
    "harvestableLocations",
    "shops",
    "factionReputations",
    "reputationScopes",
    "reputationStandings",
    "reputationContexts",
    "missionTypes",
    "missionLocalities",
    "contractScenarios",
    "lootTables",
    "lootArchetypes",
    "lootPoolFilters",
    "lootContainers",
    "mineableEntities",
    "legacyLootTables",
    "legacyLootArchetypes",
    "crafters",
    "cargoManifests",
    "missionLocationTemplates",
    "contractShipSpawns",
    "contractLocationQueries",
    "contractTemplates",
    "missionBrokerEntries",
    "contractNpcSpawns",
    "contractEntitySpawns",
    "missionPropertySets",
    "resourceTypes",
    "tags",
    "blueprintCategoryLists",
    "shopInventories",
    "explosionDefinitions",
    "ammoDefinitions",
    "factions",
    "factionIdentities",
    "reputationPerks",
    "reputationTriggerRecords",
    "reputationRewardAmounts",
    "infractions",
    "infractionSets",
    "jurisdictions",
    "locationClasses",
    "locationFunctions",
    "locationSizes",
    "locationClassRegistries",
    "locationFunctionRegistries",
    "locationSizeRegistries",
    "npcArchetypes",
    "npcClassPools",
    "autoSpawns",
    "autoSpawnSets",
    "encounterIndices",
    "inventoryContainers",
    "medicalItemTierConfigs",
    "starmapTypes",
    "starmapPresets",
    "starmapAmenities",
    "missionScenarios",
];

fn assert_kind_order() {
    assert_eq!(
        Kind::ALL.iter().map(|kind| kind.name()).collect::<Vec<_>>(),
        EXPECTED_KINDS,
        "the catalog Kind order preserves the category-list index before shop inventories"
    );
}
