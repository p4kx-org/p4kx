// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;

use p4kx_archive::{Archive, SourceEncryption};
use p4kx_dataforge::DataForge;
use p4kx_extraction::SourceLocation;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry};

use super::{VehiclePortSelection, VehicleXmlSite};
use crate::{
    ArchiveFiles, Entities, LoadoutDocumentKind, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutResolver, VehicleImplementation, VehicleItemType,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const BASE_PATH: &str = "Scripts/Entities/Vehicles/Implementations/Xml/paint_type.xml";
const PATCH_PATH: &str =
    "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/paint_type.xml";

const BASE: &[u8] = br#"<Vehicle><Parts>
    <Part name="duplicate" class="ItemPort"><ItemPort requiredTags="stock">
        <Types><Type id="first" type="WeaponGun"/></Types>
    </ItemPort></Part>
    <Part name="duplicate" class="ItemPort"><ItemPort requiredTags="paint">
        <Types><Type id="second" type="WeaponGun"/></Types>
    </ItemPort></Part>
</Parts><Modifications>
    <Modification name="Patch" patchFile="Modifications/paint_type.xml"/>
    <Modification name="Elem" patchFile="Modifications/paint_type.xml">
        <Elems><Elem idRef="second" name="TYPE" value="Paints"/></Elems>
    </Modification>
</Modifications></Vehicle>"#;

const PATCH: &[u8] = br#"<Modifications><Attributes id="second" type="Paints"/></Modifications>"#;

struct ObservePorts;

impl LoadoutReadObserver for ObservePorts {
    fn observe(&self, _event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn observes_vehicle_port_attributes(&self) -> bool {
        true
    }
}

#[test]
fn equal_port_names_retain_distinct_occurrences_and_the_winning_type() -> TestResult {
    let (_directory, archive) = archive()?;
    let files = ArchiveFiles::new(&archive);
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::with_observed_entities(
        Arc::new(Entities::new(&forge)),
        Arc::new(ObservePorts),
    )
    .with_files(&files);
    for (index, winner, field) in [
        (0, LoadoutDocumentKind::VehiclePatch, "type"),
        (1, LoadoutDocumentKind::VehicleImplementation, "value"),
    ] {
        let ship = forge.record(index).ok_or("the physical fixture Record")?;
        let tree = ship.tree()?;
        let vehicle = resolver
            .read_vehicle_tree(&ship, &tree)?
            .ok_or("the exact observed vehicle")?;
        check_ports(&vehicle, winner, field)?;
    }
    Ok(())
}

fn check_ports(
    vehicle: &VehicleImplementation,
    winner: LoadoutDocumentKind,
    field: &str,
) -> TestResult {
    let ports = vehicle.item_ports_with_selection().collect::<Vec<_>>();
    let [first, second] = ports.as_slice() else {
        return Err("both original duplicate-name ports must be retained".into());
    };
    assert_eq!(
        (first.ordinal(), second.ordinal()),
        (0, 1),
        "duplicates keep effective depth-first positions"
    );
    assert_eq!(
        (first.part().name(), second.part().name()),
        ("duplicate", "duplicate"),
        "both occurrences retain the authored name"
    );
    assert_eq!(
        vehicle
            .item_port_with_selection("DUPLICATE")
            .map(|port| port.ordinal()),
        Some(first.ordinal()),
        "the existing named lookup still selects the first occurrence"
    );
    let left = first
        .position()
        .and_then(VehicleXmlSite::location)
        .ok_or("first port site")?;
    let right = second
        .position()
        .and_then(VehicleXmlSite::location)
        .ok_or("second port site")?;
    assert_ne!(
        left, right,
        "both ports retain their separate original XML locations"
    );
    assert!(
        first.is_complete() && second.is_complete(),
        "both observed port prefixes close"
    );
    assert_eq!(
        first.port().types(),
        ["WeaponGun"],
        "the first duplicate is untouched"
    );
    assert_eq!(
        second.port().types(),
        ["Paints"],
        "the override takes effect on the second port"
    );
    check_winner(second, winner, field)
}

fn check_winner(
    port: &VehiclePortSelection<'_>,
    winner: LoadoutDocumentKind,
    field: &str,
) -> TestResult {
    let [item_type] = port.port().item_types() else {
        return Err("one physical Type declaration is required".into());
    };
    let (label, proof) = item_type.item_type_source();
    assert_eq!(
        label, "Paints",
        "the observed label is the same interpreted type"
    );
    let proof = proof.ok_or("the original effective type attribute")?;
    let original = proof.original();
    assert_eq!(
        original.file().kind(),
        winner,
        "the provenance follows the last effective overlay assignment"
    );
    let (source, attribute, encryption) = match winner {
        LoadoutDocumentKind::VehiclePatch => {
            (PATCH, r#"type="Paints""#, SourceEncryption::Encrypted)
        }
        _ => (BASE, r#"value="Paints""#, SourceEncryption::Unencrypted),
    };
    assert_eq!(
        site_field(original),
        Some(field),
        "the actual attribute coordinate survives"
    );
    assert_eq!(
        original_attribute(original, source)?,
        attribute,
        "the exact original XML bytes supply the value"
    );
    assert_eq!(
        site_encryption(original),
        Some(encryption),
        "the physical value's encryption follows its Entry"
    );
    assert!(
        proof.selectors().iter().any(|site| {
            site_field(site) == Some("id")
                && site_encryption(site) == Some(SourceEncryption::Encrypted)
        }),
        "the winning assignment retains its encrypted target-selection input"
    );
    check_type_selection(item_type)
}

fn check_type_selection(item_type: &VehicleItemType) -> TestResult {
    let selected = item_type.selection_sites().collect::<Vec<_>>();
    let [section, declaration] = selected.as_slice() else {
        return Err("the actual Types container and Type node must be visible".into());
    };
    assert_ne!(
        section.location(),
        declaration.location(),
        "the effective Type and its collection have distinct physical coordinates"
    );
    assert!(
        section.location().is_some() && declaration.location().is_some(),
        "both original selection sites are bound"
    );
    Ok(())
}

fn original_attribute<'bytes>(
    site: &VehicleXmlSite,
    bytes: &'bytes [u8],
) -> TestResult<&'bytes str> {
    let Some(SourceLocation::Subresource {
        namespace, indices, ..
    }) = site.location()
    else {
        return Err("the actual attribute coordinate is missing".into());
    };
    let [start, end] = indices.as_slice() else {
        return Err("the original attribute coordinate has no byte interval".into());
    };
    if namespace != "textXmlAttribute" {
        return Err("the fixture requires a text XML attribute coordinate".into());
    }
    let span = bytes
        .get(usize::try_from(*start)?..usize::try_from(*end)?)
        .ok_or("the location falls outside the exact original XML")?;
    Ok(std::str::from_utf8(span)?)
}

fn site_field(site: &VehicleXmlSite) -> Option<&str> {
    let SourceLocation::Subresource { field, .. } = site.location()? else {
        return None;
    };
    Some(field)
}

fn site_encryption(site: &VehicleXmlSite) -> Option<SourceEncryption> {
    site.file()
        .source()
        .entry_origin()
        .map(p4kx_extraction::EntryOrigin::encryption)
}

fn archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(format!("Data/{BASE_PATH}"), BASE))
        .entry(TestEntry::stored(format!("Data/{PATCH_PATH}"), PATCH).encrypted())
        .write_to(&path)?;
    Ok((
        directory,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn database() -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(19, "Base", "synthetic/base.xml")
                .vehicle(BASE_PATH, "Patch")
                .record(),
        )
        .record(
            TestEntity::new(19, "Elem", "synthetic/elem.xml")
                .vehicle(BASE_PATH, "Elem")
                .record(),
        )
        .build()
}
