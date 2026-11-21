// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_assembly::audio::{
    ArchiveAudio, ExportLoopOrigins, ExportOptions, ExportWatch, ExportedSound, SoundFormat,
    WemExportSource, write_txtp_with_loop_origins,
};
use p4kx_extraction::{Archive, CancelToken, ConflictPolicy};
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};

use super::audio_txtp_support::{EMBEDDED, PLAY, TestResult, pcm, sound};

const EVENT: &str = "Play_IdenticalId";
const WEM: u32 = 7001;
const FIRST: &str = "Data/Sounds/wwise/First.bnk";
const SECOND: &str = "Data/Sounds/wwise/Second.bnk";

fn bank(name: &str, wem: &[u8], actions: &[u32]) -> Vec<u8> {
    TestBank::new(wwise_short_id(name))
        .wem(WEM, wem)
        .object(TestHircObject::event(wwise_short_id(EVENT), actions))
        .object(TestHircObject::action(1, PLAY, 2))
        .object(sound(2, WEM, EMBEDDED, wem.len()))
        .bytes()
}

#[test]
fn same_short_id_keeps_both_physical_media_and_binds_each_txtp_loop() -> TestResult {
    let first = pcm().smpl_loop(0, 12, 63).bytes();
    let second = pcm().smpl_loop(0, 21, 95).bytes();
    let root = tempfile::tempdir()?;
    let path = root.path().join("Data.p4k");
    let folder = root.path().join("out");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/SC.xml",
            TestAtl::new("SC").trigger(EVENT, &[EVENT]).to_cryxml(),
        ))
        .entry(TestEntry::stored(FIRST, bank("First", &first, &[1, 1])))
        .entry(TestEntry::zstd(SECOND, bank("Second", &second, &[1])).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let audio = ArchiveAudio::new(&archive);
    let sounds = audio.resolve_trigger(EVENT).ok_or("the trigger resolves")?;
    assert_eq!(
        sounds.variants().len(),
        3,
        "both banks and both first-bank actions"
    );

    let triggers = [&sounds];
    let (exported, origins) = audio.export_sounds_watched_with_loop_origins(
        &triggers,
        &folder,
        ExportOptions {
            format: SoundFormat::Raw,
            policy: ConflictPolicy::Overwrite,
        },
        &ExportWatch::default(),
    )?;
    assert!(
        exported.is_clean(),
        "the physical file plan succeeds: {exported:?}"
    );
    let [first_file, second_file] = exported.sounds.as_slice() else {
        return Err("two files for two physical sources".into());
    };
    assert_exported_wem(&archive, &origins, first_file, &first, FIRST)?;
    assert_exported_wem(&archive, &origins, second_file, &second, SECOND)?;

    let report = write_txtp_with_loop_origins(
        &archive,
        &triggers,
        &exported,
        &folder,
        ConflictPolicy::Overwrite,
        &CancelToken::new(),
        &origins,
    )?;
    let [file] = report.files.as_slice() else {
        return Err("one TXTP for the logical event".into());
    };
    let text = fs::read_to_string(&file.path)?;
    let commands: Vec<_> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.contains(" #I "))
        .collect();
    let first_name = first_file
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("first name")?;
    let second_name = second_file
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("second name")?;
    assert_eq!(
        commands,
        [
            format!("{first_name} #I 12 64"),
            format!("{first_name} #I 12 64"),
            format!("{second_name} #I 21 96"),
        ],
        "each authored repetition plays its own physical media and loop"
    );
    assert!(
        file.origin_receipt
            .as_ref()
            .is_some_and(|receipt| receipt.is_file()),
        "the TXTP loop receipt was published"
    );
    Ok(())
}

fn assert_exported_wem(
    archive: &Archive,
    origins: &ExportLoopOrigins,
    file: &ExportedSound,
    bytes: &[u8],
    owner: &str,
) -> TestResult {
    assert_eq!(file.wem_id, WEM, "keep the original short ID");
    assert_eq!(fs::read(&file.path)?, bytes, "the exact WEM for {owner}");
    let receipt = origins
        .for_sound(file)
        .ok_or("the WEM loop has an origin")?;
    let entry = receipt.entry(archive).ok_or("the source Entry is bound")?;
    assert_eq!(entry.entry().path(), owner, "bind the original bank");
    assert_eq!(
        entry.entry().is_encrypted(),
        owner == SECOND,
        "the receipt retains the source encryption"
    );
    assert_eq!(
        file.source,
        WemExportSource::Soundbank {
            entry: entry.entry().index(),
            offset: 0,
            length: u64::try_from(bytes.len())?,
        },
        "the source key names the exact bank and range"
    );
    Ok(())
}
