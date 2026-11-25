// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_audio::Wem;
use p4kx_extraction::{
    Archive, AudioError, Checksum, Codec, ConversionError, FileReceipt, OggFileReceipt, Sound,
};
use p4kx_testkit::{
    ArchiveBuilder, TestCodec, TestEntry, TestOpus, TestOpusFraming, TestVorbis, TestWem,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

type TestResult = Result<(), Box<dyn Error>>;

const ENTRY: &str = "Data\\Sounds\\origin.wem";

#[test]
fn vorbis_receipt_binds_written_pages_and_only_the_checked_codebook_prefix() -> TestResult {
    let dir = tempfile::tempdir()?;
    let fixture = TestVorbis::new(2, 24_000, &[false; 40]);
    let bytes = fixture.wem().bytes();
    let wem = Wem::parse(&bytes)?;
    let data = wem.metadata().data_chunk();
    let setup = u16::from_le_bytes(*wem.data().first_chunk::<2>().ok_or("setup size")?);
    let audio_start = data.body_offset + 2 + u64::from(setup);
    let prefix = fixture
        .packets()
        .iter()
        .take(17)
        .map(|p| 2 + p.len() as u64)
        .sum::<u64>();
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ENTRY, bytes.as_slice()))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let sound = Sound::from_archive_observed(&archive, ENTRY)?;
    let out = dir.path().join("result.ogg");
    let receipt = sound.write_ogg_with_receipt(&out)?;
    let rows = assert_payload(&sound, dir.path(), &out, &receipt)?;
    let setup_uses = field_uses(&rows, "commentsAndSetup")?;
    assert_location(
        &setup_uses,
        "codebookAudio",
        audio_start,
        audio_start + prefix,
        "selector",
    );
    assert!(
        !setup_uses
            .iter()
            .any(|item| location(item, "audioReplayed")),
        "the full audio replay contributes no setup bytes without a loop"
    );
    let audio_uses = field_uses(&rows, "audio")?;
    assert_location(
        &audio_uses,
        "audioReplayed",
        audio_start,
        data.body_end_exclusive(),
        "conversion",
    );
    assert_vorbis_summary(&receipt, audio_start, prefix)
}

fn assert_vorbis_summary(receipt: &OggFileReceipt, audio_start: u64, prefix: u64) -> TestResult {
    let summary = serde_json::to_value(receipt.summary_origins())?;
    let inventory = serde_json::to_value(receipt.summary_provenance()?)?;
    let codebooks = summary_uses(&summary, &inventory, "/codebooks")?;
    assert_location(
        &codebooks,
        "codebookAudio",
        audio_start,
        audio_start + prefix,
        "selector",
    );
    assert!(
        !codebooks.iter().any(|item| location(item, "audioReplayed")),
        "codebook selection stays separate from full replay"
    );
    let duration = summary_uses(&summary, &inventory, "/duration_seconds")?;
    assert!(
        duration.iter().any(|item| location(item, "sampleRate")),
        "duration uses the Vorbis rate"
    );
    assert!(
        duration.iter().any(|item| location(item, "audioReplayed")),
        "duration uses written frames"
    );
    assert_eq!(
        summary.pointer("/~1loop/sourceSetComplete"),
        Some(&json!(true)),
        "loop absence is recorded"
    );
    Ok(())
}

