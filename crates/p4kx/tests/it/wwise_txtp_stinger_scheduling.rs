// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::ops::Range;

use p4kx_extraction::Soundbank;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};
use serde_json::{Value, json};

use super::{Fixture, PLAY, TestResult, extract, hirc_payload, pcm};

const TRIGGER_ACTION: u16 = 0x1B03;
const WORD: usize = size_of::<u32>();
const DECLARATION_BANK: &str = "Data/Sounds/wwise/Declaration.bnk";
const COMMAND_BANK: &str = "Data/Sounds/wwise/Commands.bnk";
const DEFERRED_EVENT: &str = "Play_Stinger_Next_Beat";
const IMMEDIATE_EVENT: &str = "Play_Stinger_Immediate";
const UNKNOWN_EVENT: &str = "Play_Stinger_Unknown_Sync";
const DIRECT_EVENT: &str = "Play_Declaring_Segment";
const CODE: &str = "omit-music-stinger-scheduling";
const OWNER: u32 = 10;
const DUPLICATE_OWNER: u32 = 11;
const TARGET: u32 = 12;
const TRACK: u32 = 13;
const MEDIA: u32 = 4_010;
const SCHEDULED: u32 = 0x6000_0010;
const IMMEDIATE: u32 = 0x6000_0011;
const UNKNOWN: u32 = 0x6000_0012;
const SYNC_IMMEDIATE: u32 = 0;
const SYNC_NEXT_GRID: u32 = 1;
const SYNC_NEXT_BEAT: u32 = 3;
const SYNC_UNKNOWN: u32 = u32::MAX;

#[test]
fn txtp_stinger_uses_the_first_declaration_in_the_selected_bank() -> TestResult {
    let declaration = declaration_bank()?;
    let fixture = fixture(&declaration)?;
    let report = export(&fixture, DEFERRED_EVENT, true)?;
    let bank = Soundbank::parse(declaration.as_slice())?;
    let owner = bank.object(OWNER).ok_or("first declaring music object")?;
    let duplicate = bank
        .object(DUPLICATE_OWNER)
        .ok_or("duplicate declaration")?;
    let owner_row = row_range(bank.object_body(owner), SCHEDULED, SYNC_NEXT_BEAT)?;
    let duplicate_row = row_range(bank.object_body(duplicate), SCHEDULED, SYNC_NEXT_GRID)?;
    let trigger = owner.id_byte_range().start + owner_row.start;
    let duplicate_trigger = duplicate.id_byte_range().start + duplicate_row.start;
    assert_ne!(trigger, duplicate_trigger, "distinct physical rows");
    let segment = trigger + WORD;
    let sync = segment + WORD;
    check_sources(
        report.pointer("/txtp/files/0").ok_or("stinger TXTP file")?,
        [
            (
                "bnkHircMusicNode",
                "musicStingerSyncPlayAt",
                "conversion",
                sync..sync + WORD,
            ),
            (
                "bnkHircMusicNode",
                "musicStingerScheduleTriggerId",
                "selector",
                trigger..segment,
            ),
            (
                "bnkHircMusicNode",
                "musicStingerScheduleSegmentId",
                "selector",
                segment..sync,
            ),
            (
                "bnkHircObject",
                "objectId",
                "selector",
                owner.id_byte_range(),
            ),
            (
                "bnkBkhd",
                "version",
                "selector",
                bank.version_byte_range().ok_or("original BKHD version")?,
            ),
        ],
    )
}

#[test]
fn txtp_stinger_diagnostics_require_a_known_sync_on_the_selected_trigger() -> TestResult {
    let declaration = declaration_bank()?;
    let fixture = fixture(&declaration)?;
    for event in [IMMEDIATE_EVENT, UNKNOWN_EVENT, DIRECT_EVENT] {
        export(&fixture, event, false)?;
    }
    Ok(())
}

fn export(fixture: &Fixture, event: &str, expected: bool) -> Result<Value, Box<dyn Error>> {
    let report = extract(fixture, event, &["--format", "wav", "--txtp"], 0)?;
    let file = report.pointer("/txtp/files/0").ok_or("written TXTP file")?;
    assert_eq!(
        file.get("approximations")
            .and_then(Value::as_array)
            .is_some_and(|codes| codes.iter().any(|code| code.as_str() == Some(CODE))),
        expected,
        "only the selected known non-immediate stinger declaration: {event}"
    );
    let text = fs::read_to_string(fixture.out.join(format!("{event}.txtp")))?;
    assert_eq!(
        text.lines()
            .any(|line| line.strip_prefix("# approximation: ") == Some(CODE)),
        expected,
        "header agrees with JSON for {event}",
    );
    let commands: Vec<_> = text.lines().filter(|line| !line.starts_with('#')).collect();
    assert_eq!(
        commands,
        [format!("{event}.wav")],
        "one unchanged playback command"
    );
    Ok(report)
}

