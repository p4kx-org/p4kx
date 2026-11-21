// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write};
#[cfg(unix)]
use std::path::{Path, PathBuf};

use p4kx_extraction::{ConflictPolicy, OriginDigestWriter, Texture, TextureConversion};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat};
use serde_json::Value;

use super::fixture::{Fixture, HEADER, files, rock, run_with, texture_entries};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const COLOR: &str = "Data/Textures/rock_diff.dds";
const ALPHA: &str = "Data/Textures/rock_diff.dds.a";
const COLOR_RECEIPT: &str = "Data/Textures/rock_diff.dds.p4kx-origin.jsonl";
const ALPHA_RECEIPT: &str = "Data/Textures/rock_diff.dds.a.p4kx-origin.jsonl";
const ALPHA_HEADER: &str = "Data\\Textures\\rock_diff.dds.a";

const MERGE_ONLY: TextureConversion = TextureConversion {
    merge: true,
    png: None,
    keep_raw: false,
};

#[test]
fn bulk_colour_and_alpha_use_the_same_exact_spans_as_standalone_merges() -> TestResult {
    let dds = tailed();
    let entries = dds
        .entries(HEADER)
        .into_iter()
        .enumerate()
        .map(|(i, entry)| {
            if matches!(i, 2 | 5) {
                entry.encrypted()
            } else {
                entry
            }
        });
    let fixture = Fixture::new(entries.collect())?;
    let report = run_with(&fixture, ConflictPolicy::Overwrite, MERGE_ONLY);
    assert!(report.is_complete(), "{report:?}");
    let [color, alpha] = report.merged_texture_artifacts() else {
        return Err("colour and alpha should each publish one companion".into());
    };
    assert_eq!(color.path, COLOR, "colour artifact follows its header");
    assert_eq!(alpha.path, ALPHA, "alpha artifact follows its own header");
    assert_eq!(
        color.origin_receipt, COLOR_RECEIPT,
        "colour's portable sibling"
    );
    assert_eq!(
        alpha.origin_receipt, ALPHA_RECEIPT,
        "alpha's portable sibling"
    );
    assert_eq!(
        fixture.read(COLOR)?,
        dds.merged_bytes(),
        "merged colour bytes"
    );
    let expected_alpha = dds.alpha_tail().ok_or("authored alpha tail")?;
    assert_eq!(
        fixture.read(ALPHA)?,
        expected_alpha.merged_bytes(),
        "merged alpha bytes"
    );
    compare_standalone(&fixture, HEADER, color)?;
    compare_standalone(&fixture, ALPHA_HEADER, alpha)?;
    assert_eq!(
        files(&fixture.target)?,
        [COLOR, ALPHA, ALPHA_RECEIPT, COLOR_RECEIPT],
        "only the two bound payloads and their receipts"
    );
    Ok(())
}

fn compare_standalone(
    fixture: &Fixture,
    entry: &str,
    artifact: &p4kx_extraction::MergedTextureArtifact,
) -> TestResult {
    let standalone = tempfile::tempdir()?;
    let output = standalone.path().join("standalone.dds");
    let (_, origins) =
        Texture::from_archive(&fixture.archive, entry)?.merge_to_file_with_origins(&output)?;
    let actual = rows(&fixture.read(&artifact.origin_receipt)?)?;
    let expected = rows(&fs::read(origins.receipt)?)?;
    assert_eq!(
        actual.get(1..),
        expected.get(1..),
        "the same archive inputs prove exactly the same ranges and roles"
    );
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(&fixture.read(&artifact.path)?)?;
    let expected = measured.finish()?;
    assert_eq!(
        artifact.digest, expected,
        "the report binds the exact payload"
    );
    let header = actual.first().ok_or("companion header")?;
    assert_eq!(
        header.pointer("/binding/sha256").and_then(Value::as_str),
        Some(expected.sha256()),
        "the receipt binds those bytes"
    );
    let mut flags = actual
        .iter()
        .filter(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("ddsMipBytes"))
        .map(|row| row["containsEncryptedSource"].as_bool());
    assert!(
        flags.clone().any(|flag| flag == Some(true)),
        "one encrypted authored mip"
    );
    assert!(
        flags.clone().any(|flag| flag == Some(false)),
        "other mip inputs are clear"
    );
    assert!(
        flags.all(|flag| flag.is_some()),
        "every emitted mip has an exact marker"
    );
    Ok(())
}

