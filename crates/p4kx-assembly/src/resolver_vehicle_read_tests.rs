// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_archive::{Archive, SourceEncryption};
use p4kx_dataforge::{DataForge, TreeBuilder};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout};

use crate::{
    ArchiveFiles, Entities, LoadoutDocumentKind, LoadoutError, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutRequest, LoadoutResolver, VehicleImplementation,
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
    let (_dir, archive) = archive()?;
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

fn archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(format!("Data/{BASE}"), DEFINITION))
        .entry(TestEntry::stored(format!("Data/{PATCH}"), PATCH_DOCUMENT).encrypted())
        .entry(TestEntry::stored(
            format!("Data/{LOADOUT}"),
            b"<broken>".as_slice(),
        ))
        .write_to(&path)?;
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
