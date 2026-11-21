// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Checksum, Exclusion, Filter};
use p4kx_media::{ExportOptions, Identity, ProviderRegistry};
use p4kx_testkit::TestEntry;
use serde_json::json;

use super::required_input_support as input;
use super::support::{self, TestResult};

const PART: &[u8] = b"needed";

#[test]
fn excluded_individual_part_is_read_without_publishing_its_raw_bytes() -> TestResult {
    use sha2::{Digest as _, Sha256};

    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", PART),
        ],
    )?;
    let output = folder.path().join("media");
    let options = ExportOptions {
        exclude: Exclusion::parse(["*.dds.1"])?,
        ..ExportOptions::default()
    };
    let (index, visits) = input::run(&archive, &output, &options, 1, 1)?;
    assert_eq!(visits, vec![0], "required bytes create no target callback");
    assert_eq!(
        index.source_entries.enumerated, 2,
        "physical inputs count once"
    );
    let row = input::row(&output, &index)?;
    assert_eq!(
        row.pointer("/bytes/0"),
        Some(&json!(PART)),
        "actual required bytes"
    );
    assert_eq!(
        row.pointer("/reference/resolution/status"),
        Some(&json!("not-selected"))
    );
    assert_eq!(
        row.pointer("/inputs/0/omission"),
        Some(&json!("explicit-exclusion"))
    );
    assert_eq!(row.pointer("/inputs/0/source/raw"), Some(&json!(null)));
    let expected = Checksum::Sha256(Sha256::digest(PART).into()).hex();
    assert_eq!(row.pointer("/inputs/0/digest/hex"), Some(&json!(expected)));
    assert_eq!(
        row.pointer("/inputs/0/digest/scope"),
        Some(&json!("decoded-entry-payload"))
    );
    assert!(
        !output
            .join(&index.artifact_base)
            .join("raw")
            .join(&expected)
            .exists(),
        "excluded physical part gets no own published raw payload",
    );
    let sources = input::sources(&output, &index)?;
    assert_eq!(
        sources.len(),
        1,
        "only selected header has a retained-source receipt"
    );
    assert!(
        matches!(&sources[0].identity, Identity::Entry { entry, .. } if entry.entry_index == 0)
    );
    input::no_private_spool(&output, &index)?;
    Ok(())
}

#[test]
fn unmatched_required_part_can_retain_without_selection_or_callback_promotion() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", PART),
        ],
    )?;
    let options = ExportOptions {
        include: Some(Filter::parse("*.dds")?),
        ..ExportOptions::default()
    };
    let output = folder.path().join("media");
    let (index, visits) = input::run(&archive, &output, &options, 2, 1)?;
    let row = input::row(&output, &index)?;
    assert_eq!(
        visits,
        vec![0],
        "unmatched target stays outside normal callbacks"
    );
    assert_eq!(row.pointer("/inputs/0/omission"), Some(&json!(null)));
    assert!(
        row.pointer("/inputs/0/source/raw/path").is_some(),
        "usable retained bytes"
    );
    assert_eq!(
        row.pointer("/reference/resolution/status"),
        Some(&json!("not-selected"))
    );
    assert_eq!(
        input::sources(&output, &index)?.len(),
        2,
        "two physical retentions"
    );
    assert_eq!(
        (
            index.source_entries.enumerated,
            index.source_entries.written,
            index.source_entries.skipped
        ),
        (2, 1, 1),
        "read evidence does not promote normal selection",
    );
    Ok(())
}

#[test]
fn early_and_late_reuse_preserve_physical_order_and_charge_every_reader_pass() -> TestResult {
    for early in [false, true] {
        let folder = tempfile::tempdir()?;
        let mut entries = vec![
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", PART),
        ];
        if !early {
            entries.reverse();
        }
        let archive = support::archive_with(folder.path(), entries)?;
        let mut options = ExportOptions::default();
        options.limits.sources = 2;
        options.limits.source_bytes = 19;
        let output = folder.path().join("exact");
        let (index, visits) = input::run(&archive, &output, &options, 2, 1)?;
        assert_eq!(
            visits,
            vec![0, 1],
            "early reads do not reorder target callbacks"
        );
        assert_eq!(
            input::sources(&output, &index)?.len(),
            2,
            "one raw receipt per physical source"
        );
        assert_eq!(
            index.source_entries.enumerated, 2,
            "normal visit reuses early admission"
        );
        options.limits.source_bytes = 18;
        let error = input::run(&archive, &folder.path().join("short"), &options, 2, 1)
            .err()
            .ok_or("reader bytes were not charged")?;
        assert!(
            input::has_limit(error.as_ref(), "source_bytes"),
            "actual reread exhausts shared bytes: {error:?}"
        );
    }
    Ok(())
}

