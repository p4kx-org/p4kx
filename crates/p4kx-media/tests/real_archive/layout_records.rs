// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;

use p4kx_extraction::{
    Archive, EntryMetadataInfo, EntryRecordFormat, MetadataAccess, MetadataExtraStatus,
    MetadataRecord, MetadataSection, MetadataSpan, MetadataTableValue, MetadataView,
};

use super::TestResult;
use super::layout_footer::{self, FREE_LEN, Footer, Installation, PROGRESS_LEN, word};
use super::layout_inventory::require;

const DIRECTORY_SAMPLE: u64 = 16;
const EXTRA_SAMPLE: u64 = 32;
const TABLE_SAMPLE: u64 = 8;
const CENTRAL_LEN: usize = 46;

pub(super) fn verify<'source>(
    archive: &'source Archive,
    access: &mut MetadataAccess<'source, '_>,
    file: &mut File,
    footer: &Footer,
) -> TestResult {
    let view = archive.metadata_view();
    for section in [
        MetadataSection::ZipEnd,
        MetadataSection::ZipComment,
        MetadataSection::ZipFollowing,
        MetadataSection::Zip64End,
        MetadataSection::Zip64Extension,
        MetadataSection::Zip64Locator,
    ] {
        if let Some(span) = view.span(section)? {
            compare_raw(access, file, span)?;
        }
    }
    let mut records = view.records()?;
    let mut offset = footer.directory_offset()?;
    let sample_count = footer.directory_count()?.min(DIRECTORY_SAMPLE);
    require(
        sample_count > 0,
        "installed v1 source has directory records",
    )?;
    let mut admitted = 0;
    for ordinal in 0..sample_count {
        let record = records
            .next_record(access)?
            .ok_or("source directory record")?;
        let header = layout_footer::fixed_at::<CENTRAL_LEN>(file, offset)?;
        require(header.starts_with(b"PK\x01\x02"), "directory signature")?;
        require(
            record.format() == EntryRecordFormat::ZipCentralDirectory,
            "ZIP central directory record format",
        )?;
        require(
            record.original_directory_ordinal() == Some(ordinal),
            "original directory record ordinal",
        )?;
        offset = verify_record(access, file, record, &header, offset)?;
        if let Some(index) = record.admitted_entry_index() {
            verify_entry(archive, index, ordinal, record, &header)?;
            admitted += 1;
        }
    }
    require(admitted > 0, "sample contains a physical Entry")?;
    verify_tables(archive, access, file, footer)
}

fn verify_record<'source>(
    access: &mut MetadataAccess<'source, '_>,
    file: &mut File,
    record: MetadataRecord<'source>,
    header: &[u8; CENTRAL_LEN],
    offset: u64,
) -> TestResult<u64> {
    let spans = [record.zip_name(), record.zip_extra(), record.zip_comment()];
    let mut body_at = offset + CENTRAL_LEN as u64;
    for (span, length_at) in spans.into_iter().zip([28, 30, 32]) {
        let span = span.ok_or("original ZIP field span")?;
        let length = u64::from(u16::from_le_bytes(word(header, length_at)?));
        require(
            span.range().offset() == body_at && span.range().len() == length,
            "directory field extent",
        )?;
        body_at += length;
    }
    let raw = record.raw();
    require(
        raw.range().offset() == offset && raw.range().len() == body_at - offset,
        "raw directory record extent",
    )?;
    require(
        record.v2_signature()?.is_none() && record.v2_stored_digest()?.is_none(),
        "v2 record fields absent",
    )?;
    compare_raw(access, file, raw)?;
    verify_extras(access, file, record)?;
    Ok(body_at)
}

fn verify_entry(
    archive: &Archive,
    index: usize,
    ordinal: u64,
    record: MetadataRecord<'_>,
    header: &[u8; CENTRAL_LEN],
) -> TestResult {
    let entry = archive.entries().get(index).ok_or("physical Entry index")?;
    let data = EntryMetadataInfo::of(entry);
    let zip = data.zip_header.as_ref().ok_or("shared ZIP Entry header")?;
    require(data.owner_entry_index == index, "physical Entry index")?;
    require(
        data.original_directory_ordinal
            .is_some_and(|value| value.get() == ordinal),
        "shared Entry original ordinal",
    )?;
    require(
        data.raw_v2_encryption_word.is_none(),
        "v2 encryption word absent",
    )?;
    for (actual, offset) in [
        (zip.raw_version_made_by, 4),
        (zip.raw_version_needed, 6),
        (zip.general_purpose_flags, 8),
        (data.raw_compression_method, 10),
        (data.dos_timestamp.raw_time, 12),
        (data.dos_timestamp.raw_date, 14),
        (zip.disk_start, 34),
        (zip.internal_attributes, 36),
    ] {
        require(
            actual == u16::from_le_bytes(word(header, offset)?),
            "shared Entry 16-bit directory word",
        )?;
    }
    require(
        zip.external_attributes == u32::from_le_bytes(word(header, 38)?),
        "shared Entry external attributes",
    )?;
    require(
        archive.metadata_view().entry_record(entry)?.range() == record.raw().range(),
        "physical Entry matches original directory record",
    )?;
    Ok(())
}

