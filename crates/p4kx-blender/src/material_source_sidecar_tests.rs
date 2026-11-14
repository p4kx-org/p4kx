// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use flate2::{Compression, GzBuilder};
use p4kx_extraction::material_source_info::{MaterialSourceEvidence, MaterialSourceOriginError};
use p4kx_extraction::{
    FieldOrigins, FieldPath, InputScope, MaterialLibrarySourceInfo, SourceInput,
    SourceInventoryFreezeError, SourceLocation, SourceUseRole, SourceUses,
};
use p4kx_model::{MaterialLibrary, MaterialSource};

use super::{DIRECTORY, FORMAT, Row, VERSION, row, write};
use crate::finite::tests::manifest;
use crate::material_source_sidecar::{MaterialSourceReference, file_name};
use crate::package_receipt::{RECEIPT_FILE, write_metadata};
use crate::stage::Staging;
use crate::{MANIFEST_FILE, MaterialLibrarySourceDescription, PackageError, PackageWriteContext};

type TestResult = Result<(), Box<dyn Error>>;

fn large_library() -> Result<MaterialLibrary, p4kx_model::MaterialError> {
    let payload = "x".repeat(8_192);
    let mut xml = String::from(r#"<Material Shader="Illum"><Details>"#);
    for _ in 0..64 {
        xml.push_str("<Source>");
        xml.push_str(&payload);
        xml.push_str("</Source>");
    }
    xml.push_str("</Details></Material>");
    MaterialLibrary::parse("synthetic/large", xml.as_bytes())
}

#[test]
fn aborted_large_library_projection_releases_already_staged_artifacts() -> TestResult {
    let library = large_library()?;
    let folder = tempfile::tempdir()?;
    let destination = folder.path().join("package");
    let observed = AtomicBool::new(false);
    let ordinary = PackageWriteContext::standalone(&|| false, &observed);
    output(&destination, &library, &ordinary)?;
    let previous = receipt_bytes(&destination)?;
    let staged_file = OnceLock::new();
    let polls = AtomicUsize::new(0);
    let cancel = || {
        staged_file.get().is_some_and(|path: &std::path::PathBuf| {
            path.is_file() && polls.fetch_add(1, Ordering::Relaxed) >= 48
        })
    };
    let context = PackageWriteContext::standalone(&cancel, &observed);
    let stage = Staging::create(&destination, context)?;
    let created = context.create_directory(&stage.path().join(DIRECTORY))?;
    staged_file
        .set(stage.path().join(file_name(0)))
        .map_err(|_| "a single stage file")?;
    let result = write(stage.path(), 0, &library.authored, None, &context);
    drop(created);
    assert!(
        matches!(stage.complete(result), Err(PackageError::Cancelled)),
        "mid-library interruption is a typed cancellation"
    );
    assert!(
        receipt_bytes(&destination)? == previous,
        "the previous receipt and every listed file remain byte-identical"
    );
    assert_eq!(
        fs::read_dir(folder.path())?.count(),
        1,
        "only the previous package remains"
    );
    Ok(())
}

#[test]
fn json_fragment_cancellation_is_not_reported_as_a_write_failure() -> TestResult {
    let library = large_library()?;
    let source = MaterialLibrarySourceInfo::of_source(&library.authored);
    let observed = AtomicBool::new(false);
    let polls = AtomicUsize::new(0);
    let stop = || polls.fetch_add(1, Ordering::Relaxed) >= 12;
    let context = PackageWriteContext::standalone(&stop, &observed);
    let mut bytes = Vec::new();
    let result = row(
        &mut bytes,
        &context,
        Path::new("test-material-source"),
        &Row::Library { authored: &source },
    );
    assert!(
        matches!(result, Err(PackageError::Cancelled)),
        "an interrupted JSON fragment keeps its type"
    );
    assert!(
        bytes.starts_with(br#"{"type":"library""#),
        "the row had already started"
    );
    assert!(
        !bytes.ends_with(
            b"
"
        ),
        "the row did not reach its delimiter"
    );
    Ok(())
}

#[test]
fn the_library_and_field_origin_projections_poll_during_retained_contents() -> TestResult {
    let library = large_library()?;
    let mut budget = 16;
    let value = MaterialLibrarySourceInfo::of_source_with(&library.authored, &mut || {
        if budget == 0 {
            return Err("stop after some projected values");
        }
        budget -= 1;
        Ok(())
    });
    assert!(
        matches!(value, Err("stop after some projected values")),
        "the original checkpoint error propagates"
    );
    let uses = SourceUses::new();
    let shared = uses.shared_origin(&[], false);
    let evidence = MaterialSourceEvidence::new(&uses, None, &shared);
    let address = FieldPath::parse("/authored")?;
    let mut fields = FieldOrigins::new();
    let polls = AtomicUsize::new(0);
    let result = evidence.project_with(&library.authored, &address, &mut fields, &|| {
        polls.fetch_add(1, Ordering::Relaxed) >= 16
    });
    assert!(
        matches!(result, Err(MaterialSourceOriginError::Cancelled)),
        "field evidence stops mid-traversal"
    );
    let projected = fields.iter().count();
    let mut complete = FieldOrigins::new();
    evidence.project(&library.authored, &address, &mut complete)?;
    assert!(
        projected > 0 && projected < complete.iter().count(),
        "field traversal stops after starting"
    );
    for (path, origin) in fields.iter() {
        assert_eq!(
            complete.get(path),
            Some(origin),
            "the stored subset comes from the same projection"
        );
    }
    Ok(())
}

#[test]
fn completeness_scan_matches_the_wire_for_complete_and_bounded_source_trees() -> TestResult {
    use p4kx_model::MaterialSourceLimits;
    use std::convert::Infallible;

    let normal = large_library()?;
    let mut limits = MaterialSourceLimits::default();
    limits.elements = Some(1);
    let bounded = MaterialLibrary::parse_with(
        "synthetic/bounded",
        b"<Material><Child/></Material>",
        limits,
        || Ok::<(), Infallible>(()),
    )?;
    for (library, complete) in [(normal, true), (bounded, false)] {
        let wire = MaterialLibrarySourceInfo::of_source(&library.authored);
        let raw =
            MaterialLibrarySourceInfo::source_is_complete_with(&library.authored, &mut || {
                Ok::<(), Infallible>(())
            })?;
        assert_eq!(raw, complete, "the fixture has the required completeness");
        assert_eq!(
            raw,
            wire.is_complete(),
            "collection uses the same completeness decisions"
        );
    }
    Ok(())
}

#[test]
fn freezing_a_large_inventory_preserves_the_checkpoint_error_and_registry() -> TestResult {
    let uses = SourceUses::new();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    for ordinal in 0..32 {
        scope.at(
            SourceLocation::Subresource {
                namespace: "synthetic".to_owned(),
                indices: vec![ordinal],
                field: "value".to_owned(),
            },
            SourceUseRole::Payload,
        )?;
    }
    let mut budget = 12;
    let frozen = uses.freeze_with(|| {
        if budget == 0 {
            return Err("inventory checkpoint stopped");
        }
        budget -= 1;
        Ok(())
    });
    assert!(
        matches!(
            frozen,
            Err(SourceInventoryFreezeError::Interrupted(
                "inventory checkpoint stopped"
            ))
        ),
        "cloning source-use rows is interruptible"
    );
    assert_eq!(
        uses.freeze()?.uses().len(),
        32,
        "partial inventory collection does not change registered evidence"
    );
    Ok(())
}

#[test]
fn checkpoints_preserve_the_complete_sidecar_and_receipt_bytes() -> TestResult {
    let library = large_library()?;
    let folder = tempfile::tempdir()?;
    let observed = AtomicBool::new(false);
    let polls = AtomicUsize::new(0);
    let observe = || {
        polls.fetch_add(1, Ordering::Relaxed);
        false
    };
    let context = PackageWriteContext::standalone(&observe, &observed);
    let first = folder.path().join("uninterrupted");
    let second = folder.path().join("checked");
    let ordinary = PackageWriteContext::standalone(&|| false, &observed);
    let name = file_name(0);
    output(&first, &library, &ordinary)?;
    output(&second, &library, &context)?;
    assert!(
        polls.load(Ordering::Relaxed) > 200,
        "the verified stream actually invokes checkpoints"
    );
    assert_eq!(
        fs::read(first.join(&name))?,
        baseline_bytes(&library.authored)?,
        "noncancelled gzip bytes match direct pre-checkpoint serialization"
    );
    assert_eq!(
        fs::read(first.join(&name))?,
        fs::read(second.join(&name))?,
        "same gzip bytes across checkpoint implementations"
    );
    assert_eq!(
        fs::read(first.join(RECEIPT_FILE))?,
        fs::read(second.join(RECEIPT_FILE))?,
        "the whole output receipt remains deterministic"
    );
    Ok(())
}

#[derive(PartialEq, Eq)]
struct PublishedFile {
    path: String,
    bytes: Vec<u8>,
}

fn receipt_bytes(root: &Path) -> Result<Vec<PublishedFile>, Box<dyn Error>> {
    let receipt = fs::read(root.join(RECEIPT_FILE))?;
    let inventory: serde_json::Value = serde_json::from_slice(&receipt)?;
    let paths = inventory
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or("previous receipt lists files")?;
    let mut files = vec![PublishedFile {
        path: RECEIPT_FILE.to_owned(),
        bytes: receipt,
    }];
    for row in paths {
        let path = row
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or("previous receipt lists each file's path")?;
        files.push(PublishedFile {
            path: path.to_owned(),
            bytes: fs::read(root.join(path))?,
        });
    }
    assert!(
        files.iter().any(|file| file.path == MANIFEST_FILE),
        "the scene manifest is included"
    );
    assert!(
        files.iter().any(|file| file.path == file_name(0)),
        "the material source companion is included"
    );
    Ok(files)
}

fn output(root: &Path, library: &MaterialLibrary, context: &PackageWriteContext<'_>) -> TestResult {
    fs::create_dir(root)?;
    let directory = context.create_directory(&root.join(DIRECTORY))?;
    let file = write(root, 0, &library.authored, None, context)?;
    let mut manifest = manifest(Vec::new());
    manifest
        .material_libraries
        .push(MaterialLibrarySourceDescription {
            path: library.path.clone(),
            authored_file: Some(MaterialSourceReference::new(0)),
            authored: None,
            authored_origin: None,
            emission_provenance: None,
        });
    write_metadata(root, &manifest, vec![file], context)?;
    drop(directory);
    Ok(())
}

fn baseline_bytes(source: &MaterialSource) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut gzip = GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), Compression::default());
    let mut emit = |row: &Row<'_>| {
        serde_json::to_writer(&mut gzip, row)?;
        gzip.write_all(
            b"
",
        )
    };
    emit(&Row::Header {
        format: FORMAT,
        version: VERSION,
        library: 0,
    })?;
    let authored = MaterialLibrarySourceInfo::of_source(source);
    emit(&Row::Library {
        authored: &authored,
    })?;
    let uses = SourceUses::new();
    let shared = uses.shared_origin(&[], false);
    let evidence = MaterialSourceEvidence::new(&uses, None, &shared);
    let mut fields = FieldOrigins::new();
    evidence.project(source, &FieldPath::parse("/authored")?, &mut fields)?;
    for (path, origin) in fields.iter() {
        emit(&Row::Field { path, origin })?;
    }
    emit(&Row::End {
        sources: 0,
        uses: 0,
        fields: fields.iter().count(),
    })?;
    Ok(gzip.finish()?)
}
