// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::metadata_range::MetadataRange;
use crate::metadata_read_error::MetadataReadError;
use crate::metadata_region::MetadataRegion;
use crate::metadata_section::MetadataSection;
use crate::metadata_tables::MetadataTables;
use crate::v2_directory_metadata::V2DirectoryMetadata;
use crate::zip_directory_metadata::ZipDirectoryMetadata;

const V2_FIRST_OPAQUE_OFFSET: u64 = 105;
const V2_SECOND_OPAQUE_OFFSET: u64 = 137;
const V2_OPAQUE_LEN: u64 = 32;

fn complete(range: MetadataRange, source_len: u64) -> MetadataRegion {
    MetadataRegion::of(range.offset(), range.len(), source_len)
}

fn table(
    tables: MetadataTables,
    section: MetadataSection,
    source_len: u64,
) -> Option<MetadataRegion> {
    let MetadataTables::Located { progress, free } = tables else {
        return None;
    };
    match section {
        MetadataSection::Installation => Some(complete(progress, source_len)),
        MetadataSection::FreeRanges => Some(complete(free, source_len)),
        _ => None,
    }
}

pub(crate) fn zip(
    value: &ZipDirectoryMetadata,
    section: MetadataSection,
    source_len: u64,
) -> Result<Option<MetadataRegion>, MetadataReadError> {
    let region = match section {
        MetadataSection::Directory => Some(complete(value.records, source_len)),
        MetadataSection::ZipEnd => Some(complete(value.end.fixed(), source_len)),
        MetadataSection::ZipComment => Some(value.end.comment()),
        MetadataSection::ZipFollowing => Some(complete(
            value
                .end
                .following()
                .ok_or(MetadataReadError::MalformedSection)?,
            source_len,
        )),
        MetadataSection::Zip64End => value
            .zip64
            .map(|record| complete(record.fixed(), source_len)),
        MetadataSection::Zip64Extension => match value.zip64 {
            Some(record) => Some(
                record
                    .extension()
                    .ok_or(MetadataReadError::MalformedSection)?,
            ),
            None => None,
        },
        MetadataSection::Zip64Locator => value
            .locator
            .map(|record| complete(record.range(), source_len)),
        MetadataSection::Installation | MetadataSection::FreeRanges => match value.installation {
            Some(record) => Some(
                table(record.tables(), section, source_len)
                    .ok_or(MetadataReadError::MalformedSection)?,
            ),
            None => None,
        },
        _ => None,
    };
    Ok(region)
}

pub(crate) fn v2(
    value: &V2DirectoryMetadata,
    section: MetadataSection,
    source_len: u64,
) -> Result<Option<MetadataRegion>, MetadataReadError> {
    let extent = match section {
        MetadataSection::Directory => (
            value.directory_offset,
            value
                .base_directory_size
                .checked_add(value.extension_directory_size)
                .ok_or(MetadataReadError::MalformedSection)?,
        ),
        MetadataSection::V2BaseDirectory => (value.directory_offset, value.base_directory_size),
        MetadataSection::V2ExtensionDirectory => (
            value
                .directory_offset
                .checked_add(value.base_directory_size)
                .ok_or(MetadataReadError::MalformedSection)?,
            value.extension_directory_size,
        ),
        MetadataSection::V2BaseNames => (value.names_offset, value.base_names_size),
        MetadataSection::V2ExtensionNames => (
            value
                .names_offset
                .checked_add(value.base_names_size)
                .ok_or(MetadataReadError::MalformedSection)?,
            value.extension_names_size,
        ),
        MetadataSection::V2Trailer => return Ok(Some(complete(value.trailer, source_len))),
        MetadataSection::V2OpaqueFirst => (
            value
                .trailer
                .offset()
                .checked_add(V2_FIRST_OPAQUE_OFFSET)
                .ok_or(MetadataReadError::MalformedSection)?,
            V2_OPAQUE_LEN,
        ),
        MetadataSection::V2OpaqueSecond => (
            value
                .trailer
                .offset()
                .checked_add(V2_SECOND_OPAQUE_OFFSET)
                .ok_or(MetadataReadError::MalformedSection)?,
            V2_OPAQUE_LEN,
        ),
        MetadataSection::V2Trie => return Ok(Some(value.trie(source_len))),
        MetadataSection::Installation | MetadataSection::FreeRanges => {
            return table(value.tables(source_len), section, source_len)
                .map(Some)
                .ok_or(MetadataReadError::MalformedSection);
        }
        _ => return Ok(None),
    };
    Ok(Some(MetadataRegion::of(extent.0, extent.1, source_len)))
}
