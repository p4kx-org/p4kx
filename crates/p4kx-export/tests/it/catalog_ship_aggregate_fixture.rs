// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity,
    TestEntry,
};
use serde_json::Value;

use crate::commodity_v3_fixture::{DcbOrigin, TestResult};

pub(super) const XML: &str = "Data/Scripts/Entities/Vehicles/Implementations/Xml/AggregateShip.xml";
pub(super) const PATCH: &str =
    "Data/Scripts/Entities/Vehicles/Implementations/Xml/Modifications/aggregate_bad.xml";
const DEFINITION: &str = "Scripts/Entities/Vehicles/Implementations/Xml/AggregateShip.xml";
const RECORD: &str = "libs/foundry/records/entities/spaceships/aggregate.xml";
const PARTS: &[u8] = br#"<Vehicle><Parts><Part name="hull" mass="12" damageMax="8"><Parts><Part name="skip" mass="100" damageMax="100" skipPart="true"/></Parts></Part><Part name="hidden" damageMax="40"><ItemPort flags="$uneditable"/></Part></Parts></Vehicle>"#;
const EMPTY: &[u8] = br#"<Vehicle><Parts><Part name="empty"/></Parts></Vehicle>"#;
const BAD_PATCH: &[u8] = br#"<Vehicle><Parts><Part name="empty"/></Parts><Modifications><Modification name="Broken" patchFile="Modifications/aggregate_bad"/></Modifications></Vehicle>"#;

#[derive(Clone, Copy)]
pub(super) enum Case {
    Parts,
    Physics,
    Empty,
    InvalidValues,
    InvalidValuesWithPhysics,
    Missing,
    Malformed,
    Unreadable,
    UnreadableWithPhysics,
    BrokenPatch,
    BrokenPatchWithValues,
    RepairedWithMass,
    RepairedWithoutMass,
    Aliases,
    RoundedZero,
}

pub(super) fn open(case: Case) -> TestResult<(tempfile::TempDir, Archive)> {
    open_with_origin(case, DcbOrigin::Plain)
}

pub(super) fn open_with_origin(
    case: Case,
    origin: DcbOrigin,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let mut archive = ArchiveBuilder::new().entry(dataforge(database(case), origin));
    if let Some(xml) = xml(case)? {
        let mut entry = TestEntry::zstd(XML, xml).encrypted();
        if matches!(case, Case::Unreadable | Case::UnreadableWithPhysics) {
            entry = entry.with_payload(vec![0, 1, 2]);
        }
        archive = archive.entry(entry);
    }
    if matches!(case, Case::BrokenPatch | Case::BrokenPatchWithValues) {
        archive = archive.entry(TestEntry::zstd(PATCH, b"<broken>".as_slice()).encrypted());
    }
    archive.write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn database(case: Case) -> Vec<u8> {
    let mut database = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    for mass in if matches!(case, Case::Aliases) {
        vec![10.0, 20.0]
    } else {
        vec![42.0]
    } {
        let mut entity =
            TestEntity::new(0x7511, "AggregateShip", RECORD).attach_type("NOITEM_Vehicle");
        if !matches!(case, Case::Physics) {
            entity = entity.vehicle(
                DEFINITION,
                if matches!(case, Case::BrokenPatch | Case::BrokenPatchWithValues) {
                    "Broken"
                } else {
                    ""
                },
            );
        }
        if matches!(
            case,
            Case::Parts
                | Case::Physics
                | Case::Aliases
                | Case::UnreadableWithPhysics
                | Case::InvalidValuesWithPhysics
        ) {
            entity = entity.physics_mass(mass);
        }
        database = database.record(entity.record());
    }
    database.build()
}

fn xml(case: Case) -> TestResult<Option<Vec<u8>>> {
    Ok(match case {
        Case::Parts | Case::Unreadable | Case::UnreadableWithPhysics => Some(PARTS.to_vec()),
        Case::Empty | Case::Aliases => Some(EMPTY.to_vec()),
        Case::InvalidValues | Case::InvalidValuesWithPhysics => Some(
            br#"<Vehicle><Parts><Part mass="invalid" damageMax="invalid"/></Parts></Vehicle>"#
                .to_vec(),
        ),
        Case::BrokenPatch => Some(BAD_PATCH.to_vec()),
        Case::BrokenPatchWithValues => Some(
            std::str::from_utf8(BAD_PATCH)?
                .replace(
                    "name=\"empty\"",
                    "name=\"empty\" mass=\"12\" damageMax=\"8\"",
                )
                .into_bytes(),
        ),
        Case::Malformed => Some(b"<broken>".to_vec()),
        Case::RepairedWithMass => Some(repaired(true)?),
        Case::RepairedWithoutMass => Some(repaired(false)?),
        Case::RoundedZero => {
            Some(br#"<Vehicle><Parts><Part mass="0.00000001"/></Parts></Vehicle>"#.to_vec())
        }
        Case::Physics | Case::Missing => None,
    })
}

pub(super) fn catalog(archive: &Archive) -> TestResult<Value> {
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build()?;
    let built =
        pool.install(|| p4kx_export::build_catalog(archive, options.with_full_provenance()))?;
    Ok(serde_json::from_slice(built.json())?)
}

fn repaired(mass: bool) -> TestResult<Vec<u8>> {
    let mut part = TestElement::new("Part");
    if mass {
        part = part.attribute("mass", "2");
    }
    let root = TestElement::new("Vehicle")
        .attribute("name", "REPAIR_THIS_TEXT")
        .child(TestElement::new("Parts").child(part));
    let mut bytes = root.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let at = bytes
        .windows(b"REPAIR_THIS_TEXT".len())
        .position(|value| value == b"REPAIR_THIS_TEXT")
        .ok_or("actual original string table")?;
    *bytes
        .get_mut(at)
        .ok_or("actual original string-table byte")? = 0xff;
    Ok(bytes)
}

fn dataforge(bytes: Vec<u8>, origin: DcbOrigin) -> TestEntry {
    let entry = TestEntry::zstd("Data/Game2.dcb", bytes);
    match origin {
        DcbOrigin::Plain => entry,
        DcbOrigin::Encrypted => entry.encrypted(),
        DcbOrigin::Unknown => entry.encrypted().with_encryption_field(7),
    }
}
