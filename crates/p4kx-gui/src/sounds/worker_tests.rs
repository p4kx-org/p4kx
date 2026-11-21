// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;
use std::sync::mpsc;

use p4kx_extraction::{Archive, CancelToken, FieldOrigin, SourceLocation, SourceUseRole};
use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestCodec, TestEntry, TestHircObject, TestWem, test_key,
    wwise_short_id,
};

use super::provenance::ResolvedSoundList;
use super::view::Playback;
use super::worker::{Answer, Request, Worker, run};
use crate::EntryIndex;

type TestResult = Result<(), Box<dyn Error>>;

fn opened() -> Result<(tempfile::TempDir, Arc<EntryIndex>), Box<dyn Error>> {
    open_entries([definitions()])
}

const TRIGGER: &str = "Play_It";
const BANK_PATH: &str = "Data/Sounds/wwise/twin.bnk";
const MEDIA_ID: u32 = 301;
const PLAY_ACTION: u16 = 0x0403;
const TARGET_ID: u32 = 90;

fn definitions() -> TestEntry {
    TestEntry::stored(
        "Data\\Libs\\GameAudio\\SC_Audio.xml",
        TestAtl::new("SC_Audio")
            .trigger(TRIGGER, &[TRIGGER])
            .to_xml(),
    )
}

fn open_entries(
    entries: impl IntoIterator<Item = TestEntry>,
) -> Result<(tempfile::TempDir, Arc<EntryIndex>), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    let index = EntryIndex::build(Archive::open_with_key(&path, Some(test_key()))?);
    Ok((folder, Arc::new(index)))
}

#[test]
fn a_cancelled_worker_stops_indexing_and_answers_nothing() -> TestResult {
    let (_folder, index) = opened()?;
    let (_requests, waiting) = mpsc::channel::<Request>();
    let (answer, answers) = mpsc::channel::<Answer>();
    let cancel = CancelToken::new();
    cancel.cancel();

    run(&index, (&waiting, &answer), &cancel, &|| {});

    assert!(
        answers.try_recv().is_err(),
        "no answer from a cancelled build"
    );
    Ok(())
}

#[test]
fn an_uncancelled_worker_answers_that_the_index_is_ready() -> TestResult {
    let (_folder, index) = opened()?;
    let (requests, waiting) = mpsc::channel::<Request>();
    let (answer, answers) = mpsc::channel::<Answer>();
    drop(requests);

    run(&index, (&waiting, &answer), &CancelToken::new(), &|| {});

    assert!(
        matches!(answers.try_recv(), Ok(Answer::Ready(info)) if info.triggers == 1),
        "the index is ready"
    );
    Ok(())
}

#[test]
fn dropping_the_worker_cancels_its_indexing() -> TestResult {
    let (_folder, index) = opened()?;
    let worker = Worker::start(&index, Arc::new(|| {}))?;
    let token = worker.cancel_token();

    assert!(!token.is_cancelled(), "running");
    drop(worker);

    assert!(token.is_cancelled(), "stopped with its handle");
    Ok(())
}

fn duplicate_bank_index(
    selected: &[u8],
) -> Result<(tempfile::TempDir, Arc<EntryIndex>), Box<dyn Error>> {
    let decoy = TestBank::new(1)
        .wem(
            MEDIA_ID,
            TestWem::pcm(TestCodec::Pcm, 1, 8_000, &[1]).bytes(),
        )
        .bytes();
    open_entries([
        definitions(),
        TestEntry::stored(BANK_PATH, decoy),
        TestEntry::deflated(BANK_PATH, selected).encrypted(),
    ])
}

fn selected_bank(wem: Vec<u8>) -> Vec<u8> {
    TestBank::new(2)
        .object(TestHircObject::event(wwise_short_id(TRIGGER), &[10, 11]))
        .object(TestHircObject::action(10, PLAY_ACTION, 20))
        .object(TestHircObject::action(11, PLAY_ACTION, TARGET_ID))
        .object(TestHircObject::sound(20, MEDIA_ID, 0, 0))
        .wem(MEDIA_ID, wem)
        .bytes()
}

