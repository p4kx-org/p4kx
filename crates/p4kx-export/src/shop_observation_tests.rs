// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::ArchiveFiles;
use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use crate::catalog_crafting::CraftingKinds;
use crate::catalog_read::original::{EntryCapture, FailedEntryObservation};
use crate::collected::Collected;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_containers::Component;
use crate::shop_inventories::Inventories;
use crate::shops::{self, REGISTRY};
use crate::{CatalogError, CatalogOptions, Kind, ShopSummary};

#[path = "shop_inventory_test_source.rs"]
mod inventory_source;

#[path = "shop_inventory_observation_tests.rs"]
mod inventories;

#[path = "shop_registry_observation_tests.rs"]
mod registry_observations;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ENTITY: &str = "4dce4077-9730-ac77-5074-916b3b2700b7";
const ROOT: &str = "4a269932-183e-362d-aacc-188e7debbea9";
const CONTAINER: &str = "Data/ObjectContainers/PU/Shops/admin/test.socpak";

fn read_shops(archive: &Archive, capture: &EntryCapture<'_>) -> Result<shops::Built, CatalogError> {
    let mut inventories = Inventories::prepare(archive, capture, None)?.inventories;
    shops::shops(
        archive,
        &ArchiveFiles::new(archive),
        (&[], true),
        None,
        capture,
        &mut inventories,
    )
}

fn opened(
    entries: Vec<TestEntry>,
) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let builder = entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry);
    builder.write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

fn registry(extra: &str) -> String {
    format!(
        r#"<BackupData><LayerBackups><LayerBackup PlatformName="StantonSystem"><ShopBackups>
        <ShopBackup ID="{ROOT},{ENTITY}" SuperName="Root,SCShop_Test"/>{extra}
        </ShopBackups></LayerBackup></LayerBackups></BackupData>"#
    )
}

fn corrupt_zstd(path: &str) -> TestEntry {
    let entry = TestEntry::zstd(path, "expected source bytes");
    let payload = entry
        .payload()
        .get(..4)
        .expect("the testkit writes a complete Zstd magic")
        .to_vec();
    entry.with_payload(payload)
}

fn empty_collected() -> Collected<'static> {
    Collected::from_crafting(CraftingKinds {
        blueprints: Vec::new(),
        properties: Vec::new(),
        quality: crate::quality_catalog::QualityRows::default(),
        pools: Vec::new(),
        reward_declarations: Vec::new(),
        reward_numbers: Vec::new(),
        dismantle: Vec::new(),
        skipped: Vec::new(),
        failures: Vec::new(),
        warnings: Vec::new(),
        ledger: ExportLedger::default(),
        global: Vec::new(),
    })
}

#[test]
fn shop_placements_share_their_item_list_and_emit_the_same_guids() -> TestResult {
    let other_root = "4b269932-183e-362d-aacc-188e7debbea9";
    let item = "70a98c94-9252-4fd3-a673-5a584339e273";
    let extra =
        format!(r#"<ShopBackup ID="{other_root},{ENTITY}" SuperName="OtherRoot,SCShop_Test"/>"#);
    let prefab = r#"<PrefabLibrary><Object Type="Entity" Id="{9730AC77-4077-4DCE-B700-273B6B917450}">
        <PropertiesDataCore><EntityComponentShop offlineInventoryJSON="Scripts/ShopInventories/shared.json"/>
        </PropertiesDataCore></Object></PrefabLibrary>"#;
    let (_folder, archive) = opened(vec![
        TestEntry::stored(REGISTRY, registry(&extra)),
        TestEntry::stored("Data/Prefabs/Shops/shared.xml", prefab),
        TestEntry::stored(
            "Data/Scripts/ShopInventories/shared.json",
            format!(r#"{{"Inventory":[{{"ID":{{"ID":["{item}"]}}}}]}}"#),
        ),
    ])?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        read_shops(&archive, capture)
    })?;
    let [first, second] = read.shops.as_slice() else {
        return Err("two physical shop placements".into());
    };
    let (Some(first_guids), Some(second_guids)) =
        (first.item_guids.value(), second.item_guids.value())
    else {
        return Err("both shops have a resolved inventory".into());
    };
    assert!(
        Arc::ptr_eq(&first_guids.0, &second_guids.0),
        "placements sharing an inventory reuse its in-memory list"
    );
    for shop in &read.shops {
        let wire = serde_json::to_value(shop)?;
        assert_eq!(
            wire.pointer("/itemGuids/value/0/value"),
            Some(&serde_json::json!(item)),
            "both placements retain the original inventory item",
        );
    }
    Ok(())
}

