// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::path::Path;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestStruct};
use serde_json::{Value, json};

use super::document;
use super::plan;
use crate::catalog_queries::equipment::ShipSnapshot;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use crate::{BasetoolCatalogOptions, CatalogError, CatalogFile, CatalogOptions};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const SHIP_PATH: &str = "libs/foundry/records/entities/spaceships/FixtureShip";
const MISSING_IMPLEMENTATION: &str =
    "Scripts/Entities/Vehicles/Implementations/Xml/FixtureShip.xml";

#[test]
fn reduced_document_publishes_every_accepted_dataset_with_the_recorded_boundaries() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = empty_archive(scratch.path())?;
    let watch = ArchiveWatch::new(&archive);
    let output = scratch.path().join("catalog.json.zst");
    let options = CatalogOptions::new(UNIX_EPOCH);
    let (file, sizes) = document::prepare(&archive, scratch.path(), &options, || {
        CatalogFile::create(&output)
    })?;
    assert!(
        !output.exists(),
        "a prepared document is private until commit"
    );
    file.commit_if_current(&watch)?;
    assert_eq!(
        sizes.compressed,
        output.metadata()?.len(),
        "the measured compressed size is the committed file size"
    );
    let payload = zstd::decode_all(File::open(output)?)?;
    assert_eq!(
        sizes.decoded,
        u64::try_from(payload.len())?,
        "the decoded guard covers the entire document and its final newline"
    );
    let root: Value = serde_json::from_slice(&payload)?;
    let schema = super::schema::basetool_catalog_schema();
    jsonschema::validator_for(&schema)?
        .validate(&root)
        .map_err(|error| error.to_string())?;
    verify_document(&root)
}

fn verify_document(root: &Value) -> TestResult {
    assert_eq!(
        root.get("format"),
        Some(&json!(plan::FORMAT)),
        "reduced profile tag"
    );
    assert_eq!(
        root.get("formatVersion"),
        Some(&json!(plan::FORMAT_VERSION)),
        "independent wire version"
    );
    let catalog = root.get("catalog").ok_or("complete catalog payload")?;
    assert_eq!(
        catalog.get("schemaVersion"),
        Some(&json!(crate::contract::SCHEMA_VERSION)),
        "canonical data version"
    );
    assert_eq!(
        catalog.pointer("/exportHealth/status"),
        Some(&json!("complete")),
        "the selected source closure passed all staging checks"
    );
    assert_eq!(
        catalog.pointer("/exportHealth/scope"),
        Some(&json!(plan::SELECTION.as_slice())),
        "the health applies to precisely the approved fields"
    );
    for path in plan::SELECTION {
        let pointer = format!("/{}", path.replace('.', "/"));
        assert!(
            catalog.pointer(&pointer).is_some(),
            "the selected path {path} exists"
        );
    }
    let coverage = catalog
        .get("completeness")
        .and_then(Value::as_array)
        .ok_or("selected coverage")?;
    for (path, selection) in [
        ("loadouts", "shipAndVehicleRoots"),
        ("placements.starmapPlacements", "boundStarmapObjects"),
    ] {
        assert!(
            coverage.contains(&json!({"path": path, "rows": 0, "selection": selection})),
            "empty scope retains its canonical selection"
        );
    }
    assert!(
        catalog.get("provenance").is_none(),
        "the reduced contract carries only requested markers"
    );
    Ok(())
}

#[test]
fn cancelled_reduced_document_preserves_the_preceding_generation() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = empty_archive(scratch.path())?;
    let output = scratch.path().join("catalog.json.zst");
    std::fs::write(&output, b"preceding generation")?;
    let cancel = CancelToken::default();
    cancel.cancel();
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.cancel = Some(&cancel);
    let result = document::prepare(&archive, scratch.path(), &options, || {
        CatalogFile::create(&output)
    });
    assert!(
        matches!(
            result,
            Err(super::ReducedError::Catalog(CatalogError::Cancelled))
        ),
        "cancellation refuses publication"
    );
    assert_eq!(
        std::fs::read(output)?,
        b"preceding generation",
        "the previous generation survives"
    );
    Ok(())
}

