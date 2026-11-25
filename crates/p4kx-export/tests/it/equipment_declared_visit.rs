// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{self, DeclaredEquipmentSnapshot};
use p4kx_export::catalog_queries::shops::{self, ShopInventorySelection};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
    ProvenanceMode,
};
use p4kx_export::{CatalogError, Kind};
use p4kx_extraction::{Archive, CancelToken, SourceInventory};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;

use crate::commodity_v3_fixture::{self as fixture, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const LANGUAGES: CatalogLanguageSelection<'static> = CatalogLanguageSelection::Selected(&[]);
const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const PORTS: &str = "SItemPortContainerComponentParams";
const PORT: &str = "SItemPortDef";
const ATTACHABLE: &str = "SAttachableComponentParams";
const ATTACHMENT: &str = "SItemDefinition";
const SHIP: &str = "libs/foundry/records/entities/spaceships/declared_fixture.xml";
const ITEM: &str = "libs/foundry/records/entities/scitem/fixture/declared_fixture.xml";
const EMPTY: &str = "libs/foundry/records/entities/scitem/fixture/empty_fixture.xml";
const BROKEN: &str = "libs/foundry/records/entities/scitem/fixture/broken_fixture.xml";
const UNRELATED: &str = "libs/foundry/records/entities/commodities/decoy_fixture.xml";
const SHOP: &str = "Data/Scripts/ShopInventories/Inv_DeclaredFixture.json";
const SHARED_GUID: u128 = 0x1165_0001;

#[test]
fn declared_equipment_matches_both_canonical_visitors() -> TestResult {
    let (_directory, archive) = fixture::open(&database(), DcbOrigin::Encrypted)?;
    let targets = [(0, Kind::Ships), (1, Kind::Items), (2, Kind::Items)];
    for mode in [ProvenanceMode::Full, ProvenanceMode::MarkersOnly] {
        let session =
            CatalogQuerySession::open(&archive, LANGUAGES, None)?.with_provenance_mode(mode);
        let parsed = session.parsed(None)?;
        let expected = reference_declarations(&parsed, &targets)?;
        let mut owners = Vec::new();
        let summary = parsed.visit_declared_equipment(
            CatalogRecordSelection::All,
            None,
            |snapshot| -> TestResult {
                owners.push(verify_declared_owner(snapshot, &expected, mode)?);
                Ok(())
            },
        )?;
        assert_eq!(
            owners, targets,
            "Ships and Items retain physical Record order"
        );
        assert_eq!(
            summary.owner_count(),
            3,
            "empty declarations still retain their owner"
        );
        assert_eq!(
            summary.row_count(),
            3,
            "one row per readable selected owner"
        );
        assert_eq!(
            summary.coverage().unreadable_bodies,
            [3],
            "the unreadable Item counts, while the unrelated Commodity is never opened"
        );
    }
    Ok(())
}

fn reference_declarations(
    parsed: &ParsedCatalogQueries<'_>,
    targets: &[(u32, Kind)],
) -> TestResult<Vec<(Value, Value)>> {
    let mut expected = Vec::new();
    for &(index, _) in targets {
        let selection = CatalogRecordSelection::RecordIndex(index);
        let mut port_rows = Value::Null;
        let mut attachment_rows = Value::Null;
        let _ = parsed.visit_item_ports(selection, None, |_, snapshot| -> TestResult {
            port_rows = normalized(&snapshot.rows, &snapshot.source_inventory)?;
            Ok(())
        })?;
        let _ =
            parsed.visit_attachment_constraints(selection, None, |_, snapshot| -> TestResult {
                attachment_rows = normalized(snapshot.rows(), snapshot.source_inventory())?;
                Ok(())
            })?;
        expected.push((port_rows, attachment_rows));
    }
    Ok(expected)
}

fn verify_declared_owner(
    snapshot: &DeclaredEquipmentSnapshot<'_>,
    expected: &[(Value, Value)],
    mode: ProvenanceMode,
) -> TestResult<(u32, Kind)> {
    let [owner] = snapshot.rows() else {
        return Err("one original owner per declared equipment callback".into());
    };
    let index = *owner.source().record_index().value().ok_or("owner index")?;
    let (ports, attachments) = expected
        .get(usize::try_from(index)?)
        .ok_or("expected projections for the physical owner")?;
    let provenance = snapshot.source_inventory();
    assert_eq!(
        normalized(owner.port_containers(), provenance)?,
        *ports,
        "authored ports and their exact original inputs for Record {index}"
    );
    assert_eq!(
        normalized(owner.attachments(), provenance)?,
        *attachments,
        "authored attachments and their exact original inputs for Record {index}"
    );

    let guid = owner.source().guid();
    let expected_guid = if index == 2 {
        SHARED_GUID + 1
    } else {
        SHARED_GUID
    };
    assert_eq!(
        guid.value(),
        Some(&fixture::guid(expected_guid).parse()?),
        "preserve each physical alias of the nominal GUID"
    );
    assert_eq!(
        guid.origin().contains_encrypted_source(),
        Some(true),
        "identity retains its own DataForge encryption marker"
    );
    assert_eq!(
        provenance.provenance_mode(),
        mode,
        "session mode reaches the visitor"
    );
    assert_eq!(
        provenance.uses().is_empty(),
        mode == ProvenanceMode::MarkersOnly,
        "only Full retains the physical uses"
    );
    Ok((index, owner.kind()))
}

#[test]
fn declared_equipment_leaves_unrelated_encrypted_entries_unopened() -> TestResult {
    let (_directory, archive) = archive_without_key()?;
    let before = archive.key_demand().snapshot().unread();
    let selection = CatalogRecordSelection::Guid(fixture::guid(SHARED_GUID).parse()?);
    let summary =
        equipment::visit_declared_equipment(&archive, selection, LANGUAGES, None, |_| {
            Ok::<_, CatalogError>(())
        })?;
    assert_eq!(
        summary.owner_count(),
        2,
        "GUID selection preserves both physical aliases"
    );
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        before,
        "declared equipment must not try to read the shop inventory"
    );
    let shop =
        shops::read_shop_inventories(&archive, ShopInventorySelection::ArchivePath(SHOP), None)?;
    assert_eq!(
        shop.coverage.entries.failed_required_reads, 1,
        "the encrypted shop is a real read failure when requested"
    );
    assert!(
        archive.key_demand().snapshot().unread() > before,
        "the Archive observes a required key only for the explicitly requested shop"
    );
    Ok(())
}

