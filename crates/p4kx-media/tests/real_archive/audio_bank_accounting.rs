// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SectionId, Soundbank};
use p4kx_media::MediaRecord;
use serde_json::json;

use super::{TestResult, at, range, require};

const DIDX_TRIPLET_BYTES: usize = 12;

pub(super) fn verify(input: &[u8], rows: &[MediaRecord]) -> TestResult {
    let bank = Soundbank::parse(input)?;
    let original = bank.sections().collect::<Vec<_>>();
    let actual = at(rows, "audio-bank-section").collect::<Vec<_>>();
    require(
        original.len() == actual.len(),
        "complete soundbank section directory",
    )?;
    for (ordinal, (section, row)) in original.into_iter().zip(actual).enumerate() {
        let ordinal = u64::try_from(ordinal)?;
        require(
            row.subresource.indices == [ordinal],
            "original bank section position",
        )?;
        require(
            row.data.pointer("/payload/tag") == Some(&json!(section.id.0)),
            "original bank section tag",
        )?;
        require(
            range(row, "/payload/body_range", input)? == section.body,
            "original bank section body",
        )?;
        require(
            range(row, "/payload/header_range", input)?.starts_with(&section.id.0),
            "original bank section header",
        )?;
        match section.id {
            SectionId::HIRC => verify_hirc(&bank, input, rows, ordinal)?,
            SectionId::DIDX => verify_didx(section.body, input, rows, ordinal)?,
            _ => {}
        }
    }
    Ok(())
}

fn verify_hirc(
    bank: &Soundbank<&[u8]>,
    input: &[u8],
    rows: &[MediaRecord],
    section: u64,
) -> TestResult {
    let actual = section_rows(rows, "audio-bank-object", section);
    let expected = bank.objects();
    require(
        actual.len() == expected.len(),
        "complete HIRC object directory",
    )?;
    for (ordinal, (row, object)) in actual.into_iter().zip(expected).enumerate() {
        require(
            row.subresource.indices == [section, u64::try_from(ordinal)?],
            "original HIRC object position",
        )?;
        require(
            row.data.pointer("/payload/object_id") == Some(&json!(object.id)),
            "original HIRC object ID",
        )?;
        require(
            range(row, "/payload/body_range", input)? == bank.object_body(object),
            "original HIRC object body",
        )?;
    }
    Ok(())
}

fn verify_didx(body: &[u8], input: &[u8], rows: &[MediaRecord], section: u64) -> TestResult {
    let actual = section_rows(rows, "audio-bank-media", section);
    let (expected, remainder) = body.as_chunks::<DIDX_TRIPLET_BYTES>();
    require(
        actual.len() == expected.len() && remainder.is_empty(),
        "complete DIDX triplet directory",
    )?;
    for (ordinal, (row, triplet)) in actual.into_iter().zip(expected).enumerate() {
        require(
            row.subresource.indices == [section, u64::try_from(ordinal)?],
            "original DIDX occurrence",
        )?;
        require(
            range(row, "/payload/index_range", input)? == triplet,
            "original DIDX triplet range",
        )?;
        for (key, word) in [
            "/payload/media_id",
            "/payload/data_offset",
            "/payload/data_length",
        ]
        .into_iter()
        .zip(triplet.as_chunks::<{ size_of::<u32>() }>().0)
        {
            let original = u32::from_le_bytes(*word);
            require(
                row.data.pointer(key) == Some(&json!(original)),
                "original DIDX word",
            )?;
        }
    }
    Ok(())
}

fn section_rows<'rows>(
    rows: &'rows [MediaRecord],
    namespace: &str,
    section: u64,
) -> Vec<&'rows MediaRecord> {
    at(rows, namespace)
        .filter(|row| row.subresource.indices.first() == Some(&section))
        .collect()
}