#[test]
fn switch_opus_identification_binds_first_packet_words_and_unobserved_receipts_stay_unknown()
-> TestResult {
    let dir = tempfile::tempdir()?;
    let mut data = vec![0; 4];
    data.extend_from_slice(&2_u32.to_be_bytes());
    data.extend_from_slice(&[0, 0, 0, 0, 0xfb, 2]);
    let bytes = TestOpus::new(TestOpusFraming::Switch, 1, 9_600)
        .declared_sample_count(480)
        .wem()?
        .data(data)
        .bytes();
    let input = dir.path().join("input.wem");
    fs::write(&input, &bytes)?;
    let first = Wem::parse(&bytes)?.metadata().data_chunk().body_offset + 4;
    let sound = Sound::from_file_observed(&input)?;
    let out = dir.path().join("result.ogg");
    let observed = sound.write_ogg_with_receipt(&out)?;
    let rows = assert_payload(&sound, dir.path(), &out, &observed)?;
    let identification = field_uses(&rows, "identification")?;
    assert_location(
        &identification,
        "switchFirstPacketSize",
        first,
        first + 4,
        "selector",
    );
    assert_location(
        &identification,
        "switchFirstDurationHeader",
        first + 8,
        first + 10,
        "conversion",
    );
    assert!(
        !identification
            .iter()
            .any(|item| location(item, "audioReplayed")),
        "only the first packet's timing words contribute to identification"
    );
    assert_opus_rate(&observed)?;
    let unobserved = Sound::from_file(&input)?.write_ogg_with_receipt(&out)?;
    assert_eq!(
        observed.report, unobserved.report,
        "artifact replacement uses the same report"
    );
    let unknown = serde_json::to_value(unobserved.summary_origins())?;
    assert_eq!(
        unknown.pointer("/~1codebooks/sourceSetComplete"),
        Some(&json!(false)),
        "unobserved absence"
    );
    assert_eq!(
        serde_json::to_value(unobserved.summary_provenance()?)?,
        json!({"sources":[],"uses":[]}),
        "unobserved source inventory"
    );
    Ok(())
}

fn assert_opus_rate(observed: &OggFileReceipt) -> TestResult {
    let observed_json = serde_json::to_value(observed.summary_origins())?;
    let provenance = serde_json::to_value(observed.summary_provenance()?)?;
    let rate = summary_uses(&observed_json, &provenance, "/sample_rate")?;
    assert!(
        rate.iter().any(|item| location(item, "formatTag")),
        "48 kHz follows from the codec"
    );
    assert!(
        rate.iter()
            .all(|item| item.get("role") != Some(&json!("payload"))),
        "no authored payload rate"
    );
    Ok(())
}

#[test]
fn foreign_companions_and_unsupported_ogg_leave_existing_files_intact() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("input.wem");
    let out = dir.path().join("result.ogg");
    let companion = FileReceipt::new(&out, None, None)?;
    fs::write(
        &input,
        TestVorbis::new(1, 24_000, &[false; 5]).wem().bytes(),
    )?;
    fs::write(&out, b"incumbent")?;
    fs::write(companion.path(), b"foreign")?;
    let sound = Sound::from_file_observed(&input)?;
    let rejected = sound.write_ogg_with_receipt(&out).err();
    assert!(
        matches!(rejected, Some(ConversionError::Write { .. })),
        "{rejected:?}"
    );
    let aliases = sound.write_ogg_with_receipt(&input).err();
    assert!(
        matches!(aliases, Some(ConversionError::OutputIsInput { .. })),
        "{aliases:?}"
    );

    fs::write(
        &input,
        TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &[0; 4]).bytes(),
    )?;
    let unsupported = Sound::from_file_observed(&input)?
        .write_ogg_with_receipt(&out)
        .err();
    assert!(
        matches!(
            unsupported,
            Some(ConversionError::Audio {
                source: AudioError::UnsupportedOggCodec(Codec::Pcm),
                ..
            })
        ),
        "{unsupported:?}"
    );
    assert_eq!(
        fs::read(out)?,
        b"incumbent",
        "failed overwrite leaves original artifact"
    );
    assert_eq!(
        fs::read(companion.path())?,
        b"foreign",
        "foreign companion preserved"
    );
    Ok(())
}

fn assert_payload(
    sound: &Sound,
    dir: &Path,
    out: &Path,
    receipt: &OggFileReceipt,
) -> Result<Vec<Value>, Box<dyn Error>> {
    let ordinary = dir.join("ordinary.ogg");
    let report = sound.write_ogg_to_file(&ordinary)?;
    let bytes = fs::read(out)?;
    assert_eq!(report, receipt.report, "identical ordinary report");
    assert_eq!(bytes, fs::read(ordinary)?, "identical complete payload");
    let rows = fs::read_to_string(&receipt.companion)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let schema = schemars::schema_for!(p4kx_extraction::OriginCompanionRow<'static>).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    for row in &rows {
        validator
            .validate(row)
            .map_err(|error| format!("{error}: {row}"))?;
    }
    let header = rows.first().ok_or("binding")?;
    assert_eq!(
        header.pointer("/binding/byteLength"),
        Some(&json!(bytes.len().to_string())),
        "reported byte length"
    );
    assert_eq!(
        header.pointer("/binding/sha256"),
        Some(&json!(
            Checksum::Sha256(Sha256::digest(&bytes).into()).hex()
        )),
        "reported payload digest"
    );
    assert_output_addresses(&rows, &bytes)?;
    Ok(rows)
}

