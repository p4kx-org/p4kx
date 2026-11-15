// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;

use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestCodec, TestContainerKind, TestEntry, TestHircObject,
    TestNodeBase, TestWem, wwise_short_id,
};
use serde_json::Value;

use super::super::super::STREAMED;
use super::{
    BANK_PATH, CONTAINER, EMBEDDED, EVENT, FIRST_SOUND, FIRST_WEM, Fixture, ID_BYTES, PLAY,
    SECOND_SOUND, SECOND_WEM, TXTP, TXTP_FILE, TestResult, check_receipt, decimal, export, pcm,
    selection_origin, selection_uses,
};

const LOOSE_PATH: &str = "Data/Sounds/wwise/Media/100.wem";
const OUTSIDE_PATH: &str = "Data/Sounds/wwise/Media/102.wem";
const OUTSIDE_SOUND: u32 = 22;
const OUTSIDE_WEM: u32 = 102;
const DECOY_WEM: u32 = 99;
const CHUNK_HEADER_BYTES: usize = 2 * ID_BYTES;
const DIDX_TRIPLET_BYTES: usize = 3 * ID_BYTES;

#[test]
fn selection_attests_to_the_wems_used_by_its_written_wavs() -> TestResult {
    let loose = pcm();
    let embedded = TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &[0, 1, 2, 3]).bytes();
    let bank = bank(&embedded);
    let fixture = fixture(&bank, &loose)?;
    let report = export(&fixture, &[])?;
    let file = report.pointer(TXTP_FILE).ok_or("written TXTP file")?;
    let origin = selection_origin(file)?;
    let text = check_receipt(&fixture.out, file, origin)?;
    check_written_commands(&fixture, &text);
    assert_eq!(
        origin["containsEncryptedSource"], true,
        "an encrypted WEM contributes within the selection"
    );
    assert_eq!(
        origin["sourceSetComplete"], false,
        "media proof leaves other route dependencies incomplete"
    );
    assert_eq!(
        media_uses(file, origin)?,
        original_media_uses(&bank, loose.len(), &embedded)?,
        "only the two rendered alternatives contribute WEM and bank media sites"
    );
    check_retained_audio(&fixture, file, &text)
}

fn check_written_commands(fixture: &Fixture, text: &str) {
    for id in [FIRST_WEM, SECOND_WEM, OUTSIDE_WEM] {
        let wav = format!("{EVENT}_{id}.wav");
        assert!(
            text.lines().any(|line| line.trim() == wav),
            "TXTP uses the WAV {wav}"
        );
        assert!(fixture.out.join(&wav).is_file(), "the WAV {wav} is emitted");
    }
    assert!(
        text.lines().any(|line| line.trim() == "group = -R2>1"),
        "the nested group selects between its two alternatives"
    );
}

fn check_retained_audio(fixture: &Fixture, file: &Value, text: &str) -> TestResult {
    let receipt = file["originReceipt"].as_str().ok_or("original receipt")?;
    fs::remove_file(fixture.out.join(receipt))?;
    fs::remove_file(fixture.out.join(TXTP))?;
    let report = export(fixture, &["--conflict", "if-changed"])?;
    assert_eq!(
        report.get("kept").and_then(Value::as_u64),
        Some(3),
        "all WAVs are retained"
    );
    let file = report.pointer(TXTP_FILE).ok_or("rewritten TXTP file")?;
    let origin = selection_origin(file)?;
    assert_eq!(file["kept"], false, "the TXTP is written again");
    assert_eq!(
        check_receipt(&fixture.out, file, origin)?,
        text,
        "the rewritten TXTP carries the same commands"
    );
    assert_eq!(
        origin["containsEncryptedSource"],
        Value::Null,
        "kept audio provides no proof of an encrypted contributor"
    );
    assert_eq!(
        origin["sourceSetComplete"], false,
        "kept audio leaves source evidence incomplete"
    );
    assert!(media_uses(file, origin)?.is_empty(), "no fresh WAV proof");
    assert!(
        selection_uses(file, origin)?.next().transpose()?.is_some(),
        "the fresh choice still has exact topology evidence"
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MediaUse<'a> {
    path: &'a str,
    namespace: &'a str,
    field: &'a str,
    indices: Vec<usize>,
}

fn media_uses<'a>(
    file: &'a Value,
    origin: &'a Value,
) -> Result<BTreeSet<MediaUse<'a>>, Box<dyn Error>> {
    let mut media = BTreeSet::new();
    for observation in selection_uses(file, origin)? {
        let observation = observation?;
        if !observation.is_media() {
            continue;
        }
        let site = observation.site;
        let path = observation.path()?;
        assert_eq!(site["role"], "selector", "the site chooses the WEM source");
        assert_eq!(site["scope"], "payload", "the site is in the decoded entry");
        let encryption = if path == BANK_PATH {
            "unencrypted"
        } else {
            "encrypted"
        };
        assert_eq!(
            observation.physical["storedEncryption"], encryption,
            "physical encryption for {path}"
        );
        let location = &site["location"];
        assert!(
            media.insert(MediaUse {
                path,
                namespace: location["namespace"].as_str().ok_or("media namespace")?,
                field: location["field"].as_str().ok_or("media field")?,
                indices: location["indices"]
                    .as_array()
                    .ok_or("media coordinates")?
                    .iter()
                    .map(decimal)
                    .collect::<Result<_, _>>()?,
            }),
            "each contributing site is listed once"
        );
    }
    Ok(media)
}

