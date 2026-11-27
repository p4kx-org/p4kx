// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    ArchiveBuilder, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase, test_key,
};

use serde_json::Value;
use tempfile::TempDir;

use super::values::Projection;
use super::{
    Archive, ArchiveWatch, Bank, Family, Inventory, ObjectView, TestResult, compare,
    inspect_archive, read_bank,
};

const BANK_ENTRY: &str = "Data/Sounds/wwise/witness.bnk";
const BANK_VERSION: u32 = 154;
const GROUP: u32 = 7;
const SEGMENT: u32 = 10;
const TRACK: u32 = 11;
const SWITCH: u32 = 12;
const PLAYLIST: u32 = 13;
const DIALOGUE: u32 = 15;

#[test]
fn wwise_witness_reconciles_all_populated_family_facets() -> TestResult {
    let children = [SEGMENT, SEGMENT];
    let mut bank = TestBank::new(1).version(BANK_VERSION);
    for (id, kind) in [
        (5, TestContainerKind::Sequence),
        (6, TestContainerKind::Switch),
        (9, TestContainerKind::Blend),
    ] {
        bank = bank.object(TestHircObject::container(
            kind,
            id,
            &children,
            TestNodeBase::Plain,
        ));
    }
    let bank = bank
        .object(TestHircObject::music_segment(SEGMENT, &[TRACK], &[]))
        .object(TestHircObject::music_switch_track(
            TRACK,
            &[100],
            GROUP,
            &[101],
        ))
        .object(TestHircObject::music_switch(
            SWITCH,
            &children,
            GROUP,
            &[SEGMENT, SEGMENT, 0],
        ))
        .object(TestHircObject::music_playlist(PLAYLIST, &[SEGMENT]))
        .object(dialogue());
    let older = TestBank::new(2).version(153).bytes();
    let inventory = witness(
        &ArchiveBuilder::new()
            .entry(TestEntry::stored("Data/ignored.dat", [1]))
            .entry(TestEntry::stored("Data/Sounds/wwise/older.bnk", older))
            .entry(TestEntry::stored(BANK_ENTRY, bank.bytes())),
    )?;
    assert_eq!(
        inventory.other_versions, 1,
        "the verified v153 bank is reported separately"
    );
    assert_eq!(
        inventory.compared_banks, 1,
        "only the v154 bank supplies evidence"
    );
    assert_eq!(
        inventory.skipped_size, 0,
        "both fixture banks fit the input allowance"
    );
    assert!(inventory.covered(), "all required families are witnessed");
    assert!(
        inventory.missing.is_empty(),
        "all twelve facets, including every stinger and rule owner: {:?}",
        inventory.missing,
    );
    Ok(())
}

#[test]
fn wwise_witness_accepts_compact_encrypted_decision_graph() -> TestResult {
    let bank = TestBank::new(1).version(BANK_VERSION).object(
        TestHircObject::music_switch_with_shared_layers(SWITCH, 16, SEGMENT),
    );
    let inventory = witness(
        &ArchiveBuilder::v2().entry(TestEntry::zstd(BANK_ENTRY, bank.bytes()).encrypted()),
    )?;
    for family in [
        Family::SwitchStinger,
        Family::SwitchTransition,
        Family::MusicDecision,
    ] {
        assert!(
            !inventory.missing.contains(&family),
            "encrypted bank witnesses {family:?}",
        );
    }
    assert!(
        !inventory.covered(),
        "one compact decision graph supplies only its own families",
    );
    Ok(())
}

#[test]
fn wwise_witness_distinguishes_empty_owner_and_uninspected_family() -> TestResult {
    let bank =
        TestBank::new(1)
            .version(BANK_VERSION)
            .object(TestHircObject::music_segment_with_stingers(
                SEGMENT,
                &[],
                &[],
            ));
    let inventory =
        witness(&ArchiveBuilder::new().entry(TestEntry::stored(BANK_ENTRY, bank.bytes())))?;
    assert!(
        inventory.missing.contains(&Family::SegmentStinger)
            && inventory.compared.contains(&Family::SegmentStinger),
        "a checked owner with an explicit zero does not witness populated coverage",
    );
    assert!(
        inventory.missing.contains(&Family::Dialogue)
            && !inventory.compared.contains(&Family::Dialogue),
        "an uninspected family remains distinct from a checked owner",
    );
    assert!(
        !inventory.covered(),
        "an empty family supplies no populated evidence"
    );
    Ok(())
}

