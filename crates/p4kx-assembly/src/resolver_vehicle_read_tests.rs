// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_archive::{Archive, SourceEncryption};
use p4kx_dataforge::{DataForge, Record, RecordTree, TreeBuilder};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout};

use crate::{
    ArchiveFiles, Entities, LoadoutDocumentKind, LoadoutError, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutRequest, LoadoutResolver, ValueOrigin, VehicleImplementation,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const BASE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/scoped.xml";
const PATCH: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/scoped.xml";
const LOADOUT: &str = "synthetic/scoped-loadout.xml";
const DEFINITION: &[u8] = br#"<Vehicle><Parts/>
    <Modifications>
        <Modification name="Scoped" patchFile="Modifications/scoped.xml"/>
    </Modifications>
</Vehicle>"#;
const PATCH_DOCUMENT: &[u8] = br#"<Modifications><Parts>
    <Part name="paint" class="ItemPort">
        <ItemPort id="paint-port" requiredTags="scoped"><Types><Type type="Paints"/></Types></ItemPort>
    </Part>
</Parts></Modifications>"#;

#[derive(Default)]
struct Observer {
    reads: Mutex<Vec<LoadoutDocumentKind>>,
    vehicles: Mutex<Vec<u32>>,
}

impl LoadoutReadObserver for Observer {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        if let LoadoutReadEvent::FileRead { observation, .. } = event {
            self.reads
                .lock()
                .expect("the file witness lock remains available")
                .push(observation.kind());
        }
        ControlFlow::Continue(())
    }

    fn observes_vehicle_port_attributes(&self) -> bool {
        true
    }

    fn vehicle_used(
        &self,
        record_index: u32,
        _vehicle: &VehicleImplementation,
        _caller: &LoadoutRequest,
    ) -> ControlFlow<()> {
        self.vehicles
            .lock()
            .expect("the vehicle witness lock remains available")
            .push(record_index);
        ControlFlow::Continue(())
    }
}

#[test]
fn exact_tree_reads_vehicle_evidence_without_loading_an_authored_loadout() -> TestResult {
    let (_dir, archive) = archive(Some(PATCH_DOCUMENT))?;
    let files = ArchiveFiles::new(&archive);
    let data = database();
    let forge = DataForge::open(&data)?;
    let record = forge.record(0).ok_or("the first ship Record")?;
    let alias = forge.record(1).ok_or("the second ship Record")?;
    let tree = TreeBuilder::new().build(&record)?;
    let other = TreeBuilder::new().build(&alias)?;
    let observer = Arc::new(Observer::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone())
            .with_files(&files);
    assert!(
        matches!(
            resolver.read_vehicle_tree(&record, &other),
            Err(LoadoutError::UnreadableRecord { .. })
        ),
        "an equal GUID cannot substitute a different original physical tree"
    );
    let read = resolver
        .read_vehicle_tree(&record, &tree)?
        .ok_or("the exact vehicle implementation")?;
    verify_selected_port(&read)?;
    verify_normal_resolution(&archive, &files, &forge, &record, &tree, &read)?;
    let reads = observer.reads.lock().map_err(|_| "the file witness lock")?;
    assert_eq!(
        *reads,
        [
            LoadoutDocumentKind::VehicleImplementation,
            LoadoutDocumentKind::VehiclePatch,
        ],
        "both required documents are read and the unrelated XML loadout is untouched"
    );
    let vehicles = observer
        .vehicles
        .lock()
        .map_err(|_| "the owner witness lock")?;
    assert_eq!(
        *vehicles,
        [record.index()],
        "a single physical vehicle caller is observed"
    );
    Ok(())
}

fn verify_normal_resolution<'f>(
    archive: &Archive,
    files: &'f ArchiveFiles<'_>,
    forge: &'f DataForge<'f>,
    record: &Record<'f>,
    tree: &RecordTree<'f>,
    scoped: &VehicleImplementation,
) -> TestResult {
    let mut resolver = LoadoutResolver::new(forge).with_files(files);
    let node = resolver.resolve_tree(record, tree)?;
    assert!(
        node.issues()
            .iter()
            .any(|issue| matches!(issue, LoadoutError::XmlLoadout { path, .. } if path == LOADOUT)),
        "normal resolution encounters the malformed unrelated loadout"
    );
    let normal = node.vehicle().ok_or("the normally resolved vehicle")?;
    let digest = scoped
        .complete_structure_digest()
        .map_err(|_| "the scoped implementation is complete")?;
    assert_eq!(
        normal.complete_structure_digest(),
        Ok(digest),
        "both routes compare the same effective vehicle despite the unrelated loadout issue"
    );
    verify_declaration(
        archive,
        scoped,
        normal,
        BASE,
        ValueOrigin::Base,
        SourceEncryption::Unencrypted,
    )?;
    verify_declaration(
        archive,
        scoped,
        normal,
        PATCH,
        ValueOrigin::Patch,
        SourceEncryption::Encrypted,
    )
}