fn verify_extras<'source>(
    access: &mut MetadataAccess<'source, '_>,
    file: &mut File,
    record: MetadataRecord<'source>,
) -> TestResult {
    let range = record
        .zip_extra()
        .ok_or("original ZIP extra section")?
        .range();
    let mut extra_at = range.offset();
    let extra_end = range.offset() + range.len();
    let mut extras = record.extras().ok_or("ordered ZIP extra cursor")?;
    for ordinal in 0..EXTRA_SAMPLE {
        if extra_at == extra_end {
            require(
                extras.next_field(access)?.is_none(),
                "ordered extra cursor exhausts original extra region",
            )?;
            break;
        }
        require(extra_end - extra_at >= 4, "complete ZIP extra header")?;
        let header = layout_footer::fixed_at::<4>(file, extra_at)?;
        let id = u16::from_le_bytes(word(&header, 0)?);
        let length = u16::from_le_bytes(word(&header, 2)?);
        require(
            length >= 4 && u64::from(length) <= extra_end - extra_at,
            "valid ZIP extra length",
        )?;
        let field = extras.next_field(access)?.ok_or("source ZIP extra field")?;
        require(
            field.ordinal() == ordinal && field.id() == Some(id),
            "extra ordinal and ID",
        )?;
        require(
            field.declared_total_len() == Some(length),
            "extra total-length word",
        )?;
        require(
            field.status() == MetadataExtraStatus::Complete,
            "complete extra framing",
        )?;
        require(
            field.raw().range().offset() == extra_at
                && field.raw().range().len() == u64::from(length),
            "raw extra extent",
        )?;
        let body = field.body().ok_or("ZIP extra body")?.range();
        require(
            body.offset() == extra_at + 4 && body.len() == u64::from(length - 4),
            "ZIP extra body extent",
        )?;
        extra_at += u64::from(length);
    }
    Ok(())
}

fn verify_tables<'source>(
    archive: &'source Archive,
    access: &mut MetadataAccess<'source, '_>,
    file: &mut File,
    footer: &Footer,
) -> TestResult {
    let view = archive.metadata_view();
    let Some(tables) = &footer.installation else {
        require(
            view.installation_records()?.is_none() && view.free_records()?.is_none(),
            "installation and free tables absent",
        )?;
        return Ok(());
    };
    let mut progress = view
        .installation_records()?
        .ok_or("original progress table")?;
    for ordinal in 0..footer.directory_count()?.min(TABLE_SAMPLE) {
        let offset = tables.progress_at + ordinal * PROGRESS_LEN as u64;
        let expected = layout_footer::fixed_at::<PROGRESS_LEN>(file, offset)?;
        let row = progress.next_record(access)?.ok_or("progress row")?;
        let MetadataTableValue::Installation(value) = row.value() else {
            return Err("progress row type".into());
        };
        require(
            row.ordinal() == ordinal && row.raw().range().offset() == offset,
            "installation row position",
        )?;
        require(
            row.raw().range().len() == PROGRESS_LEN as u64,
            "installation row width",
        )?;
        require(
            value.written() == u64::from_le_bytes(word(&expected, 0)?),
            "original installation progress word",
        )?;
        require(
            value.reserved() == Some(u32::from_le_bytes(word(&expected, 8)?)),
            "original installation reserved word",
        )?;
    }
    verify_free(view, access, file, tables)
}

fn verify_free<'source>(
    view: MetadataView<'source>,
    access: &mut MetadataAccess<'source, '_>,
    file: &mut File,
    tables: &Installation,
) -> TestResult {
    let mut free = view.free_records()?.ok_or("original free table")?;
    for ordinal in 0..tables.free_count.min(TABLE_SAMPLE) {
        let offset = tables.free_at + ordinal * FREE_LEN as u64;
        let expected = layout_footer::fixed_at::<FREE_LEN>(file, offset)?;
        let row = free.next_record(access)?.ok_or("free row")?;
        let MetadataTableValue::FreeRange(value) = row.value() else {
            return Err("free row type".into());
        };
        require(
            row.ordinal() == ordinal && row.raw().range().offset() == offset,
            "free row position",
        )?;
        require(row.raw().range().len() == FREE_LEN as u64, "free row width")?;
        require(
            value.extent().offset() == u64::from_le_bytes(word(&expected, 0)?),
            "original free range offset",
        )?;
        require(
            value.extent().declared_len() == u64::from_le_bytes(word(&expected, 8)?),
            "original free range length",
        )?;
    }
    if tables.free_count == 0 {
        require(
            free.next_record(access)?.is_none(),
            "declared empty free table has no rows",
        )?;
    }
    Ok(())
}

fn compare_raw<'source>(
    access: &mut MetadataAccess<'source, '_>,
    file: &mut File,
    span: MetadataSpan<'source>,
) -> TestResult {
    const CHUNK: usize = 4096;
    let range = span.range();
    let length = usize::try_from(range.len())?;
    let mut actual = [0; CHUNK];
    let mut expected = [0; CHUNK];
    for offset in (0..length).step_by(CHUNK) {
        let count = (length - offset).min(CHUNK);
        let actual = actual
            .get_mut(..count)
            .ok_or("actual metadata chunk bounds")?;
        let expected = expected
            .get_mut(..count)
            .ok_or("expected metadata chunk bounds")?;
        access.read_into(span, u64::try_from(offset)?, actual)?;
        layout_footer::read_at(file, range.offset() + u64::try_from(offset)?, expected)?;
        require(actual == expected, "retained metadata bytes match original")?;
    }
    Ok(())
}
