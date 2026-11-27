// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Cursor, Write};
use std::path::Path;

use lewton::inside_ogg::OggStreamReader;
use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};
use serde_json::{Value, json};

use super::{Fixture, PLAY, STREAMED, TestResult, count, extract, fixture, names, pcm, xma};

pub(super) fn assert_sound_receipt(file: &Value, address: &str) -> TestResult {
    let rows = sound_receipt(file)?;
    assert!(
        rows.iter().any(|row| {
            row.pointer("/address/type").and_then(Value::as_str) == Some(address)
                && row["containsEncryptedSource"] == false
                && row["sourceSetComplete"] == true
                && row["sourceUsesRefs"]
                    .as_array()
                    .is_some_and(|uses| !uses.is_empty())
        }),
        "plain Archive input is cited for {address}"
    );
    Ok(())
}

fn sound_receipt(file: &Value) -> Result<Vec<Value>, Box<dyn Error>> {
    assert_eq!(
        file.get("origin_receipt_failure"),
        Some(&Value::Null),
        "companion publication completed"
    );
    let output = Path::new(file["file"].as_str().ok_or("exported sound path")?);
    let receipt = Path::new(
        file["origin_receipt"]
            .as_str()
            .ok_or("sound receipt path")?,
    );
    let leaf = output.file_name().ok_or("sound file name")?;
    let mut receipt_name = leaf.to_os_string();
    receipt_name.push(".p4kx-origin.jsonl");
    assert_eq!(
        fs::canonicalize(receipt)?,
        fs::canonicalize(output.with_file_name(receipt_name))?,
        "the report identifies the canonical sibling"
    );

    let rows = fs::read_to_string(receipt)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(&fs::read(output)?)?;
    let digest = measured.finish()?;
    let binding = rows
        .first()
        .ok_or("sound receipt header")?
        .get("binding")
        .ok_or("sound binding")?;
    assert_eq!(
        binding["sha256"],
        digest.sha256(),
        "the companion binds the exact committed bytes"
    );
    assert_eq!(
        binding["byteLength"],
        digest.byte_length().to_string(),
        "the binding covers the whole file"
    );
    assert_eq!(
        rows.last().ok_or("sound receipt end")?["type"],
        "end",
        "the companion was written completely"
    );
    Ok(rows)
}

const ATL_PATH: &str = "Data\\Libs\\GameAudio\\SC_ExtractOrigin.xml";
const EVENT: &str = "Emit_ExtractOrigin";
const TRIGGER: &str = "Play_ExtractOrigin";
const MEDIA: u32 = 321;

fn fixture_with_encrypted_atl(xml: &str) -> Result<Fixture, Box<dyn Error>> {
    let bank = TestBank::new(wwise_short_id("ExtractOrigin"))
        .object(TestHircObject::event(wwise_short_id(EVENT), &[1]))
        .object(TestHircObject::action(1, PLAY, 2))
        .object(TestHircObject::sound(2, MEDIA, STREAMED, 0));
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd(ATL_PATH, xml.as_bytes()).encrypted())
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\ExtractOrigin.bnk",
            bank.bytes(),
        ))
        .entry(TestEntry::stored(
            format!("Data\\Sounds\\wwise\\Media\\{MEDIA}.wem"),
            pcm(),
        ))
        .write_to(&archive)?;
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}