fn verify_declaration(
    archive: &Archive,
    scoped: &VehicleImplementation,
    normal: &VehicleImplementation,
    path: &str,
    origin: ValueOrigin,
    encryption: SourceEncryption,
) -> TestResult {
    let path = format!("Data/{path}").replace('/', "\\");
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or("the fixture declaration Entry")?;
    let expected = archive
        .entry_origin(entry)
        .ok_or("the original Entry origin")?;
    for vehicle in [scoped, normal] {
        let actual = vehicle
            .declaration_origin(origin)
            .ok_or("the applied declaration origin")?;
        assert_eq!(
            actual.binding(),
            expected.binding(),
            "both readers bind the original Entry"
        );
        assert_eq!(
            actual.encryption(),
            encryption,
            "the selected Entry retains its encryption"
        );
    }
    Ok(())
}

#[test]
fn exact_tree_refuses_the_comparison_digest_for_a_missing_patch() -> TestResult {
    let (_dir, archive) = archive(None)?;
    let files = ArchiveFiles::new(&archive);
    let data = database();
    let forge = DataForge::open(&data)?;
    let record = forge.record(0).ok_or("the physical ship Record")?;
    let tree = TreeBuilder::new().build(&record)?;
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let scoped = resolver
        .read_vehicle_tree(&record, &tree)?
        .ok_or("the readable base implementation")?;
    let node = LoadoutResolver::new(&forge)
        .with_files(&files)
        .resolve_tree(&record, &tree)?;
    let normal = node.vehicle().ok_or("the normally resolved base")?;
    for vehicle in [scoped.as_ref(), normal] {
        let Err(issues) = vehicle.complete_structure_digest() else {
            return Err("an unresolved selected patch cannot yield a comparison digest".into());
        };
        assert!(
            issues
                .iter()
                .any(|issue| matches!(issue, LoadoutError::MissingVehicleImplementation { .. })),
            "both readers retain the missing selected patch"
        );
        assert!(
            vehicle.declaration_origin(ValueOrigin::Patch).is_none(),
            "an unavailable patch has no successful declaration origin"
        );
    }
    assert_eq!(
        scoped.structure_digest(),
        normal.structure_digest(),
        "the raw digest remains readable without establishing comparison eligibility"
    );
    verify_declaration(
        &archive,
        &scoped,
        normal,
        BASE,
        ValueOrigin::Base,
        SourceEncryption::Unencrypted,
    )
}

fn verify_selected_port(vehicle: &VehicleImplementation) -> TestResult {
    let port = vehicle
        .item_port_with_selection("paint")
        .ok_or("the selected XML paint port")?;
    assert_eq!(
        port.port().required_tags(),
        "scoped",
        "the exact original Modification was applied"
    );
    let (_, proof) = port.port().required_tags_source();
    let source = proof
        .ok_or("the exact applied requiredTags attribute")?
        .original()
        .file()
        .source()
        .entry_origin()
        .ok_or("the original vehicle patch Entry")?;
    assert_eq!(
        source.encryption(),
        SourceEncryption::Encrypted,
        "the winning attribute retains the encryption of the physical patch"
    );
    Ok(())
}

fn archive(override_xml: Option<&[u8]>) -> TestResult<(tempfile::TempDir, Archive)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let mut builder = ArchiveBuilder::v2()
        .entry(TestEntry::stored(format!("Data/{BASE}"), DEFINITION))
        .entry(TestEntry::stored(
            format!("Data/{LOADOUT}"),
            b"<broken>".as_slice(),
        ));
    if let Some(patch_contents) = override_xml {
        builder =
            builder.entry(TestEntry::stored(format!("Data/{PATCH}"), patch_contents).encrypted());
    }
    builder.write_to(&path)?;
    Ok((
        dir,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn database() -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(17, "Carrier", "synthetic/carrier.xml")
                .vehicle(BASE, "Scoped")
                .loadout(TestLoadout::Xml(LOADOUT.to_owned()))
                .record(),
        )
        .record(
            TestEntity::new(17, "Alias", "synthetic/alias.xml")
                .vehicle(BASE, "")
                .record(),
        )
        .build()
}
