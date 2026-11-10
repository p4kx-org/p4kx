// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read as _;

use p4kx_assembly::audio::{ArchiveAudio, IndexedSoundbank};
use p4kx_audio::{HircValue, Soundbank};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const TRIGGER: &str = "Indexed_Bank_Stinger";
const EVENT: &str = "Indexed_Bank_Event";
const TRIGGER_ID: u32 = 0x77;
const HOLDER: u32 = 20;
const ACTION: u32 = 10;

struct Original {
    path: &'static str,
    id: u32,
    target: u32,
    track: u32,
    wem: u32,
    bytes: Vec<u8>,
}

fn original(first: bool, collide: bool) -> Original {
    let (path, id, wem) = if first {
        ("Data\\Sounds\\wwise\\A-prefix.bnk", 0xB0, 7001)
    } else {
        ("Data\\Sounds\\wwise\\Z-tail.bnk", 0xB1, 7002)
    };
    let target = if first || collide { 21 } else { 22 };
    let track = if first || collide { 31 } else { 32 };
    let mut bank = TestBank::new(id)
        .object(TestHircObject::music_segment_with_stingers(
            HOLDER,
            &[],
            &[(TRIGGER_ID, target), (TRIGGER_ID, target)],
        ))
        .object(TestHircObject::music_segment_with_stingers(
            target,
            &[track],
            &[],
        ))
        .object(TestHircObject::music_track(track, &[wem]));
    if !first {
        bank = bank
            .object(TestHircObject::event(
                p4kx_testkit::wwise_short_id(EVENT),
                &[ACTION],
            ))
            .object(TestHircObject::action(ACTION, 0x1B03, TRIGGER_ID));
    }
    Original {
        path,
        id,
        target,
        track,
        wem,
        bytes: bank.bytes(),
    }
}

#[test]
fn indexed_stinger_bank_with_distinct_object_ids_is_preserved() -> TestResult {
    resolves(false)
}

#[test]
fn indexed_stinger_bank_with_equal_object_ids_keeps_its_original_media() -> TestResult {
    resolves(true)
}

fn resolves(collide: bool) -> TestResult {
    let banks = [original(true, collide), original(false, collide)];
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("indexed-stinger-banks.p4k");
    let atl = TestAtl::new("Indexed").trigger(TRIGGER, &[EVENT]).to_xml();
    let atl_path = "Data\\Libs\\GameAudio\\indexed.xml";
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored(atl_path, atl.as_bytes()));
    for bank in &banks {
        builder = builder.entry(TestEntry::stored(bank.path, bank.bytes.as_slice()));
    }
    builder.write_to(&path)?;
    let archive = Archive::open(path)?;
    assert_bytes(&archive, atl_path, atl.as_bytes())?;
    for bank in &banks {
        assert_original(&archive, bank)?;
    }
    let audio = ArchiveAudio::new(&archive);
    let sounds = audio
        .resolve_trigger(TRIGGER)
        .ok_or("the actual indexed trigger")?;
    assert!(
        audio.index().issues().is_empty(),
        "both actual banks were admitted"
    );
    let names: Vec<_> = audio
        .index()
        .soundbanks()
        .iter()
        .map(IndexedSoundbank::file_name)
        .collect();
    assert_eq!(
        names,
        ["A-prefix.bnk", "Z-tail.bnk"],
        "original bank index order"
    );
    let actual: Vec<_> = sounds
        .variants()
        .iter()
        .map(|variant| (variant.wem_id(), variant.soundbank().file_name()))
        .collect();
    assert_eq!(
        actual,
        [(Some(7001), "A-prefix.bnk"), (Some(7002), "Z-tail.bnk")],
        "each indexed stinger selects its own bank despite equal local object IDs"
    );
    assert!(
        sounds.breaks().is_empty(),
        "both original stinger targets resolve"
    );
    Ok(())
}

fn assert_bytes(archive: &Archive, path: &str, expected: &[u8]) -> TestResult {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or("actual original Entry")?;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    assert_eq!(bytes, expected, "all original bytes before mapping");
    Ok(())
}

fn assert_original(archive: &Archive, expected: &Original) -> TestResult {
    assert_bytes(archive, expected.path, &expected.bytes)?;
    let bank = Soundbank::parse(expected.bytes.as_slice())?;
    assert_eq!(
        bank.header().id,
        expected.id,
        "distinct original physical bank ID"
    );
    assert!(bank.warnings().is_empty(), "genuine source layout");
    let holder = bank.object(HOLDER).ok_or("original stinger holder")?;
    let HircValue::MusicSegment(holder) = &holder.value else {
        return Err("actual original holder type".into());
    };
    let slots: Vec<_> = holder
        .node
        .stingers
        .iter()
        .map(|slot| (slot.trigger_id, slot.segment_id))
        .collect();
    assert_eq!(
        slots,
        [(TRIGGER_ID, expected.target); 2],
        "both authored equal tuples"
    );
    let segment = bank
        .object(expected.target)
        .ok_or("original target segment")?;
    let HircValue::MusicSegment(segment) = &segment.value else {
        return Err("actual original target type".into());
    };
    assert_eq!(
        segment.node.children,
        [expected.track],
        "original target-to-track edge"
    );
    assert!(segment.node.stingers.is_empty(), "no hidden target stinger");
    let track = bank.object(expected.track).ok_or("original music track")?;
    let HircValue::MusicTrack(track) = &track.value else {
        return Err("actual original track type".into());
    };
    let sources: Vec<_> = track
        .sources
        .iter()
        .map(|source| source.source_id)
        .collect();
    assert_eq!(
        sources,
        [expected.wem],
        "different genuine WEM in each original bank"
    );
    if expected.id == 0xB1 {
        assert_event(&bank)?;
    }
    Ok(())
}

fn assert_event(bank: &Soundbank<&[u8]>) -> TestResult {
    let event = bank
        .object(p4kx_testkit::wwise_short_id(EVENT))
        .ok_or("original Event")?;
    let HircValue::Event(actions) = &event.value else {
        return Err("actual Event type".into());
    };
    assert_eq!(actions, &[ACTION], "original Event-to-action edge");
    let action = bank.object(ACTION).ok_or("original Trigger action")?;
    let HircValue::Action(action) = &action.value else {
        return Err("actual action type".into());
    };
    assert!(action.is_trigger(), "actual Trigger action kind");
    assert_eq!(
        action.target_id, TRIGGER_ID,
        "actual indexed stinger trigger"
    );
    Ok(())
}

#[path = "stinger_cycle_sites.rs"]
mod cycle_sites;
