// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::Wem;
use p4kx_extraction::sound::WemSource;
use p4kx_media::{AudioOutput, ExportOptions};
use p4kx_testkit::{TestBank, TestOpus, TestOpusFraming};
use serde_json::Value;

use super::{
    AUDIO_STREAM, Origins, Output, TestResult, check_artifact, check_request, check_schema,
    encrypted_bank, has_opus_site, options, support,
};

#[test]
fn opus_ogg_origins_bind_switch_timing_and_embedded_header_pages() -> TestResult {
    let mut data = vec![0; 4];
    data.extend_from_slice(&2_u32.to_be_bytes());
    data.extend_from_slice(&[0, 0, 0, 0, 0xfb, 2]);
    let switch = TestOpus::new(TestOpusFraming::Switch, 1, 9_600)
        .declared_sample_count(480)
        .wem()?
        .data(data)
        .bytes();
    let first = Wem::parse(&switch)?.metadata().data_chunk().body_offset + 4;
    check_precise_origins(
        &switch,
        &[
            ("switchFirstPacketSize", first..first + 4, "selector"),
            (
                "switchFirstDurationHeader",
                first + 8..first + 10,
                "conversion",
            ),
        ],
        &[],
    )?;

    let embedded = TestOpus::new(TestOpusFraming::Ogg, 2, 9_600).wem()?.bytes();
    let mut source = WemSource::open(std::io::Cursor::new(&embedded), embedded.len() as u64)?;
    let mut inspected = source.inspect()?;
    let plan = inspected.opus_ogg()?;
    let pages = plan
        .ranges()
        .embedded_header_pages()
        .ok_or("selected embedded Opus headers")?;
    check_precise_origins(
        &embedded,
        &[(
            "embeddedIdentificationPages",
            pages.identification.clone(),
            "conversion",
        )],
        &[("embeddedCommentPages", pages.comments.clone(), "conversion")],
    )
}

type OpusSite<'a> = (&'a str, Range<u64>, &'a str);

fn check_precise_origins(
    input: &[u8],
    timing: &[OpusSite<'_>],
    comments: &[OpusSite<'_>],
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let bank = TestBank::new(7).wem(49, input.to_vec()).bytes();
    let archive = encrypted_bank(folder.path(), &bank)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            audio_output: AudioOutput::Ogg,
            ..options()
        },
    )?)?;
    let records = support::records(&output, &index, AUDIO_STREAM)?;
    check_schema(&records)?;
    let (ordinal, record) = records
        .iter()
        .enumerate()
        .find(|(_, row)| row.subresource.namespace == "audio-bank-wem-conversion")
        .ok_or("embedded Opus conversion row")?;
    let row = check_request(record, Output::Ogg)?;
    let base = output.join(&index.artifact_base);
    check_artifact(&base, row, input, Output::Ogg)?;
    let origins = Origins::read(&base)?;
    let artifact = origins.for_field(
        ordinal,
        1,
        "/data/payload/outcome/payload/artifact/digest/hex",
    )?;
    let frames = origins.for_field(ordinal, 1, "/data/payload/outcome/payload/sample_count")?;
    for (field, extent, role) in timing {
        assert!(
            has_opus_site(&artifact, field, extent, role),
            "artifact uses {field}"
        );
        assert!(
            has_opus_site(&frames, field, extent, role),
            "frame count uses {field}"
        );
    }
    for (field, extent, role) in comments {
        assert!(
            has_opus_site(&artifact, field, extent, role),
            "artifact uses {field}"
        );
        assert!(
            !has_opus_site(&frames, field, extent, role),
            "frame count is independent of {field}"
        );
    }
    check_opus_rate_origin(&origins, ordinal)
}

fn check_opus_rate_origin(origins: &Origins, ordinal: usize) -> TestResult {
    let rate = origins.for_field(ordinal, 1, "/data/payload/outcome/payload/sample_rate")?;
    assert!(
        rate.iter().any(
            |entry| entry.pointer("/location/field").and_then(Value::as_str) == Some("formatTag")
        ),
        "the Opus codec selects the output rate"
    );
    assert!(
        rate.iter().all(
            |entry| entry.pointer("/location/field").and_then(Value::as_str) != Some("sampleRate")
        ),
        "the original declared rate does not set the Opus remux rate"
    );
    Ok(())
}