fn resolved(index: &EntryIndex) -> Result<Box<ResolvedSoundList>, Box<dyn Error>> {
    let (requests, waiting) = mpsc::channel();
    let (answer, answers) = mpsc::channel();
    requests.send(Request::Resolve(17, TRIGGER.to_owned()))?;
    drop(requests);
    run(index, (&waiting, &answer), &CancelToken::new(), &|| {});
    let Answer::Ready(_) = answers.try_recv()? else {
        return Err("audio index not ready".into());
    };
    let Answer::Sounds(17, result) = answers.try_recv()? else {
        return Err("resolution did not return".into());
    };
    result.map_err(Into::into)
}

#[test]
fn resolved_gui_sound_keeps_the_selected_duplicate_bank_and_field_origins() -> TestResult {
    let wem = TestWem::pcm(TestCodec::Pcm, 1, 16_000, &[3, 2, 1]).bytes();
    let len = u32::try_from(wem.len())?;
    let selected = selected_bank(wem);
    let (_folder, index) = duplicate_bank_index(&selected)?;
    let result = resolved(&index)?;
    assert_playback(&result, &index, len)?;
    let proof = result.provenance?;
    let [variant] = proof.variants() else {
        return Err("one variant receipt expected".into());
    };
    let [broken] = proof.breaks() else {
        return Err("one failed route expected".into());
    };
    assert_eq!(
        variant.event.contains_encrypted_source(),
        Some(false),
        "the ATL event is clear"
    );
    assert_eq!(
        variant.wem_id.contains_encrypted_source(),
        Some(true),
        "the terminal route is encrypted"
    );
    assert_eq!(
        variant.size.contains_encrypted_source(),
        Some(true),
        "the exact DIDX length belongs to the encrypted bank"
    );
    assert_original_word(
        &proof,
        &variant.size,
        (&selected, "bnkDidx", "dataLength"),
        SourceUseRole::Payload,
        len,
    )?;
    assert_original_word(
        &proof,
        &broken.reason,
        (&selected, "bnkHircAction", "targetId"),
        SourceUseRole::Selector,
        TARGET_ID,
    )
}

fn assert_playback(result: &ResolvedSoundList, index: &EntryIndex, len: u32) -> TestResult {
    let [variant] = result.list.variants.as_slice() else {
        return Err("one physical sound expected".into());
    };
    let Some(Playback::Embedded { bank, wem }) = variant.playback else {
        return Err("embedded target lost".into());
    };
    assert_eq!(wem, MEDIA_ID, "the actual selected media ID");
    assert_eq!(
        variant.size,
        u64::from(len),
        "the selected WEM's exact length"
    );
    assert_eq!(
        index.entry(bank).ok_or("physical target")?.index(),
        2,
        "the admitted bank and its unselected sibling share one path"
    );
    Ok(())
}

fn assert_original_word(
    proof: &super::SoundListProvenance,
    origin: &FieldOrigin,
    (bank, wanted_namespace, wanted_field): (&[u8], &str, &str),
    role: SourceUseRole,
    expected: u32,
) -> TestResult {
    for site in proof.inventory().iter_uses() {
        let SourceLocation::Subresource {
            namespace,
            field,
            indices,
        } = site.location()
        else {
            continue;
        };
        if namespace != wanted_namespace
            || field != wanted_field
            || site.role() != role
            || !origin.source_uses_refs().contains(site.id())
        {
            continue;
        }
        let [start, end, ..] = indices.as_slice() else {
            return Err("original byte range".into());
        };
        let bytes = bank
            .get(usize::try_from(*start)?..usize::try_from(*end)?)
            .ok_or("read original bank word")?;
        if u32::from_le_bytes(bytes.try_into()?) != expected {
            continue;
        }
        let source = proof
            .inventory()
            .iter_sources()
            .find(|source| source.id() == site.source())
            .ok_or("original physical source")?;
        let physical = serde_json::to_value(source)?;
        assert_eq!(
            physical
                .get("entryIndex")
                .and_then(serde_json::Value::as_u64),
            Some(2),
            "the receipt resolves to the same physical bank as playback"
        );
        return Ok(());
    }
    Err("selected original field not found".into())
}
