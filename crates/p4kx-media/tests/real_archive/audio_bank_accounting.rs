// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{MediaClass, SectionId, Soundbank, Wem};
use p4kx_media::MediaRecord;
use serde_json::json;

use super::{TestResult, at, range, require};

const DIDX_TRIPLET_BYTES: usize = 12;

pub(super) fn verify<'input>(
    input: &'input [u8],
    rows: &[MediaRecord],
) -> TestResult<Soundbank<&'input [u8]>> {
    let bank = Soundbank::parse(input)?;
    let original = bank.sections().collect::<Vec<_>>();
    let data = original
        .iter()
        .enumerate()
        .find(|(_, section)| section.id == SectionId::DATA)
        .map(|(ordinal, section)| (ordinal, section.body));
    let mut embedded = embedded_rows(rows)?;
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
            SectionId::DIDX => {
                verify_didx(section.body, input, rows, ordinal, data, &mut embedded)?;
            }
            _ => {}
        }
    }
    require(
        embedded.is_empty(),
        "only full WEM extents emit embedded WEM rows",
    )?;
    Ok(bank)
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

type EmbeddedRows<'rows> = BTreeMap<(u64, u64), Vec<&'rows MediaRecord>>;

fn embedded_rows(rows: &[MediaRecord]) -> TestResult<EmbeddedRows<'_>> {
    let mut grouped: EmbeddedRows<'_> = BTreeMap::new();
    for row in rows {
        if !matches!(
            row.subresource.namespace.as_str(),
            "audio-bank-wem" | "audio-bank-wem-chunk"
        ) {
            continue;
        }
        let [section, triplet, ..] = row.subresource.indices.as_slice() else {
            return Err("missing embedded DIDX row identity".into());
        };
        grouped.entry((*section, *triplet)).or_default().push(row);
    }
    Ok(grouped)
}

fn verify_didx(
    body: &[u8],
    input: &[u8],
    rows: &[MediaRecord],
    section: u64,
    data: Option<(usize, &[u8])>,
    embedded: &mut EmbeddedRows<'_>,
) -> TestResult {
    let actual = section_rows(rows, "audio-bank-media", section);
    let (expected, remainder) = body.as_chunks::<DIDX_TRIPLET_BYTES>();
    require(
        actual.len() == expected.len() && remainder.is_empty(),
        "complete DIDX triplet directory",
    )?;
    for (ordinal, (row, triplet)) in actual.into_iter().zip(expected).enumerate() {
        let ordinal = u64::try_from(ordinal)?;
        require(
            row.subresource.indices == [section, ordinal],
            "original DIDX occurrence",
        )?;
        require(
            range(row, "/payload/index_range", input)? == triplet,
            "original DIDX triplet range",
        )?;
        let [id, offset, length] = triplet.as_chunks::<{ size_of::<u32>() }>().0 else {
            return Err("three original DIDX words".into());
        };
        let offset = u32::from_le_bytes(*offset);
        let length = u32::from_le_bytes(*length);
        for (key, original) in [
            ("/payload/media_id", u32::from_le_bytes(*id)),
            ("/payload/data_offset", offset),
            ("/payload/data_length", length),
        ] {
            require(
                row.data.pointer(key) == Some(&json!(original)),
                "original DIDX word",
            )?;
        }
        verify_data(row, input, data, offset, length, embedded)?;
    }
    Ok(())
}

fn verify_data(
    row: &MediaRecord,
    input: &[u8],
    data: Option<(usize, &[u8])>,
    offset: u32,
    length: u32,
    embedded: &mut EmbeddedRows<'_>,
) -> TestResult {
    let link = row.data.pointer("/payload/data").ok_or("DIDX DATA link")?;
    let Some((ordinal, body)) = data else {
        return require(link.get("status") == Some(&json!("missing")), "absent DATA");
    };
    let start = usize::try_from(offset)?;
    let end = start
        .checked_add(usize::try_from(length)?)
        .ok_or("DIDX DATA extent")?;
    let original = body.get(start..end).ok_or("original DIDX DATA span")?;
    require(
        link.get("status") == Some(&json!("resolved"))
            && link.get("section_ordinal") == Some(&json!(ordinal.to_string())),
        "original DATA owner",
    )?;
    verify_span(row, "/payload/data/range", input, original)?;
    let declared = match original.first_chunk::<8>() {
        Some([b'R', b'I', b'F', b'F', size @ ..]) => Some(u32::from_le_bytes(*size)),
        Some([b'R', b'I', b'F', b'X', size @ ..]) => Some(u32::from_be_bytes(*size)),
        _ => None,
    }
    .map(|size| u64::from(size) + 8);
    let class = MediaClass::of(original);
    let prefetch = declared.is_some_and(|declared| declared > u64::from(length));
    require(
        link.get("class") == Some(&json!(class.name()))
            && link.get("declared_wem_length")
                == Some(&json!(declared.map(|size| size.to_string())))
            && link.get("prefetch") == Some(&json!(prefetch)),
        "original DATA classification and prefetch extent",
    )?;
    if class == MediaClass::Wem && !prefetch {
        let [section, triplet] = row.subresource.indices.as_slice() else {
            return Err("original DIDX identity".into());
        };
        let rows = embedded
            .remove(&(*section, *triplet))
            .ok_or("complete banked WEM rows")?;
        verify_embedded(input, original, &rows)?;
    }
    Ok(())
}

fn verify_embedded(input: &[u8], original: &[u8], rows: &[&MediaRecord]) -> TestResult {
    let wem = Wem::parse(original)?;
    let Some((summary, chunks)) = rows.split_first() else {
        return Err("embedded WEM summary".into());
    };
    require(
        summary.subresource.namespace == "audio-bank-wem"
            && summary.subresource.indices.len() == 2
            && summary.data.pointer("/payload/format_tag") == Some(&json!(wem.format_tag())),
        "original embedded WEM summary",
    )?;
    let expected = wem.chunks().collect::<Vec<_>>();
    require(
        chunks.len() == expected.len(),
        "complete embedded RIFF directory",
    )?;
    for (ordinal, (row, chunk)) in chunks.iter().zip(expected).enumerate() {
        let [section, triplet, position] = row.subresource.indices.as_slice() else {
            return Err("original banked RIFF chunk identity".into());
        };
        require(
            row.subresource.namespace == "audio-bank-wem-chunk"
                && summary.subresource.indices == [*section, *triplet]
                && *position == u64::try_from(ordinal)?
                && row.data.pointer("/payload/tag") == Some(&json!(chunk.id)),
            "original embedded RIFF chunk order and tag",
        )?;
        verify_span(row, "/payload/body_range", input, chunk.body)?;
    }
    Ok(())
}

fn verify_span(row: &MediaRecord, key: &str, input: &[u8], original: &[u8]) -> TestResult {
    let actual = range(row, key, input)?;
    require(
        actual.as_ptr() == original.as_ptr() && actual.len() == original.len(),
        "exact physical extent within retained bank bytes",
    )
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
