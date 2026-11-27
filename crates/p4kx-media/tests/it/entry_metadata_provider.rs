// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_media::ExportOptions;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;

use super::support::{self, TestResult};

fn extension_only_source() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = ArchiveBuilder::v2()
        .entry(TestEntry::stored("same.bin", b"one"))
        .entry(TestEntry::stored("same.bin", b"two"))
        .build();
    let trailer = bytes.len().checked_sub(175).ok_or("v2 trailer")?;
    for (base, extension) in [(0, 8), (24, 32), (48, 56)] {
        let value: [u8; 8] = bytes
            .get(trailer + base..trailer + base + 8)
            .ok_or("base field")?
            .try_into()?;
        bytes
            .get_mut(trailer + extension..trailer + extension + 8)
            .ok_or("extension field")?
            .copy_from_slice(&value);
        bytes
            .get_mut(trailer + base..trailer + base + 8)
            .ok_or("base field")?
            .fill(0);
    }
    Ok(bytes)
}

#[test]
fn archive_provider_exports_extension_only_v2_ordinals_and_explicit_zip_absence() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("extension-only.p4k");
    std::fs::write(&source, extension_only_source()?)?;
    let archive = Archive::open(&source)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    let entries = rows
        .iter()
        .filter(|row| row.subresource.namespace == "archive-entry");
    let mut count = 0;
    for (ordinal, row) in entries.enumerate() {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            row.data["metadata"],
            json!({
                "owner_entry_index": ordinal,
                "dos_timestamp": {"raw_date": 0, "raw_time": 0, "calendar": {"state": "invalid"}},
                "record_format": "p4k_v2", "original_directory_ordinal": ordinal.to_string(),
                "raw_compression_method": 0, "zip_header": null, "raw_v2_encryption_word": 0
            }),
            "each physical extension record keeps its own source ordinal"
        );
        assert_eq!(
            row.data["installed_bytes"], 3,
            "install progress stays independent"
        );
        count += 1;
    }
    assert_eq!(
        count, 2,
        "both duplicate-name extension records are exported"
    );
    assert!(
        index.complete,
        "metadata enrichment preserves source completeness"
    );
    Ok(())
}
