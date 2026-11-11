// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::json;

use super::support;

#[path = "audio_bank_hirc_action_fixture.rs"]
mod fixture;
#[path = "audio_bank_hirc_action_origins.rs"]
mod origins;
#[path = "audio_bank_hirc_action_partial.rs"]
mod partial;

use self::fixture::{Fixture, PLAY, Play, STOP, TARGET_ID};
use self::origins::{Origins, has_exact, has_field};

type ActionTestResult<T = ()> = Result<T, Box<dyn Error>>;

const SAME_NAME: &str = "actions.BNK";
const NAMESPACE: &str = "audio-bank-hirc-action";

#[test]
fn action_exports_original_play_bank_words_with_exact_routing_receipts() -> ActionTestResult {
    let folder = tempfile::tempdir()?;
    let first = Play {
        props: 1,
        ranged: 0,
        bank_id: 0,
        bank_type: 0xa1b2_c3d4,
    };
    let second = Play {
        props: 0,
        ranged: 2,
        bank_id: 0xf1e2_d3c4,
        bank_type: 0x7788_9900,
    };
    let fixtures = [
        fixture::fixture(154, PLAY, first)?,
        fixture::fixture(154, PLAY, second)?,
        fixture::fixture(153, PLAY, second)?,
        fixture::fixture(154, STOP, first)?,
    ];
    for fixture in &fixtures {
        fixture.verify_legacy_decoder()?;
    }
    assert_ne!(
        fixtures[0].sites.bank_type, fixtures[1].sites.bank_type,
        "property counts relocate the original PlayBank"
    );
    let archive = archive(folder.path(), &fixtures)?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let records = support::records(&output, &index, "audio.jsonl")?;
    let actions = records
        .iter()
        .enumerate()
        .filter(|(_, record)| record.subresource.namespace == NAMESPACE)
        .collect::<Vec<_>>();
    assert_eq!(
        actions.len(),
        fixtures.len(),
        "one semantic Action row per original envelope"
    );
    let origins = Origins::read(&output.join(&index.artifact_base))?;
    for (owner, ((record, row), fixture)) in actions.into_iter().zip(&fixtures).enumerate() {
        check_fields(row, owner, fixture);
        check_origins(&origins, record, owner, fixture)?;
    }
    Ok(())
}

fn archive(directory: &Path, fixtures: &[Fixture]) -> ActionTestResult<Archive> {
    let path = directory.join("Data.p4k");
    let mut archive = ArchiveBuilder::new();
    for (position, fixture) in fixtures.iter().enumerate() {
        let entry = if position == 1 {
            TestEntry::zstd(SAME_NAME, fixture.bytes.as_slice()).encrypted()
        } else {
            TestEntry::stored(SAME_NAME, fixture.bytes.as_slice())
        };
        archive = archive.entry(entry);
    }
    archive.write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn check_fields(row: &MediaRecord, owner: usize, fixture: &Fixture) {
    assert!(
        matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner),
        "semantic row is owned by this physical source"
    );
    assert_eq!(
        row.subresource.indices,
        [1, 0],
        "HIRC section and object ordinals"
    );
    assert_eq!(row.status, Status::Complete, "the Action layout is decoded");
    assert_eq!(
        row.data.get("record_type"),
        Some(&json!("bank-hirc-action")),
        "owned typed record"
    );
    assert_eq!(
        row.data.pointer("/payload/action_type"),
        Some(&json!(fixture.action_type)),
        "original action type"
    );
    assert_eq!(
        row.data.pointer("/payload/target_id"),
        Some(&json!(TARGET_ID)),
        "original target ID"
    );
    let bank = if fixture.selects_play_bank() {
        json!({"bank_id":fixture.play.bank_id,"bank_type":fixture.play.bank_type})
    } else {
        json!(null)
    };
    assert_eq!(
        row.data.pointer("/payload/play_bank"),
        Some(&bank),
        "physical PlayBank fields and branch absence"
    );
}

fn check_origins(
    origins: &Origins,
    record: usize,
    owner: usize,
    fixture: &Fixture,
) -> ActionTestResult {
    let original = &fixture.sites;
    for (path, name, range) in [
        (
            "/data/payload/action_type",
            "actionType",
            &original.action_type,
        ),
        ("/data/payload/target_id", "targetId", &original.target_id),
    ] {
        assert!(
            has_exact(&origins.uses(record, owner, path)?, name, "payload", range),
            "exact word for {path}"
        );
    }
    if fixture.selects_play_bank() {
        return check_play_origins(origins, record, owner, fixture);
    }
    let uses = origins.uses(record, owner, "/data/payload/play_bank")?;
    assert!(
        has_field(&uses, "version"),
        "actual BKHD version governs absent PlayBank"
    );
    assert_eq!(
        has_exact(&uses, "actionType", "selector", &original.action_type),
        fixture.version == 154,
        "only the applicable version evaluates the action type for PlayBank"
    );
    for unread in ["bankId", "bankType", "propertyCount", "rangedPropertyCount"] {
        assert!(
            !has_field(&uses, unread),
            "unconsulted decoy field {unread} does not contribute"
        );
    }
    Ok(())
}

fn check_play_origins(
    origins: &Origins,
    record: usize,
    owner: usize,
    fixture: &Fixture,
) -> ActionTestResult {
    let original = &fixture.sites;
    for (path, name, range) in [
        (
            "/data/payload/play_bank/bank_id",
            "bankId",
            &original.bank_id,
        ),
        (
            "/data/payload/play_bank/bank_type",
            "bankType",
            &original.bank_type,
        ),
    ] {
        let uses = origins.uses(record, owner, path)?;
        assert!(
            has_exact(&uses, name, "payload", range),
            "exact routed word for {path}"
        );
        assert!(
            has_exact(&uses, "propertyCount", "selector", &original.props),
            "actual property count selects the word"
        );
        assert!(
            has_exact(&uses, "rangedPropertyCount", "selector", &original.ranged),
            "actual ranged-property count selects the word"
        );
        assert!(
            has_field(&uses, "version"),
            "actual BKHD version selects PlayBank layout"
        );
    }
    Ok(())
}