#[test]
fn wwise_witness_rejects_retargeted_serialized_source_receipt() -> TestResult {
    let original = TestBank::new(1)
        .version(BANK_VERSION)
        .object(TestHircObject::music_segment(SEGMENT, &[TRACK], &[]));
    let fixture =
        ArchiveBuilder::v2().entry(TestEntry::zstd(BANK_ENTRY, original.bytes()).encrypted());
    let (_temporary, archive) = open_fixture(&fixture)?;
    let entry = archive.entries().first().ok_or("synthetic Wwise entry")?;
    let bank = read_bank(&archive, entry)?.ok_or("supported synthetic soundbank size")?;
    let source = bank
        .soundbank()
        .objects()
        .first()
        .ok_or("original stinger owner")?;
    let mut view = ObjectView::new(0, source);
    view.attach_source_fields(&bank);
    let fields = view
        .source_fields
        .ok_or("source projection of the stinger owner")?;
    let mut document = serde_json::to_value(fields)?;
    compare_stinger_document(&bank, &document)?;

    shift_trigger_receipt(&mut document)?;
    let Err(error) = compare_stinger_document(&bank, &document) else {
        return Err("witness accepted a receipt with a shifted physical start".into());
    };
    assert_eq!(
        error.to_string(),
        "datum receipt addresses the exact original byte range",
        "the mismatched span alone causes the comparison to fail",
    );
    Ok(())
}

fn shift_trigger_receipt(document: &mut Value) -> TestResult {
    let reference = document
        .pointer("/stingers/0/trigger_id/sourceUsesRefs/0")
        .and_then(Value::as_str)
        .ok_or("serialized stinger trigger receipt reference")?
        .to_owned();
    let uses = document
        .pointer_mut("/source_inventory/uses")
        .and_then(Value::as_array_mut)
        .ok_or("serialized uses inventory")?;
    let start = uses
        .iter_mut()
        .find(|used| used.get("id").and_then(Value::as_str) == Some(reference.as_str()))
        .and_then(|used| used.pointer_mut("/location/indices/0"))
        .ok_or("original trigger receipt start")?;
    let offset = start
        .as_str()
        .ok_or("original trigger byte offset")?
        .parse::<u64>()?;
    *start = Value::String((offset + 1).to_string());
    Ok(())
}

fn compare_stinger_document(bank: &Bank, document: &Value) -> TestResult {
    let original = bank
        .soundbank()
        .objects()
        .first()
        .ok_or("original stinger owner")?;
    let origin = bank.archive_origin().ok_or("physical stinger Entry")?;
    bank.soundbank()
        .with_hirc_source_fields(0, |_, semantic| {
            let projection = Projection::new(
                document,
                bank.soundbank().object_body(original),
                semantic.object().body_offset(),
                Some(true),
                origin.entry().ordinal(),
            )?;
            compare(
                original.kind,
                semantic,
                &projection,
                &mut Inventory::new().missing,
            )
        })?
        .ok_or("selected original stinger owner")?
}

fn open_fixture(builder: &ArchiveBuilder) -> TestResult<(TempDir, Archive)> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("Data.p4k");
    builder.write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    Ok((temporary, archive))
}

fn witness(builder: &ArchiveBuilder) -> TestResult<Inventory> {
    let (temporary, archive) = open_fixture(builder)?;
    let watch = ArchiveWatch::new(&archive);
    let inventory = inspect_archive(&archive, &watch)?;
    let mut contents = std::fs::read_dir(temporary.path())?;
    let only_file = contents
        .next()
        .transpose()?
        .ok_or("original synthetic archive")?;
    assert_eq!(
        only_file.path(),
        archive.path(),
        "the input remains the only artifact"
    );
    assert!(contents.next().is_none(), "the scan writes no export files");
    Ok(inventory)
}

fn dialogue() -> TestHircObject {
    const NODE_BYTES: u32 = 12;
    let mut body = vec![70];
    body.extend_from_slice(&1_u32.to_le_bytes());
    body.extend_from_slice(&GROUP.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&NODE_BYTES.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.extend_from_slice(&SEGMENT.to_le_bytes());
    body.extend_from_slice(&45_u16.to_le_bytes());
    body.extend_from_slice(&80_u16.to_le_bytes());
    body.extend([1, 0xFD]);
    body.extend_from_slice(&3.0_f32.to_le_bytes());
    body.extend([1, 0xFE]);
    body.extend_from_slice(&0.5_f32.to_le_bytes());
    body.extend_from_slice(&1.5_f32.to_le_bytes());
    TestHircObject::raw(15, DIALOGUE, body)
}