#[test]
fn reduced_sdk_retires_staged_output_when_the_build_changes() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let folder = scratch.path();
    p4kx_testkit::write_build_manifest(folder, "sc-alpha-5.0", "5.0.0", "100", "build-1")?;
    let archive = empty_archive(folder)?;
    let output = folder.join("catalog.json.zst");
    let preceding = b"preceding generation";
    std::fs::write(&output, preceding)?;
    let options = BasetoolCatalogOptions::new(UNIX_EPOCH);
    let error = crate::write_basetool_catalog_file(&archive, &options, folder, || {
        let file = CatalogFile::create(&output)?;
        p4kx_testkit::write_build_manifest(folder, "sc-alpha-5.0", "5.0.1", "101", "build-2")?;
        Ok(file)
    })
    .err()
    .ok_or("changed Build must refuse preparation")?;
    assert!(
        matches!(
            error.into_catalog_error(),
            Ok(CatalogError::ArchiveChanged(_))
        ),
        "a source revision change must retain the typed ArchiveChanged error"
    );
    assert_eq!(
        std::fs::read(output)?,
        preceding,
        "the changed source leaves the previous generation in place"
    );
    Ok(())
}

#[test]
fn missing_selected_vehicle_implementation_refuses_reduced_publication() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let folder = scratch.path();
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .record(
            TestEntity::new(1, "FixtureShip", format!("{SHIP_PATH}.xml"))
                .vehicle(MISSING_IMPLEMENTATION, "")
                .record(),
        )
        .build();
    let input = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database))
        .write_to(&input)?;
    let archive = Archive::open(input)?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&plan::LOCALES),
        None,
    )?;
    session.with_ships(CatalogRecordSelection::All, None, |snapshot| {
        check_unavailable_ship(&snapshot)
    })??;

    let output = folder.join("catalog.json.zst");
    let preceding = b"preceding generation";
    std::fs::write(&output, preceding)?;
    let mut factory_called = false;
    let error = crate::write_basetool_catalog_file(
        &archive,
        &BasetoolCatalogOptions::new(UNIX_EPOCH),
        folder,
        || {
            factory_called = true;
            CatalogFile::create(&output)
        },
    )
    .err()
    .ok_or("the missing selected vehicle source must prevent reduced publication")?;
    assert_eq!(
        error.to_string(),
        "the selected data is incomplete at ships",
        "the reduced writer reports the exact blocking dataset"
    );
    assert!(
        !factory_called,
        "the destination factory runs only for complete inputs"
    );
    assert_eq!(
        std::fs::read(output)?,
        preceding,
        "the preceding file remains intact"
    );
    Ok(())
}

fn check_unavailable_ship(snapshot: &ShipSnapshot<'_>) -> TestResult {
    let [ship] = snapshot.rows() else {
        return Err("expected the selected physical Ship".into());
    };
    assert_eq!(
        ship.source().record_index().value(),
        Some(&0),
        "physical Record index"
    );
    assert_eq!(
        ship.source().record_path().value(),
        Some(&SHIP_PATH),
        "canonical Record path retains the original physical owner"
    );
    assert_eq!(
        ship.vehicle_definition().value(),
        Some(&MISSING_IMPLEMENTATION),
        "the authored implementation selector remains readable"
    );
    for (field, state, origin) in [
        ("mass", ship.mass().state(), ship.mass().origin()),
        (
            "massBasis",
            ship.mass_basis().state(),
            ship.mass_basis().origin(),
        ),
        (
            "hullHealth",
            ship.hull_health().state(),
            ship.hull_health().origin(),
        ),
    ] {
        assert_eq!(
            state,
            SourceValueState::Unreadable,
            "{field} needs the requested implementation"
        );
        assert!(
            !origin.source_set_complete(),
            "{field} has no complete value evidence"
        );
    }
    let sources = snapshot.sources();
    assert_eq!(
        sources.coverage().incomplete_fields,
        [0],
        "the physical Record is incomplete"
    );
    assert!(
        sources.dependency_coverage().directory_complete(),
        "the directory survey closed despite the unresolved authored dependency"
    );
    assert!(
        !sources.dependency_coverage().is_complete(),
        "directory evidence alone does not establish the selected values"
    );
    Ok(())
}

fn empty_archive(scratch: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let dcb = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("UnusedFixtureRecord"))
        .build();
    let path = scratch.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", dcb))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}
