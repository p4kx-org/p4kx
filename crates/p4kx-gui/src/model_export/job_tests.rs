// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io;
use std::process;

use p4kx_extraction::model::LodSelection;
use p4kx_extraction::{AnimationSet, Archive, ConversionError, GlbOptions, ModelSource};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestModel};

use super::{Ended, ModelExportFailure, failed};
use crate::EntryIndex;

const MODEL: &str = "Data/Objects/cube.cgf";
const GEOMETRY: &str = "Data/Objects/cube.cgfm";

#[test]
fn committed_glb_failure_retains_its_diagnosis_when_the_archive_changes()
-> Result<(), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    let model = TestModel::cube();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL, model.cgf()))
        .entry(TestEntry::zstd(GEOMETRY, model.cgfm()))
        .write_to(&archive)?;
    let index = EntryIndex::build(Archive::open(&archive)?);

    let output = folder.path().join("cube.glb");
    let receipt = output.with_extension("glb.p4kx-origin.jsonl");
    let staged = receipt.with_extension(format!("jsonl.p4kx-{}.part", process::id()));
    fs::write(&staged, b"reserved receipt staging path")?;
    let error = {
        let paths = index.path_index().ok_or("synthetic archive path index")?;
        let source = ModelSource::from_index_with(&paths, MODEL, LodSelection::Only(0))?;
        let Err(error) = source.write_glb_with_geometry_receipt(
            &AnimationSet::default(),
            &GlbOptions::default(),
            &output,
            None,
        ) else {
            return Err("the occupied staging path should prevent receipt publication".into());
        };
        error
    };
    assert!(fs::read(&output)?.starts_with(b"glTF"), "the GLB committed");
    assert!(
        !receipt.try_exists()?,
        "the origin receipt was not published"
    );

    OpenOptions::new().write(true).open(&archive)?.set_len(0)?;
    assert!(
        index.watch().check().is_err(),
        "the archive change was observed"
    );
    assert!(
        matches!(failed(&index, &error), Ended::Failed(ModelExportFailure::Failed(reason))
            if reason.contains("the GLB was committed")),
        "the model export outcome still identifies its committed GLB"
    );
    let uncommitted = ConversionError::Write {
        path: output,
        source: io::Error::other("ordinary write failure"),
    };
    assert!(
        matches!(
            failed(&index, &uncommitted),
            Ended::Failed(ModelExportFailure::ArchiveChanged)
        ),
        "the archive change still takes precedence before commit"
    );
    Ok(())
}
