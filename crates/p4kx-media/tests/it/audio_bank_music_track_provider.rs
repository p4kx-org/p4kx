// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, test_key};
use serde_json::json;

use super::audio_bank_hirc_action_origins::{self as origins, Origins};
use super::support;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const BANK: &str = "music.bnk";
const TRACK: &str = "audio-bank-music-track";
const SOURCE: &str = "audio-bank-music-track-source";
const CLIP: &str = "audio-bank-music-track-clip";
const MEMBER: &str = "audio-bank-music-track-switch-member";
const TRACK_ID: u32 = 71;
const SOURCE_ID: u32 = 0xa48d_651b;
const STATE_ID: u32 = 0xa05e_b1e7;

#[test]
fn music_track_export_preserves_physical_sources_clips_and_state_members() -> TestResult {
    let folder = tempfile::tempdir()?;
    let plain = TestBank::new(1)
        .object(TestHircObject::music_track(TRACK_ID, &[SOURCE_ID]))
        .bytes();
    let encrypted = TestBank::new(1)
        .object(TestHircObject::music_state_track(
            TRACK_ID,
            &[SOURCE_ID, SOURCE_ID],
            31,
            &[STATE_ID, STATE_ID],
        ))
        .bytes();
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(BANK, plain.as_slice()))
        .entry(TestEntry::zstd(BANK, encrypted.as_slice()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let out = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &out, &options)?)?;
    let base = out.join(&index.artifact_base);
    let rows = support::records(&out, &index, "audio.jsonl")?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let evidence = Origins::read(&base)?;
    for (owner, original) in [&plain, &encrypted].into_iter().enumerate() {
        let (ordinal, track) = row(&rows, owner, TRACK, &[1, 0])?;
        let kind = if owner == 0 { 0 } else { 3 };
        assert_eq!(track.status, Status::Complete, "the track is decoded");
        assert_eq!(
            track.data.pointer("/payload/kind_code"),
            Some(&json!(kind)),
            "the physical track keeps its original kind code"
        );
        let raw = track.source.raw.as_ref().ok_or("retained original bank")?;
        assert_eq!(
            &fs::read(base.join(&raw.path))?,
            original,
            "the bank remains available byte for byte"
        );
        assert!(
            origins::has_field(
                &evidence.uses(ordinal, owner, "/data/payload/kind_code")?,
                "musicTrackKind"
            ),
            "the normalized kind comes from the actual original kind byte"
        );
        check_source_and_clip(&rows, &evidence, original, owner)?;
    }
    check_members(&rows, &evidence, &encrypted)?;
    for record in rows {
        assert!(
            schema.is_valid(&serde_json::to_value(record)?),
            "the emitted records obey the built-in schema"
        );
    }
    Ok(())
}

fn check_source_and_clip(
    rows: &[MediaRecord],
    evidence: &Origins,
    bytes: &[u8],
    owner: usize,
) -> TestResult {
    let (ordinal, source) = row(rows, owner, SOURCE, &[1, 0, 0])?;
    assert_eq!(
        source.data.pointer("/payload/source_id"),
        Some(&json!(SOURCE_ID)),
        "the source row keeps the original ID"
    );
    assert_eq!(
        source.data.pointer("/payload/storage_code"),
        Some(&json!(2)),
        "the source row keeps the original storage byte"
    );
    let (_, clip) = row(rows, owner, CLIP, &[1, 0, 0])?;
    assert_eq!(
        clip.data.pointer("/payload/begin_trim/raw_bits"),
        Some(&json!("3fe0000000000000")),
        "the clip row keeps the exact trim word"
    );
    let start = bytes
        .windows(4)
        .position(|value| value == SOURCE_ID.to_le_bytes())
        .ok_or("source word")?;
    let uses = evidence.uses(ordinal, owner, "/data/payload/source_id")?;
    assert!(
        origins::has_exact(&uses, "musicTrackSourceId", "payload", &(start..start + 4)),
        "the source receipt selects the original ID bytes"
    );
    check_clip_track_ids(rows, evidence, bytes, owner)
}

fn check_clip_track_ids(
    rows: &[MediaRecord],
    evidence: &Origins,
    bytes: &[u8],
    owner: usize,
) -> TestResult {
    for index in 0..=owner {
        let id = u32::try_from(index + 1)?;
        let (ordinal, clip) = row(rows, owner, CLIP, &[1, 0, u64::try_from(index)?])?;
        assert_eq!(
            clip.data.pointer("/payload/track_id"),
            Some(&json!(id)),
            "the clip retains its original track ID separately from the repeated media ID",
        );
        let authored = [
            id.to_le_bytes(),
            SOURCE_ID.to_le_bytes(),
            (SOURCE_ID ^ 0x5A5A_5A5A).to_le_bytes(),
            0_u32.to_le_bytes(),
        ];
        let authored = authored.as_flattened();
        let start = bytes
            .windows(authored.len())
            .position(|words| words == authored)
            .ok_or("original clip header")?;
        let uses = evidence.uses(ordinal, owner, "/data/payload/track_id")?;
        assert!(
            origins::has_exact(
                &uses,
                "musicTrackClipTrackId",
                "payload",
                &(start..start + size_of::<u32>())
            ),
            "the clip receipt selects its first original word",
        );
    }
    Ok(())
}

fn check_members(rows: &[MediaRecord], evidence: &Origins, bytes: &[u8]) -> TestResult {
    let start = bytes
        .windows(4)
        .position(|value| value == STATE_ID.to_le_bytes())
        .ok_or("first state member word")?;
    for (index, offset) in [start, start + 4].into_iter().enumerate() {
        let (ordinal, member) = row(rows, 1, MEMBER, &[1, 0, u64::try_from(index)?])?;
        assert_eq!(
            member.data.pointer("/payload/value_id"),
            Some(&json!(STATE_ID)),
            "the original repeated state member survives"
        );
        let uses = evidence.uses(ordinal, 1, "/data/payload/value_id")?;
        assert!(
            origins::has_exact(
                &uses,
                "musicTrackSwitchValueId",
                "payload",
                &(offset..offset + 4)
            ),
            "each member receipt selects its own original bytes"
        );
    }
    Ok(())
}

fn row<'a>(
    rows: &'a [MediaRecord],
    entry: usize,
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'a MediaRecord)> {
    rows.iter().enumerate().find(|(_, row)| {
        row.subresource.namespace == namespace && row.subresource.indices == indices
            && matches!(&row.source.identity, Identity::Entry { entry: owner, .. } if owner.entry_index == entry)
    }).ok_or_else(|| "required physical MusicTrack occurrence".into())
}