#[test]
fn keep_policies_preserve_an_existing_owned_receipt_without_new_proof() -> TestResult {
    let fixture = Fixture::new(texture_entries(&rock()))?;
    let written = run_with(&fixture, ConflictPolicy::Overwrite, MERGE_ONLY);
    assert!(written.is_complete(), "{written:?}");
    let before = fixture.read(COLOR_RECEIPT)?;
    for policy in [ConflictPolicy::Skip, ConflictPolicy::IfChanged] {
        let kept = run_with(&fixture, policy, MERGE_ONLY);
        assert!(kept.is_complete(), "{kept:?}");
        assert!(
            kept.merged_texture_artifacts().is_empty(),
            "kept output gets no new proof"
        );
        assert_eq!(
            fixture.read(COLOR_RECEIPT)?,
            before,
            "existing receipt is byte-identical"
        );
        assert_eq!(
            fixture.read(COLOR)?,
            rock().merged_bytes(),
            "the DDS was retained"
        );
    }
    Ok(())
}

#[test]
fn a_foreign_alpha_receipt_preserves_its_existing_dds_and_the_committed_colour() -> TestResult {
    let dds = tailed();
    let fixture = Fixture::new(dds.entries(HEADER))?;
    fixture.write(ALPHA, b"previous alpha DDS")?;
    fixture.write(ALPHA_RECEIPT, b"foreign alpha companion")?;
    let report = run_with(&fixture, ConflictPolicy::Overwrite, MERGE_ONLY);
    assert_eq!(
        report.unconverted().len(),
        1,
        "alpha companion conflict is reported"
    );
    let [color] = report.merged_texture_artifacts() else {
        return Err("exactly the already committed colour receipt should remain".into());
    };
    assert_eq!(color.path, COLOR, "earlier colour commit is retained");
    assert_eq!(
        fixture.read(COLOR)?,
        dds.merged_bytes(),
        "raw fallback cannot replace colour"
    );
    assert_eq!(
        fixture.read(ALPHA)?,
        b"previous alpha DDS",
        "raw fallback cannot replace alpha"
    );
    assert_eq!(
        fixture.read(ALPHA_RECEIPT)?,
        b"foreign alpha companion",
        "unowned file survives"
    );
    assert!(
        !fixture.read(COLOR_RECEIPT)?.is_empty(),
        "valid colour receipt survives"
    );
    Ok(())
}

#[test]
fn archive_output_claims_prevent_raw_entries_from_overwriting_companions() -> TestResult {
    let raw_path = format!("{HEADER}.P4KX-ORIGIN.JSONL");
    for raw_first in [false, true] {
        let mut entries = texture_entries(&rock());
        let raw = TestEntry::stored(&raw_path, b"raw archive output".to_vec());
        if raw_first {
            entries.insert(0, raw);
        } else {
            entries.push(raw);
        }
        let fixture = Fixture::new(entries)?;
        let report = run_with(&fixture, ConflictPolicy::Overwrite, MERGE_ONLY);
        if raw_first {
            assert_eq!(
                report.unconverted().len(),
                1,
                "the earlier raw claim is authoritative"
            );
            assert!(
                report.merged_texture_artifacts().is_empty(),
                "the conflicting DDS lacks a receipt"
            );
            let raw_relative = raw_path.replace('\\', "/");
            assert_eq!(
                fixture.read(&raw_relative)?,
                b"raw archive output",
                "raw winner preserved"
            );
        } else {
            assert_eq!(
                report.failures().len(),
                1,
                "later raw claim collides with the receipt"
            );
            assert_eq!(
                report.merged_texture_artifacts().len(),
                1,
                "the earlier DDS pair remains"
            );
            assert_eq!(
                fixture.read(COLOR)?,
                rock().merged_bytes(),
                "merged DDS wins"
            );
        }
    }
    Ok(())
}