#[test]
fn extract_json_distinguishes_encrypted_trigger_from_plain_waveform() -> TestResult {
    let xml = TestAtl::new("SC_ExtractOrigin")
        .trigger(TRIGGER, &[EVENT])
        .to_xml();
    let fixture = fixture_with_encrypted_atl(&xml)?;
    let report = extract(&fixture, TRIGGER, &["--format", "wav"], 0)?;

    assert_eq!(
        report.pointer("/trigger/events/0/id"),
        Some(&json!(wwise_short_id(EVENT))),
        "the original event still resolves to the same ID"
    );
    assert_eq!(
        count(&report, "breaks"),
        Some(0),
        "the sound fully resolves"
    );
    assert_eq!(count(&report, "written"), Some(1), "one waveform committed");
    check_trigger_origin(&report, &xml)?;
    let fields = report.get("fieldOrigins").ok_or("file report origins")?;
    for field in ["/files/0/trigger", "/files/0/wem_id"] {
        let origin = fields.get(field).ok_or("file selection origin")?;
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&json!(true)),
            "{field} retains the encrypted trigger selection"
        );
    }
    check_size_origin(
        &report,
        "/files/0/size",
        &format!("Data\\Sounds\\wwise\\Media\\{MEDIA}.wem"),
    )?;

    let file = report.pointer("/files/0").ok_or("reported waveform")?;
    let rows = sound_receipt(file)?;
    let binding = rows.first().ok_or("artifact binding")?;
    let length = binding
        .pointer("/binding/byteLength")
        .and_then(Value::as_str)
        .ok_or("bound waveform length")?
        .parse::<u64>()?;
    assert_eq!(
        count(file, "size"),
        Some(length),
        "report matches the written byte count"
    );
    assert!(
        rows.iter().any(|row| {
            row.pointer("/address/field").and_then(Value::as_str) == Some("pcmSamples")
                && row.get("containsEncryptedSource") == Some(&json!(false))
                && row.get("sourceSetComplete") == Some(&json!(true))
                && row
                    .get("sourceUsesRefs")
                    .and_then(Value::as_array)
                    .is_some_and(|uses| !uses.is_empty())
        }),
        "the clear loose WEM is the waveform sample source"
    );
    Ok(())
}

pub(super) fn check_size_origin(report: &Value, field: &str, path: &str) -> TestResult {
    let origin = report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(field))
        .ok_or("written size origin")?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(false)),
        "{field} uses the clear WEM"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(true)),
        "the written size contributor set is complete"
    );
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("size uses")?;
    assert!(
        !references.is_empty(),
        "the written size cites original inputs"
    );
    let inventory = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("use inventory")?;
    let sources = report
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    for reference in references {
        let used = inventory
            .iter()
            .find(|used| used.get("id") == Some(reference))
            .ok_or("size source use")?;
        let id = used.get("source").ok_or("size physical source")?;
        let source = sources
            .iter()
            .find(|source| source.get("id") == Some(id))
            .ok_or("actual size source")?;
        assert_eq!(
            source.pointer("/path/value").and_then(Value::as_str),
            Some(path),
            "the byte length uses the observed WEM, even when the trigger is encrypted"
        );
    }
    Ok(())
}

fn check_trigger_origin(report: &Value, xml: &str) -> TestResult {
    let fields = report.get("fieldOrigins").ok_or("extract field origins")?;
    let origin = fields
        .get("/trigger/events/0/id")
        .ok_or("event ID origin")?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the event lookup uses the encrypted ATL"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(true)),
        "the event lookup has all original operands"
    );
    let breaks = fields
        .get("/breaks")
        .ok_or("zero broken links retain origin")?;
    assert!(
        breaks.get("containsEncryptedSource").is_some(),
        "zero has an immediate marker"
    );

    let attribute = format!(r#"wwise_name="{EVENT}""#);
    let start = xml.find(&attribute).ok_or("authored event name")?;
    let end = start + attribute.len();
    let indices = json!([start.to_string(), end.to_string()]);
    check_event_uses(report, origin, &indices)
}

fn check_event_uses(report: &Value, origin: &Value, indices: &Value) -> TestResult {
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("event source-use references")?;
    let inventory = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("original use inventory")?;
    let sources = report
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("original source inventory")?;
    let mut exact_name = false;
    for reference in references {
        let used = inventory
            .iter()
            .find(|value| value.get("id") == Some(reference))
            .ok_or("referenced original use")?;
        let id = used.get("source").ok_or("physical source reference")?;
        let source = sources
            .iter()
            .find(|source| source.get("id") == Some(id))
            .ok_or("referenced physical source")?;
        assert_eq!(
            source.pointer("/path/value").and_then(Value::as_str),
            Some(ATL_PATH),
            "the trigger ID operands belong to the original ATL"
        );
        exact_name |= used.get("role").and_then(Value::as_str) == Some("conversion")
            && used.pointer("/location/namespace").and_then(Value::as_str)
                == Some("textXmlAttribute")
            && used.pointer("/location/field").and_then(Value::as_str) == Some("wwise_name")
            && used.pointer("/location/indices") == Some(indices);
    }
    assert!(
        exact_name,
        "the ID hashes the exact original event attribute"
    );
    Ok(())
}

