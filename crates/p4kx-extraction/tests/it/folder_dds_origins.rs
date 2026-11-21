// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::RecordExportReport;
use p4kx_testkit::check_dds_origin_pair;

use super::{Fixture, TestResult, tree};

const DDS: &str = "Data/Textures/rock_diff.dds";
const RECEIPT: &str = "Data/Textures/rock_diff.dds.p4kx-origin.jsonl";

#[test]
fn a_converted_folder_equals_what_extraction_with_conversion_writes() -> TestResult {
    let fixture = Fixture::new()?;
    let expected = fixture.extract("converted", true)?;
    let raw = fixture.extract("raw", false)?;

    let report = fixture.convert(&raw, "from-folder", None, true)?;

    assert!(report.is_complete(), "complete: {report:?}");
    let [artifact] = report.merged_texture_artifacts() else {
        return Err("one observed loose DDS artifact".into());
    };
    assert_eq!(artifact.path, DDS, "same emitted DDS path");
    assert_eq!(artifact.origin_receipt, RECEIPT, "durable sidecar path");
    let mut written = tree(&fixture.folder("from-folder"))?;
    let mut archived = tree(&expected)?;
    let archive_receipt = archived.remove(RECEIPT).ok_or("Archive DDS receipt")?;
    let loose_receipt = written.remove(RECEIPT).ok_or("loose DDS receipt")?;
    let dds = written.get(DDS).ok_or("the written DDS")?;
    check_dds_origin_pair(dds, &archive_receipt, &loose_receipt)?;
    assert_eq!(written, archived, "identical payload files");
    assert!(
        written
            .keys()
            .any(|name| Path::new(name).extension().is_some_and(|ext| ext == "png")),
        "the texture became a PNG: {:?}",
        written.keys().collect::<Vec<_>>()
    );
    assert!(
        written.keys().any(|name| name == "Data/Sounds/Media/5.wav"),
        "the sound was decoded"
    );
    assert_eq!(report.records().map(RecordExportReport::written), Some(1));
    Ok(())
}

#[test]
fn loose_merge_replaces_old_archive_proof_for_identical_payload_bytes() -> TestResult {
    let fixture = Fixture::new()?;
    let target = fixture.extract("converted", true)?;
    let archive_receipt = fs::read(target.join(RECEIPT))?;
    let archived_dds = fs::read(target.join(DDS))?;
    let raw = fixture.extract("raw", false)?;

    let report = fixture.convert(&raw, "converted", None, true)?;
    assert!(report.is_complete(), "{report:?}");
    let [artifact] = report.merged_texture_artifacts() else {
        return Err("the replacement must have one new loose origin receipt".into());
    };
    assert_eq!(
        artifact.origin_receipt, RECEIPT,
        "the old location is reused"
    );
    let written = fs::read(target.join(DDS))?;
    assert_eq!(
        written, archived_dds,
        "the loose merge emits the same DDS bytes"
    );
    let loose_receipt = fs::read(target.join(RECEIPT))?;
    check_dds_origin_pair(&written, &archive_receipt, &loose_receipt)
}