#[test]
fn declared_equipment_stops_after_cancellation_and_callback_errors() -> TestResult {
    let (_directory, archive) = fixture::open(&database(), DcbOrigin::Plain)?;
    let session = CatalogQuerySession::open(&archive, LANGUAGES, None)?;
    let parsed = session.parsed(None)?;
    let cancel = CancelToken::new();
    let mut visits = 0;
    let cancelled =
        parsed.visit_declared_equipment(CatalogRecordSelection::All, Some(&cancel), |_| {
            visits += 1;
            cancel.cancel();
            Ok::<_, CatalogError>(())
        });
    assert!(
        matches!(cancelled, Err(CatalogError::Cancelled)),
        "cancelled visits cannot succeed"
    );
    assert_eq!(visits, 1, "cancellation prevents later callbacks");

    let mut visits = 0;
    let stopped =
        parsed.visit_declared_equipment(CatalogRecordSelection::All, None, |_| -> TestResult {
            visits += 1;
            Err("consumer staging failure".into())
        });
    assert_eq!(
        stopped
            .expect_err("callback failure must interrupt traversal")
            .to_string(),
        "consumer staging failure",
        "the original callback failure survives"
    );
    assert_eq!(visits, 1, "callback failure prevents later callbacks");
    Ok(())
}

fn normalized<Row: Serialize>(rows: &[Row], inventory: &SourceInventory) -> TestResult<Value> {
    let sources = serde_json::json!({"provenance": inventory});
    proof::with_origins(serde_json::to_value(rows)?, &sources)
}

fn archive_without_key() -> TestResult<(TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("declared-equipment.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .entry(TestEntry::stored(SHOP, r#"{"Inventory":[]}"#).encrypted())
        .write_to(&path)?;
    Ok((directory, Archive::open(path)?))
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(ENTITY).array("Components", TestType::StrongPointer(COMPONENT.into())),
        )
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(PORTS)
                .parent(COMPONENT)
                .array("Ports", TestType::Class(PORT.into())),
        )
        .structure(
            TestStruct::new(PORT)
                .field("Name", TestType::String)
                .field("MinSize", TestType::UInt32)
                .field("MaxSize", TestType::UInt32)
                .field("RequiredPortTags", TestType::String),
        )
        .structure(
            TestStruct::new(ATTACHABLE)
                .parent(COMPONENT)
                .field("AttachDef", TestType::Class(ATTACHMENT.into())),
        )
        .structure(
            TestStruct::new(ATTACHMENT)
                .field("Type", TestType::String)
                .field("SubType", TestType::String)
                .field("Size", TestType::UInt32)
                .field("Tags", TestType::String),
        )
        .record(record(SHARED_GUID, "DeclaredShip", SHIP).value(
            "Components",
            TestValue::Array(vec![port_container(), attachment()]),
        ))
        .record(record(SHARED_GUID, "DeclaredItem", ITEM).value(
            "Components",
            TestValue::Array(vec![attachment(), port_container()]),
        ))
        .record(record(SHARED_GUID + 1, "EmptyItem", EMPTY))
        .record(record(SHARED_GUID + 2, "BrokenItem", BROKEN).with_instance_index(u16::MAX))
        .record(
            record(SHARED_GUID + 3, "UnrelatedCommodity", UNRELATED).with_instance_index(u16::MAX),
        )
}

fn record(guid: u128, name: &str, path: &str) -> TestRecord {
    TestRecord::new(guid, ENTITY, format!("{ENTITY}.{name}"), path)
}

fn port_container() -> TestValue {
    let port = || {
        TestValue::Struct(
            TestInstance::new(PORT)
                .value("Name", TestValue::text("mount"))
                .value("MinSize", TestValue::UInt32(1))
                .value("MaxSize", TestValue::UInt32(3))
                .value("RequiredPortTags", TestValue::text("compatible_fixture")),
        )
    };
    TestValue::Struct(
        TestInstance::new(PORTS).value("Ports", TestValue::Array(vec![port(), port()])),
    )
}

fn attachment() -> TestValue {
    let definition = TestInstance::new(ATTACHMENT)
        .value("Type", TestValue::text("PowerPlant"))
        .value("SubType", TestValue::text("FixtureSubtype"))
        .value("Size", TestValue::UInt32(2))
        .value("Tags", TestValue::text("compatible_fixture"));
    TestValue::Struct(
        TestInstance::new(ATTACHABLE).value("AttachDef", TestValue::Struct(definition)),
    )
}