#[test]
fn editor_and_soc_failures_keep_exact_nested_identities_and_valid_components() -> TestResult {
    let metadata = format!(r#"<Entities><Entity CryGUID="{ENTITY}"/></Entities>"#);
    let soc = format!(
        "CrChF\0{ENTITY}\0EntityComponentShop\0offlineInventoryJSON\0Scripts/ShopInventories/good.json\0"
    );
    let container = TestContainer::new()
        .entry(TestEntry::stored("padding.txt", "ignored"))
        .entry(TestEntry::stored(
            "test/metadata/entitycomponentshop.xml",
            metadata,
        ))
        .entry(TestEntry::stored("test_editor.xml", "malformed XML"))
        .entry(corrupt_zstd("bad.soc"))
        .entry(TestEntry::stored("good.soc", soc));
    let (_folder, archive) = opened(vec![
        TestEntry::stored("padding.txt", "ignored"),
        TestEntry::stored(CONTAINER, container.build()),
    ])?;
    let source = archive.entries().get(1).ok_or("outer source")?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        crate::shop_container_scan::shop_entities(&archive, source, None, capture)
    })?;
    assert!(
        read.ledger.incomplete_inventories.contains(&Kind::Shops),
        "failed editor/SOC reads can hide inventory dependencies"
    );
    let (entities, _) = read.entities.ok_or("retained components")?;
    assert_eq!(
        entities,
        [(
            ENTITY.to_owned(),
            Component::File(crate::shop_inventory_path::InventoryStem::accounting(
                "good"
            ))
        )],
        "the valid SOC is retained despite separate failures"
    );
    let entries = crate::export_health_entries::compile(read.ledger.entries)?;
    assert_eq!(
        entries.sources.len(),
        5,
        "outer directory, metadata, editor and the two consumed SOC Entries"
    );
    assert_eq!(
        entries.failed.len(),
        2,
        "each failing physical operation is retained"
    );
    assert_failed_identities(&entries.failed);
    let counts = crate::export_health_entries::counts(Kind::Shops, &entries.sources)?;
    assert_eq!(
        (counts.dependencies, counts.failed_dependencies),
        (5, 2),
        "operations reduce to the actual physical Entry units"
    );
    Ok(())
}

fn assert_failed_identities(failed: &[FailedEntryObservation]) {
    let identities: Vec<_> = failed
        .iter()
        .map(|source| {
            (
                source.source.operation,
                source.source.entry_index,
                source.source.container_entry_index,
            )
        })
        .collect();
    assert_eq!(
        identities,
        [
            (EntryOperation::ShopEditor, Some(1), Some(2)),
            (EntryOperation::ShopSoc, Some(1), Some(3)),
        ],
        "all failed operations retain their exact outer and nested physical identities"
    );
}

#[test]
fn unreadable_metadata_does_not_invent_a_soc_or_editor_read() -> TestResult {
    let container = TestContainer::new()
        .entry(TestEntry::stored(
            "metadata/entitycomponentshop.xml",
            "malformed XML",
        ))
        .entry(TestEntry::stored("test_editor.xml", "unused malformed XML"))
        .entry(TestEntry::stored("unused.soc", "unused bytes"));
    let (_folder, archive) = opened(vec![TestEntry::stored(CONTAINER, container.build())])?;
    let container = archive.entries().first().ok_or("container")?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        crate::shop_container_scan::shop_entities(&archive, container, None, capture)
    })?;
    assert!(
        read.entities.is_none(),
        "the selected metadata projection failed"
    );
    assert_eq!(
        read.ledger.entries.len(),
        2,
        "only the outer directory and metadata were consumed"
    );
    assert!(
        read.ledger
            .entries
            .iter()
            .any(|source| source.operation == EntryOperation::ShopMetadata
                && source.disposition == SourceDisposition::ProjectionFailed),
        "actual XML failure remains visible"
    );
    Ok(())
}

#[test]
fn a_filter_finishes_an_empty_shop_inventory_without_reading_shop_sources() -> TestResult {
    let (_folder, archive) = opened(vec![
        TestEntry::stored(REGISTRY, "bad XML"),
        TestEntry::stored(CONTAINER, "bad container"),
    ])?;
    let mut collected = empty_collected();
    let filter = Filter::new("*/items/*");
    let options = CatalogOptions {
        filter: Some(&filter),
        ..CatalogOptions::new(std::time::UNIX_EPOCH)
    };
    let summary = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        shops::add_shops(
            &archive,
            &ArchiveFiles::new(&archive),
            &mut collected,
            (
                &options,
                capture,
                &crate::starmap_containers::Placements::default(),
            ),
        )
    })?;
    assert_eq!(
        summary,
        ShopSummary::default(),
        "filtered shops have no invented source counts"
    );
    assert_eq!(
        collected.ledger.inventories,
        [Kind::ShopInventories, Kind::Shops],
        "the global files and filtered shop registry each finish their selection"
    );
    assert_eq!(
        collected.ledger.entries,
        Vec::new(),
        "excluded registry and containers were not read"
    );
    assert_eq!(
        collected.ledger.incomplete_inventories,
        Vec::new(),
        "an excluded kind has no attempted failed walk"
    );
    Ok(())
}