fn check_sources<const N: usize>(
    file: &Value,
    expected: [(&str, &str, &str, Range<usize>); N],
) -> TestResult {
    let references = diagnostic_references(file)?;
    let uses = file
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("uses")?;
    let sources = file
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("sources")?;
    assert_eq!(
        references.len(),
        N,
        "every contributor is a checked original field"
    );
    for (namespace, field, role, range) in expected {
        let source_use = uses
            .iter()
            .find(|word| {
                word.pointer("/location/field").and_then(Value::as_str) == Some(field)
                    && word.get("id").is_some_and(|id| references.contains(id))
            })
            .ok_or_else(|| format!("reference for {field}"))?;
        assert_eq!(
            source_use.get("role").and_then(Value::as_str),
            Some(role),
            "exact role of {field}"
        );
        assert_eq!(
            source_use
                .pointer("/location/namespace")
                .and_then(Value::as_str),
            Some(namespace),
            "namespace for {field}"
        );
        assert_eq!(
            source_use.pointer("/location/indices"),
            Some(&json!([range.start.to_string(), range.end.to_string()])),
            "exact declaring word for {field}"
        );
        let source = sources
            .iter()
            .find(|source| source.get("id") == source_use.get("source"))
            .ok_or("physical declaration Entry")?;
        assert_eq!(
            source.pointer("/path/value").and_then(Value::as_str),
            Some(DECLARATION_BANK),
            "{field} belongs to the original declaration bank"
        );
    }
    Ok(())
}

fn diagnostic_references(file: &Value) -> Result<&[Value], &'static str> {
    let index = file
        .get("approximations")
        .and_then(Value::as_array)
        .and_then(|codes| codes.iter().position(|code| code.as_str() == Some(CODE)))
        .ok_or("stinger code index")?;
    let origin = file
        .get("fieldOrigins")
        .and_then(|fields| fields.get(format!("/approximations/{index}")))
        .ok_or("stinger diagnostic origin")?;
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "complete source proof"
    );
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(false),
        "plain declaration bank"
    );
    origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or("origin references")
}

fn fixture(declaration: &[u8]) -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    let mut definitions = TestAtl::new("Stingers").trigger(DIRECT_EVENT, &[DIRECT_EVENT]);
    let mut command = TestBank::new(wwise_short_id("Commands")).version(154);
    for (action, (event, trigger)) in (1..).zip([
        (DEFERRED_EVENT, SCHEDULED),
        (IMMEDIATE_EVENT, IMMEDIATE),
        (UNKNOWN_EVENT, UNKNOWN),
    ]) {
        definitions = definitions.trigger(event, &[event]);
        command = command
            .object(TestHircObject::event(wwise_short_id(event), &[action]))
            .object(TestHircObject::action(action, TRIGGER_ACTION, trigger));
    }
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data/Libs/GameAudio/Stingers.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::stored(COMMAND_BANK, command.bytes()))
        .entry(TestEntry::stored(DECLARATION_BANK, declaration))
        .entry(TestEntry::stored(
            format!("Data/Sounds/wwise/Media/{MEDIA}.wem"),
            pcm(),
        ))
        .write_to(&archive)?;
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}

fn declaration_bank() -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(TestBank::new(wwise_short_id("Declaration"))
        .version(154)
        .object(TestHircObject::event(wwise_short_id(DIRECT_EVENT), &[1]))
        .object(TestHircObject::action(1, PLAY, OWNER))
        .object(declares(
            OWNER,
            &[TRACK],
            [SYNC_NEXT_BEAT, SYNC_IMMEDIATE, SYNC_UNKNOWN],
        )?)
        .object(declares(DUPLICATE_OWNER, &[], [SYNC_NEXT_GRID; 3])?)
        .object(TestHircObject::music_segment_with_stingers(
            TARGET,
            &[TRACK],
            &[],
        ))
        .object(TestHircObject::music_track(TRACK, &[MEDIA]))
        .bytes())
}

fn declares(id: u32, children: &[u32], syncs: [u32; 3]) -> Result<TestHircObject, Box<dyn Error>> {
    let triggers = [IMMEDIATE, UNKNOWN, SCHEDULED];
    let mut payload = hirc_payload(TestHircObject::music_segment_with_stingers(
        id,
        children,
        &triggers.map(|trigger| (trigger, TARGET)),
    ))?;
    for (trigger, sync) in [SCHEDULED, IMMEDIATE, UNKNOWN].into_iter().zip(syncs) {
        let start = row_range(&payload, trigger, SYNC_IMMEDIATE)?.start + 2 * WORD;
        payload
            .get_mut(start..start + WORD)
            .ok_or("original stinger sync word")?
            .copy_from_slice(&sync.to_le_bytes());
    }
    Ok(TestHircObject::raw(10, id, payload))
}

fn row_range(payload: &[u8], trigger: u32, sync: u32) -> Result<Range<usize>, &'static str> {
    let row = [trigger, TARGET, sync, 0, 0, 0].map(u32::to_le_bytes);
    let row = row.as_flattened();
    let mut positions = payload
        .windows(row.len())
        .enumerate()
        .filter_map(|(start, bytes)| (bytes == row).then_some(start..start + row.len()));
    let range = positions.next().ok_or("checked stinger row")?;
    assert!(positions.next().is_none(), "one physical stinger row");
    Ok(range)
}