fn assert_output_addresses(rows: &[Value], bytes: &[u8]) -> TestResult {
    let mut fields = rows.iter().filter_map(|row| row.get("address"));
    let mut offset = 0;
    for expected in ["identification", "commentsAndSetup", "audio"] {
        let span = fields.next().ok_or("one typed page-group origin")?;
        assert_eq!(span.get("type"), Some(&json!("oggBytes")), "Ogg address");
        assert_eq!(span.get("field"), Some(&json!(expected)), "addressed group");
        assert_eq!(
            span.get("start"),
            Some(&json!(offset.to_string())),
            "contiguous address"
        );
        let end: usize = span
            .get("endExclusive")
            .and_then(Value::as_str)
            .ok_or("group end")?
            .parse()?;
        let mut group = bytes.get(offset..end).ok_or("bytes in addressed group")?;
        assert!(
            !group.is_empty(),
            "each range has at least one complete page"
        );
        while !group.is_empty() {
            group = skip_page(group, expected)?;
        }
        offset = end;
    }
    assert!(fields.next().is_none(), "exactly three groups");
    assert_eq!(
        offset,
        bytes.len(),
        "groups cover complete output through EOS"
    );
    Ok(())
}

fn skip_page<'a>(bytes: &'a [u8], field: &str) -> Result<&'a [u8], Box<dyn Error>> {
    let header = bytes.get(..27).ok_or("Ogg page header")?;
    assert!(header.starts_with(b"OggS"), "address starts at a page");
    let count = usize::from(*header.last().ok_or("Ogg lacing count")?);
    let lacing = bytes.get(27..27 + count).ok_or("Ogg page lacing")?;
    let end = 27 + count + lacing.iter().map(|&size| usize::from(size)).sum::<usize>();
    let rest = bytes
        .get(end..)
        .ok_or("complete page within addressed group")?;
    if field == "audio" && rest.is_empty() {
        assert!(
            header.get(5).is_some_and(|flags| flags & 4 != 0),
            "audio ends with EOS"
        );
    }
    Ok(rest)
}

fn location(source_use: &Value, field: &str) -> bool {
    source_use
        .pointer("/location/field")
        .and_then(Value::as_str)
        == Some(field)
}

fn assert_location(uses: &[&Value], field: &str, start: u64, end: u64, role: &str) {
    let span = json!([start.to_string(), end.to_string()]);
    assert!(
        uses.iter().any(|item| {
            location(item, field)
                && item.pointer("/location/indices") == Some(&span)
                && item.get("role").and_then(Value::as_str) == Some(role)
        }),
        "original {field} range {start}..{end} with {role}: {uses:?}",
    );
}

fn field_uses<'a>(rows: &'a [Value], group: &str) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let row = rows
        .iter()
        .find(|row| row.pointer("/address/field").and_then(Value::as_str) == Some(group))
        .ok_or("addressed Ogg group")?;
    assert_eq!(
        row.get("sourceSetComplete"),
        Some(&json!(true)),
        "observed group"
    );
    resolve_uses(row, &rows.iter().filter_map(|row| row.get("sourceUse")))
}

fn summary_uses<'a>(
    summary: &Value,
    inventory: &'a Value,
    field: &str,
) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let origin = summary.get(field).ok_or("addressed Ogg summary field")?;
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(true)),
        "{field}"
    );
    let uses = inventory
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("summary uses")?;
    resolve_uses(origin, &uses.iter())
}

fn resolve_uses<'a>(
    origin: &Value,
    source_uses: &(impl Iterator<Item = &'a Value> + Clone),
) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let refs = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("own source references")?;
    refs.iter()
        .map(|id| {
            source_uses
                .clone()
                .find(|item| item.get("id") == Some(id))
                .ok_or_else(|| "unresolved source use".into())
        })
        .collect()
}