fn tailed() -> SplitDds {
    SplitDds::new(TestTextureFormat::LegacyBgra8, 16, 8)
        .split(2)
        .alpha(TestTextureFormat::R8)
}

fn rows(bytes: &[u8]) -> TestResult<Vec<Value>> {
    Ok(std::str::from_utf8(bytes)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?)
}

#[cfg(unix)]
#[test]
fn allowed_destination_symlink_is_replaced_without_writing_its_referent() -> TestResult {
    for policy in [ConflictPolicy::IfChanged, ConflictPolicy::Overwrite] {
        let fixture = Fixture::new(texture_entries(&rock()))?;
        let referent = fixture.target.join("old-referent.bin");
        fs::write(&referent, b"original link target")?;
        let output = linked_destination(&fixture, &referent)?;

        let kept = run_with(&fixture, ConflictPolicy::Skip, MERGE_ONLY);
        assert!(kept.is_complete(), "{kept:?}");
        assert!(
            kept.merged_texture_artifacts().is_empty(),
            "Skip issues no proof"
        );
        assert!(
            fs::symlink_metadata(&output)?.file_type().is_symlink(),
            "Skip keeps the link"
        );

        let replaced = run_with(&fixture, policy, MERGE_ONLY);
        assert!(replaced.is_complete(), "{replaced:?}");
        let [artifact] = replaced.merged_texture_artifacts() else {
            return Err("replacement should publish one DDS companion".into());
        };
        assert!(
            fs::symlink_metadata(&output)?.is_file(),
            "the replacement is a regular file"
        );
        let bytes = fixture.read(COLOR)?;
        assert_eq!(
            bytes,
            rock().merged_bytes(),
            "the link namespace now holds the DDS"
        );
        assert_eq!(
            fs::read(referent)?,
            b"original link target",
            "referent is byte-identical"
        );
        let mut digest = OriginDigestWriter::new(io::sink());
        digest.write_all(&bytes)?;
        let expected = digest.finish()?;
        assert_eq!(
            artifact.digest, expected,
            "the report describes the installed replacement"
        );
        let receipt = rows(&fixture.read(COLOR_RECEIPT)?)?;
        let binding = &receipt.first().ok_or("receipt header")?["binding"];
        assert_eq!(
            binding.get("sha256").and_then(Value::as_str),
            Some(expected.sha256()),
            "the sidecar binds the new DDS"
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn archive_destination_redirected_to_protected_source_is_never_replaced() -> TestResult {
    let fixture = Fixture::new(texture_entries(&rock()))?;
    let source = fixture.archive.path();
    let original = fs::read(source)?;
    let output = linked_destination(&fixture, source)?;

    let report = run_with(&fixture, ConflictPolicy::Overwrite, MERGE_ONLY);
    assert!(!report.is_complete(), "the source alias remains protected");
    assert!(
        report.merged_texture_artifacts().is_empty(),
        "no receipt for refused output"
    );
    assert!(
        fs::symlink_metadata(output)?.file_type().is_symlink(),
        "the protected link remains"
    );
    assert_eq!(
        fs::read(source)?,
        original,
        "the synthetic source bytes remain identical"
    );
    Ok(())
}

#[cfg(unix)]
fn linked_destination(fixture: &Fixture, referent: &Path) -> TestResult<PathBuf> {
    let output = fixture.target.join(COLOR);
    fs::create_dir_all(output.parent().ok_or("the destination has a parent")?)?;
    std::os::unix::fs::symlink(referent, &output)?;
    Ok(output)
}
