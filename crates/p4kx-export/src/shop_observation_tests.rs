// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ArchiveFiles;
use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use crate::catalog_crafting::CraftingKinds;
use crate::catalog_read::original::{EntryCapture, FailedEntryObservation};
use crate::collected::Collected;
use crate::contract::CraftingGlobal;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_containers::Component;
use crate::shop_inventories::Inventories;
use crate::shop_registry::RegistryNeed;
use crate::shops::{self, REGISTRY};
use crate::{CatalogError, CatalogOptions, Kind, ShopSummary};

#[path = "shop_inventory_test_source.rs"]
mod inventory_source;

#[path = "shop_inventory_observation_tests.rs"]
mod inventories;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ENTITY: &str = "4dce4077-9730-ac77-5074-916b3b2700b7";
const ROOT: &str = "4a269932-183e-362d-aacc-188e7debbea9";
const CONTAINER: &str = "Data/ObjectContainers/PU/Shops/admin/test.socpak";

fn read_shops(archive: &Archive, capture: &EntryCapture<'_>) -> Result<shops::Built, CatalogError> {
    let mut inventories = Inventories::prepare(archive, capture, None)?.inventories;
    shops::shops(
        archive,
        &ArchiveFiles::new(archive),
        &[],
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
    Collected::new(
        Vec::new(),
        CraftingKinds {
            blueprints: Vec::new(),
            properties: Vec::new(),
            quality: crate::quality_catalog::QualityRows::default(),
            pools: Vec::new(),
            dismantle: Vec::new(),
            skipped: Vec::new(),
            failures: Vec::new(),
            warnings: Vec::new(),
            ledger: ExportLedger::default(),
            global: CraftingGlobal {
                dismantle_excluded_resource_guids: Vec::new(),
                dismantle_excluded_item_guids: Vec::new(),
            },
        },
    )
}

#[test]
fn an_absent_registry_without_starmap_is_explicitly_optional() -> TestResult {
    let (_folder, archive) = opened(Vec::new())?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        read_shops(&archive, capture)
    })?;
    assert_eq!(
        read.ledger.inventories,
        [Kind::Shops],
        "the source inventory completed"
    );
    assert_eq!(
        read.ledger.incomplete_inventories,
        Vec::new(),
        "optional absence hides no required inputs"
    );
    let [source] = read.ledger.entries.as_slice() else {
        return Err("one optional registry observation".into());
    };
    assert_eq!(source.entry_index, None, "no physical Entry is invented");
    assert_eq!(
        source.disposition,
        SourceDisposition::SkippedBeforeRead,
        "absence is not a blocked read"
    );
    assert_eq!(
        source.diagnosis.code,
        Some(ExportDiagnosticCode::OptionalAbsent),
        "the optional state is explicit"
    );
    Ok(())
}

#[test]
fn an_absent_registry_required_by_starmap_is_a_failed_configured_source() -> TestResult {
    let (_folder, archive) = opened(Vec::new())?;
    let mut summary = ShopSummary::default();
    let mut ledger = ExportLedger::default();
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        crate::shop_registry::read_source(
            &archive,
            &ArchiveFiles::new(&archive),
            RegistryNeed::Required,
            &mut summary,
            &mut ledger,
            capture,
        )
    })?;
    assert!(read.is_none(), "the configured registry is absent");
    let [source] = ledger.entries.as_slice() else {
        return Err("one required missing registry observation".into());
    };
    assert_eq!(
        ledger.incomplete_inventories,
        [Kind::Shops],
        "a required missing registry hides its configured shop inputs"
    );
    assert_eq!(
        source.entry_index, None,
        "a missing registry has no physical Entry"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::ReadFailed,
        "the actual consuming context requires this source"
    );
    assert_eq!(
        source.diagnosis.code,
        Some(ExportDiagnosticCode::DanglingReference),
        "required absence is distinguishable from optional absence"
    );
    Ok(())
}

#[test]
fn a_registry_projection_failure_does_not_claim_unconsumed_container_reads() -> TestResult {
    let (_folder, archive) = opened(vec![
        TestEntry::stored("padding.txt", "ignored"),
        TestEntry::stored(REGISTRY, "malformed XML"),
        TestEntry::stored(CONTAINER, "bad unused container"),
    ])?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        read_shops(&archive, capture)
    })?;
    assert_eq!(
        read.ledger.incomplete_inventories,
        [Kind::Shops],
        "unparseable registry bytes cannot enumerate shops"
    );
    let [source] = read.ledger.entries.as_slice() else {
        return Err("only the actually consumed registry".into());
    };
    assert_eq!(
        source.entry_index,
        Some(1),
        "the stored Entry index survives"
    );
    assert_eq!(
        source.operation,
        EntryOperation::ShopRegistry,
        "the failing operation survives"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::ProjectionFailed,
        "bytes were read but XML projection failed"
    );
    Ok(())
}

#[test]
fn malformed_registry_rows_retain_valid_shops_and_one_failed_source() -> TestResult {
    let malformed = r#"<ShopBackup ID="not-a-guid" SuperName="Root,Lost"/>"#;
    let (_folder, archive) = opened(vec![TestEntry::stored(REGISTRY, registry(malformed))])?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        read_shops(&archive, capture)
    })?;
    assert_eq!(
        read.shops.len(),
        1,
        "the valid registry row remains present"
    );
    assert_eq!(
        read.ledger.incomplete_inventories,
        Vec::new(),
        "malformed enumerated rows do not imply a truncated source walk"
    );
    let entries = crate::export_health_entries::compile(read.ledger.entries)?;
    assert_eq!(
        entries.failed.len(),
        1,
        "failed source identity is independent of retained rows"
    );
    let counts = crate::export_health_entries::counts(Kind::Shops, &entries.sources)?;
    assert_eq!(counts.selected, 1, "the registry is one source Entry");
    assert_eq!(
        counts.failed_projections, 1,
        "the partial registry is not declared complete"
    );
    assert_eq!(
        counts.read_successful, 1,
        "the canonical source read succeeded"
    );
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
        crate::shop_container_scan::shop_entities(&archive, source, capture)
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
        crate::shop_container_scan::shop_entities(&archive, container, capture)
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
            &options,
            capture,
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