fn original_media_uses(
    bank: &[u8],
    loose_len: usize,
    embedded: &[u8],
) -> Result<BTreeSet<MediaUse<'static>>, Box<dyn Error>> {
    let selected_triplet = chunk_body(bank, *b"DIDX")? + DIDX_TRIPLET_BYTES;
    let id = bank
        .get(selected_triplet..selected_triplet + ID_BYTES)
        .ok_or("second DIDX entry")?;
    assert_eq!(id, SECOND_WEM.to_le_bytes(), "the actual media ID word");
    let bank_data = chunk_body(bank, *b"DATA")?;
    let offset_word = selected_triplet + ID_BYTES;
    let offset = bank
        .get(offset_word..offset_word + ID_BYTES)
        .ok_or("second bank media offset")?;
    let start = bank_data + usize::try_from(u32::from_le_bytes(offset.try_into()?))?;
    let end = start + embedded.len();
    assert_eq!(
        bank.get(start..end),
        Some(embedded),
        "the selected WEM slice"
    );
    let mut result = BTreeSet::from([
        MediaUse {
            path: LOOSE_PATH,
            namespace: "wemBytes",
            field: "mediaPayload",
            indices: vec![0, loose_len],
        },
        MediaUse {
            path: BANK_PATH,
            namespace: "wemBytes",
            field: "mediaPayload",
            indices: vec![start, end],
        },
    ]);
    for (namespace, index, fields) in [
        (
            "bnkDidx",
            selected_triplet,
            ["mediaId", "dataOffset", "dataLength"].as_slice(),
        ),
        ("bnkSection", bank_data, ["tag", "bodyLength"].as_slice()),
    ] {
        for &field in fields {
            result.insert(MediaUse {
                path: BANK_PATH,
                namespace,
                field,
                indices: vec![index],
            });
        }
    }
    Ok(result)
}

fn chunk_body(bank: &[u8], tag: [u8; ID_BYTES]) -> Result<usize, Box<dyn Error>> {
    bank.windows(ID_BYTES)
        .position(|window| window == tag)
        .map(|start| start + CHUNK_HEADER_BYTES)
        .ok_or_else(|| format!("fixture lacks {}", String::from_utf8_lossy(&tag)).into())
}

fn fixture(bank: &[u8], loose: &[u8]) -> Result<Fixture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let out = directory.path().join("out");
    let definitions = TestAtl::new("SC_Audio").trigger(EVENT, &[EVENT]);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data/Libs/GameAudio/SC_Audio.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::stored(BANK_PATH, bank))
        .entry(TestEntry::zstd(LOOSE_PATH, loose).encrypted())
        .entry(TestEntry::zstd(OUTSIDE_PATH, loose).encrypted())
        .write_to(&archive)?;
    Ok(Fixture {
        _dir: directory,
        archive,
        out,
    })
}

fn bank(embedded: &[u8]) -> Vec<u8> {
    TestBank::new(wwise_short_id("Topology"))
        .version(154)
        .wem(DECOY_WEM, pcm())
        .wem(SECOND_WEM, embedded)
        .object(TestHircObject::event(wwise_short_id(EVENT), &[1, 2]))
        .object(TestHircObject::action(1, PLAY, CONTAINER))
        .object(TestHircObject::action(2, PLAY, OUTSIDE_SOUND))
        .object(TestHircObject::container(
            TestContainerKind::Random,
            CONTAINER,
            &[FIRST_SOUND, SECOND_SOUND],
            TestNodeBase::Plain,
        ))
        .object(TestHircObject::sound(FIRST_SOUND, FIRST_WEM, STREAMED, 0))
        .object(TestHircObject::sound(SECOND_SOUND, SECOND_WEM, EMBEDDED, 0))
        .object(TestHircObject::sound(
            OUTSIDE_SOUND,
            OUTSIDE_WEM,
            STREAMED,
            0,
        ))
        .bytes()
}