#[test]
fn released_private_inputs_charge_new_materialisation_without_double_source_admission() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", PART),
        ],
    )?;
    let mut options = ExportOptions {
        exclude: Exclusion::parse(["*.dds.1"])?,
        ..ExportOptions::default()
    };
    options.limits.sources = 2;
    options.limits.source_bytes = 25;
    let (index, _) = input::run(&archive, &folder.path().join("exact"), &options, 2, 1)?;
    assert_eq!(
        index.source_entries.enumerated, 2,
        "repeated private reads share physical admission"
    );
    options.limits.source_bytes = 24;
    let failure = input::run(&archive, &folder.path().join("short"), &options, 2, 1)
        .err()
        .ok_or("second private materialisation was free")?;
    assert!(
        input::has_limit(failure.as_ref(), "source_bytes"),
        "actual private work exhausts shared bytes: {failure:?}"
    );
    Ok(())
}

#[test]
fn ambiguous_members_have_no_input_token_and_excluded_containers_stay_unopened() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", b"first"),
            TestEntry::stored("owner.dds.1", b"second"),
            TestEntry::stored("blocked.socpak", b"invalid container"),
        ],
    )?;
    let options = ExportOptions {
        exclude: Exclusion::parse(["*.socpak"])?,
        ..ExportOptions::default()
    };
    let output = folder.path().join("media");
    let (index, visits) = input::run(&archive, &output, &options, 1, 1)?;
    let row = input::row(&output, &index)?;
    assert_eq!(visits, vec![0, 1, 2], "excluded container has no callback");
    assert_eq!(
        row.pointer("/reference/resolution/status"),
        Some(&json!("ambiguous"))
    );
    assert_eq!(row.get("inputs"), Some(&json!([])), "no first-wins token");
    assert!(
        fs::read(output.join("media-index.json")).is_ok(),
        "invalid excluded container was never opened"
    );
    Ok(())
}

#[test]
fn validated_skip_checks_additional_unmatched_raw_sources_without_promoting_tallies() -> TestResult
{
    use p4kx_extraction::ConflictPolicy;

    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", PART),
        ],
    )?;
    let output = folder.path().join("media");
    let mut options = ExportOptions {
        include: Some(Filter::parse("*.dds")?),
        ..ExportOptions::default()
    };
    let (index, visits) = input::run(&archive, &output, &options, 2, 1)?;
    assert_eq!(
        visits,
        vec![0],
        "unmatched retention adds no provider callback"
    );
    let sources = input::sources(&output, &index)?;
    assert_eq!(
        sources.len(),
        2,
        "selected header and required unmatched input retain usable bytes"
    );
    assert_eq!(
        index.source_entries.written, 1,
        "source-row count does not redefine normal written selection"
    );
    let previous = fs::read(output.join("media-index.json"))?;
    options.conflict = ConflictPolicy::Skip;
    let providers = assert_valid_skip(&archive, &output, &options, &previous)?;
    let retained = sources
        .iter()
        .find_map(|source| {
            if matches!(&source.identity, Identity::Entry { entry, .. } if entry.entry_index == 1) {
                source.raw.as_ref()
            } else {
                None
            }
        })
        .ok_or("actual unmatched raw receipt")?;
    fs::write(
        output.join(&index.artifact_base).join(&retained.path),
        b"broken",
    )?;
    assert!(
        p4kx_media::export_with(&archive, &output, &options, &providers).is_err(),
        "validated skip checks the unmatched artifact through the actual hash manifest",
    );
    assert_eq!(
        fs::read(output.join("media-index.json"))?,
        previous,
        "failed validation keeps the previous index bytes"
    );
    Ok(())
}

fn assert_valid_skip(
    archive: &Archive,
    output: &Path,
    options: &ExportOptions,
    previous: &[u8],
) -> Result<ProviderRegistry, Box<dyn std::error::Error>> {
    use std::sync::{Arc, Mutex};

    use p4kx_media::ExportOutcome;

    use super::required_input_support::InputProvider;

    let callbacks = Arc::new(Mutex::new(Vec::new()));
    let providers = ProviderRegistry::try_new(vec![Box::new(InputProvider {
        uses: 2,
        rereads: 1,
        prefix: None,
        probe: false,
        visits: Arc::clone(&callbacks),
    })])?;
    let outcome = p4kx_media::export_with(archive, output, options, &providers)?;
    assert!(
        matches!(outcome, ExportOutcome::Skipped { .. }),
        "additional unmatched raw source passes complete generation validation"
    );
    assert!(
        callbacks
            .lock()
            .map_err(|_| "visit lock poisoned")?
            .is_empty(),
        "validated skip schedules no provider work"
    );
    assert_eq!(
        fs::read(output.join("media-index.json"))?,
        previous,
        "validated skip preserves the complete index bytes"
    );
    Ok(providers)
}