#[test]
fn a_trigger_with_several_sounds_is_named_by_trigger_and_wem() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "play_multi", &[], 1)?;
    assert_eq!(
        names(&fixture.out)?,
        [
            "Play_Multi_100.ogg",
            "Play_Multi_100.ogg.p4kx-origin.jsonl",
            "Play_Multi_101.wav",
            "Play_Multi_101.wav.p4kx-origin.jsonl",
            "Play_Multi_102.wem",
            "Play_Multi_102.wem.p4kx-origin.jsonl",
            "Play_Multi_103.wav",
            "Play_Multi_103.wav.p4kx-origin.jsonl",
        ]
    );
    assert_eq!(report["format"], "ogg");
    assert_eq!(count(&report, "written"), Some(4));
    assert_eq!(count(&report, "unconverted"), Some(1));
    assert_eq!(count(&report, "failed"), Some(1));
    let failure = report["failures"][0]["wem_id"].as_u64();
    assert_eq!(failure, Some(104));
    for (file, kind) in report["files"]
        .as_array()
        .ok_or("reported sound files")?
        .iter()
        .zip(["oggBytes", "wavBytes", "opaquePayload", "wavBytes"])
    {
        assert_sound_receipt(file, kind)?;
    }
    let ogg = fs::read(fixture.out.join("Play_Multi_100.ogg"))?;
    OggStreamReader::new(Cursor::new(ogg))?;
    assert_eq!(fs::read(fixture.out.join("Play_Multi_102.wem"))?, xma());
    let wav = fs::read(fixture.out.join("Play_Multi_101.wav"))?;
    assert_eq!(wav.get(..4), Some(&b"RIFF"[..]));
    Ok(())
}

#[test]
fn raw_keeps_the_wem_bytes() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "Play_Single", &["--format", "raw"], 0)?;
    assert_eq!(report["format"], "raw");
    assert_eq!(report["files"][0]["format"], "wem");
    assert_eq!(fs::read(fixture.out.join("Play_Single.wem"))?, pcm());
    Ok(())
}

#[test]
fn conflict_policies_decide_about_existing_files() -> TestResult {
    let fixture = fixture()?;
    extract(&fixture, "Play_Single", &[], 0)?;
    let target = fixture.out.join("Play_Single.wav");
    let receipt = fixture.out.join("Play_Single.wav.p4kx-origin.jsonl");
    let previous = fs::read(&receipt)?;
    fs::write(&target, b"old")?;
    let skipped = extract(&fixture, "Play_Single", &[], 0)?;
    assert_eq!(count(&skipped, "kept"), Some(1));
    assert_eq!(count(&skipped["files"][0], "size"), Some(3));
    let size = skipped
        .get("fieldOrigins")
        .and_then(|fields| fields.get("/files/0/size"))
        .ok_or("kept size origin")?;
    assert_eq!(
        size.get("containsEncryptedSource"),
        Some(&Value::Null),
        "the incumbent has unknown origin"
    );
    assert_eq!(
        size.get("sourceSetComplete"),
        Some(&json!(false)),
        "the incumbent source set is incomplete"
    );
    assert_eq!(
        size.get("sourceUsesRefs"),
        Some(&json!([])),
        "the incumbent does not inherit the attempted WEM use"
    );
    assert_eq!(
        skipped["files"][0].get("origin_receipt"),
        Some(&Value::Null)
    );
    assert_eq!(fs::read(&target)?, b"old");
    assert_eq!(fs::read(&receipt)?, previous);
    let replaced = extract(&fixture, "Play_Single", &["--conflict", "if-changed"], 0)?;
    assert_eq!(count(&replaced, "written"), Some(1));
    assert_sound_receipt(&replaced["files"][0], "wavBytes")?;
    let published = fs::read(&receipt)?;
    let unchanged = extract(&fixture, "Play_Single", &["--conflict", "if-changed"], 0)?;
    assert_eq!(count(&unchanged, "kept"), Some(1));
    assert_eq!(
        unchanged["files"][0].get("origin_receipt"),
        Some(&Value::Null)
    );
    assert_eq!(fs::read(&receipt)?, published);
    fs::write(&target, b"old")?;
    let overwritten = extract(&fixture, "Play_Single", &["--conflict", "overwrite"], 0)?;
    assert_eq!(count(&overwritten, "written"), Some(1));
    assert_ne!(fs::read(&target)?, b"old");
    Ok(())
}
