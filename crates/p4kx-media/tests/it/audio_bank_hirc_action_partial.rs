// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::ConflictPolicy;
use p4kx_media::{ExportOptions, ExportOutcome, Index, MediaError, MediaKind, Status};
use p4kx_testkit::TestEntry;
use serde_json::json;

use super::super::{layout_health, support};
use super::{ActionTestResult, NAMESPACE, SAME_NAME, fixture, origins};

#[test]
fn truncated_action_retains_the_read_prefix_and_prior_complete_index() -> ActionTestResult {
    let folder = tempfile::tempdir()?;
    let good = folder.path().join("good");
    let bad = folder.path().join("bad");
    fs::create_dir(&good)?;
    fs::create_dir(&bad)?;
    let play = fixture::Play {
        props: 1,
        ranged: 0,
        bank_id: 0,
        bank_type: 93,
    };
    let valid = fixture::fixture(154, fixture::PLAY, play)?;
    let prefix = fixture::without_bank_type(play)?;
    let good = support::archive_with(
        &good,
        [TestEntry::stored(SAME_NAME, valid.bytes.as_slice())],
    )?;
    let bad = support::archive_with(
        &bad,
        [TestEntry::stored(SAME_NAME, prefix.bytes.as_slice())],
    )?;
    let output = folder.path().join("out");
    let index_path = output.join("media-index.json");
    let options = ExportOptions {
        kinds: vec![MediaKind::Audio],
        conflict: ConflictPolicy::Overwrite,
        ..ExportOptions::default()
    };
    support::published(p4kx_media::export(&good, &output, &options)?)?;
    let committed = fs::read(&index_path)?;
    require_strict_failure(&bad, &output, &options)?;
    let options = ExportOptions {
        diagnostic: true,
        ..options
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&bad, &output, &options)?
    else {
        return Err("the malformed action has an incomplete diagnostic generation".into());
    };
    assert_eq!(
        fs::read(&index_path)?,
        committed,
        "the complete index is still current"
    );
    require_readable_prefix(
        diagnostic.parent().ok_or("diagnostic root")?,
        &index,
        &prefix,
    )
}

fn require_strict_failure(
    archive: &p4kx_extraction::Archive,
    output: &Path,
    options: &ExportOptions,
) -> ActionTestResult {
    let error = p4kx_media::export(archive, output, options)
        .err()
        .ok_or("truncated Action cannot publish normally")?;
    assert!(
        matches!(
            layout_health::leaf(&error),
            MediaError::ProjectionLoss {
                kind: MediaKind::Audio,
                ..
            }
        ),
        "the physical parser's failure is diagnosed as Action projection loss",
    );
    Ok(())
}

fn require_readable_prefix(
    generation: &Path,
    index: &Index,
    fixture: &fixture::Fixture,
) -> ActionTestResult {
    let rows = support::records(generation, index, "audio.jsonl")?;
    let (position, action) = rows
        .iter()
        .enumerate()
        .find(|(_, row)| row.subresource.namespace == NAMESPACE)
        .ok_or("semantic Action prefix row")?;
    assert_eq!(action.status, Status::Partial, "partial Action coverage");
    assert_eq!(
        action.data,
        json!({"record_type":"bank-hirc-action","payload":{
            "layout":"malformed","action_type":fixture::PLAY,"target_id":fixture::TARGET_ID,
            "play_bank":{"bank_id":0,"bank_type":null}
        }}),
        "the readable prefix and the missing bank type have distinct states",
    );
    let base = generation.join(&index.artifact_base);
    let origins = origins::Origins::read(&base)?;
    let id = origins.uses(position, 0, "/data/payload/play_bank/bank_id")?;
    assert!(
        origins::has_exact(&id, "bankId", "payload", &fixture.sites.bank_id),
        "the bank ID is the actually read word"
    );
    let missing = origins.uses(position, 0, "/data/payload/play_bank/bank_type")?;
    assert!(
        !origins::has_field(&missing, "bankType"),
        "no receipt for an unread bank type word"
    );
    let raw = action.source.raw.as_ref().ok_or("retained exact BNK")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        fixture.bytes,
        "the exact original body is preserved"
    );
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let mut json = serde_json::to_value(action)?;
    assert!(
        validator.is_valid(&json),
        "the actual diagnostic obeys the built-in schema"
    );
    *json
        .pointer_mut("/data/payload/layout")
        .ok_or("Action layout discriminator")? = json!("decoded");
    assert!(
        !validator.is_valid(&json),
        "a missing PlayBank word cannot be advertised as decoded"
    );
    Ok(())
}
