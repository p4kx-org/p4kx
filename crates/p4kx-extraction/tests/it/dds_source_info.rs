// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_extraction::{
    Archive, DdsRawHeader, DdsRawHeaderInfo, DdsSourceInfo, PathIndex, Texture, TextureError,
};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;
const RESERVED_WORD_OFFSET: usize = 32;
const MISC_FLAGS2_OFFSET: usize = 144;
const DXGI_FORMAT_OFFSET: usize = 128;

#[test]
fn archive_and_disk_metadata_share_exact_raw_words_without_another_header_read() -> TestResult {
    let folder = tempfile::tempdir()?;
    let dds = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 8).mips(1);
    let mut bytes = dds.header_bytes();
    set_word(&mut bytes, RESERVED_WORD_OFFSET, u32::MAX)?;
    set_word(&mut bytes, MISC_FLAGS2_OFFSET, 0xface_0007)?;
    let disk = folder.path().join("synthetic.dds");
    fs::write(&disk, &bytes)?;
    let source = folder.path().join("synthetic.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/synthetic.dds", bytes.clone()))
        .write_to(&source)?;
    let archive = Archive::open(&source)?;
    let index = PathIndex::new(&archive);
    let from_archive = Texture::from_index(&index, "Data/synthetic.dds")?;
    let from_disk = Texture::from_file(&disk)?;
    let info = from_disk.source_metadata();
    assert_eq!(
        from_archive.source_metadata(),
        info,
        "same shared DTO for either source"
    );
    assert_eq!(
        info.raw.legacy.reserved1.first(),
        Some(&u32::MAX),
        "authored reserved word"
    );
    assert_eq!(
        info.raw.dx10.as_ref().ok_or("DX10")?.misc_flags2,
        0xface_0007,
        "extra flags stay exact"
    );
    fs::remove_file(&disk)?;
    assert_eq!(
        from_disk.source_metadata(),
        info,
        "metadata uses the captured prefix without reopening"
    );
    let mut merged = Vec::new();
    from_archive.merge_into(&mut merged)?;
    assert_eq!(
        merged.get(..148),
        bytes.get(..148),
        "existing merged DDS keeps its original header"
    );
    Ok(())
}

#[test]
fn unknown_decode_format_keeps_raw_projection_and_uint64_length() -> TestResult {
    let mut bytes = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 8)
        .mips(1)
        .header_bytes();
    set_word(&mut bytes, DXGI_FORMAT_OFFSET, u32::MAX)?;
    let info = DdsSourceInfo::parse(&bytes, u64::MAX)?;
    assert!(
        info.normalized.block_layout.is_none(),
        "unknown byte layout is explicit"
    );
    let value = serde_json::to_value(info)?;
    assert_eq!(
        value.pointer("/header_source_bytes"),
        Some(&json!("18446744073709551615")),
        "source byte lengths use lossless decimal strings"
    );
    assert_eq!(
        value
            .pointer("/raw/dx10/dxgi_format")
            .and_then(Value::as_u64),
        Some(u64::from(u32::MAX)),
        "DWORDs remain unsigned numeric values"
    );
    let validator = jsonschema::validator_for(&schemars::schema_for!(DdsSourceInfo).to_value())?;
    assert!(
        validator.is_valid(&value),
        "unknown raw declarations do not imply corruption"
    );
    Ok(())
}

#[test]
fn schema_requires_nullable_keys_and_exact_unsigned_widths() -> TestResult {
    let bytes = SplitDds::new(TestTextureFormat::LegacyDxt1, 8, 8)
        .mips(1)
        .header_bytes();
    let original = serde_json::to_value(DdsSourceInfo::parse(&bytes, 128)?)?;
    let validator = jsonschema::validator_for(&schemars::schema_for!(DdsSourceInfo).to_value())?;
    assert!(
        validator.is_valid(&original),
        "explicit null DX10 extension remains valid"
    );
    for (pointer, invalid) in [
        ("/raw/legacy/flags", json!(u64::from(u32::MAX) + 1)),
        ("/raw/legacy/flags", json!(-1)),
        ("/raw/legacy/reserved1/0", json!(u64::from(u32::MAX) + 1)),
        ("/raw/legacy/pixel_format/four_cc/0", json!(256)),
        ("/schema_version", json!(2)),
        ("/header_source_bytes", json!("18446744073709551616")),
    ] {
        let mut value = original.clone();
        *value.pointer_mut(pointer).ok_or("fixture field")? = invalid;
        assert!(
            !validator.is_valid(&value),
            "{pointer} must retain its exact source type and width"
        );
    }
    for (owner, field) in [("raw", "dx10"), ("normalized", "block_layout")] {
        let mut missing = original.clone();
        missing
            .get_mut(owner)
            .and_then(Value::as_object_mut)
            .ok_or("projection object")?
            .remove(field)
            .ok_or("present nullable field")?;
        assert!(
            !validator.is_valid(&missing),
            "{owner}.{field} is present even when null"
        );
    }
    Ok(())
}

#[test]
fn raw_projection_survives_an_explicit_normalization_failure() -> TestResult {
    let mut bytes = SplitDds::new(TestTextureFormat::LegacyDxt1, 8, 8)
        .mips(1)
        .header_bytes();
    set_word(&mut bytes, 16, 0)?;
    let raw = DdsRawHeader::parse(&bytes)?;
    let projection = serde_json::to_value(DdsRawHeaderInfo::of(&raw))?;
    assert_eq!(
        projection.pointer("/legacy/width"),
        Some(&json!(0)),
        "invalid topology does not erase authored words"
    );
    assert!(
        matches!(
            DdsSourceInfo::from_raw(&raw, 128),
            Err(TextureError::InvalidHeader(_))
        ),
        "normalization failure is never a fabricated complete topology"
    );
    Ok(())
}

#[test]
fn metadata_distinguishes_integer_and_float_sources_without_decoding_images() -> TestResult {
    for (code, label, high_range) in [(57, "R16_UINT", false), (54, "R16_FLOAT", true)] {
        let mut bytes = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 8)
            .mips(1)
            .header_bytes();
        set_word(&mut bytes, DXGI_FORMAT_OFFSET, code)?;
        let info = DdsSourceInfo::parse(&bytes, 148)?;
        assert_eq!(
            info.raw.dx10.as_ref().ok_or("DX10")?.dxgi_format,
            code,
            "authored source encoding is not an 8-bit preview label"
        );
        assert_eq!(
            info.normalized.format, label,
            "existing reader format identity"
        );
        assert_eq!(
            info.normalized.reader_high_range, high_range,
            "reader high-range classification remains distinct from decoding"
        );
    }
    Ok(())
}

#[test]
fn captured_projection_returns_the_same_layout_for_split_cube_array_expectations() -> TestResult {
    let bytes = SplitDds::new(TestTextureFormat::BC7_SRGB, 16, 8)
        .mips(4)
        .split(2)
        .cubemap()
        .array(2)
        .header_bytes();
    let size = u64::try_from(bytes.len())?;
    let raw = DdsRawHeader::parse(&bytes)?;
    let (info, header) = DdsSourceInfo::from_raw_with_header(&raw, size)?;
    assert_eq!(
        info,
        DdsSourceInfo::from_raw(&raw, size)?,
        "the existing projection contract remains identical"
    );
    assert_eq!(
        (
            header.width(),
            header.height(),
            header.mip_count(),
            header.array_size(),
            header.faces()
        ),
        (16, 8, 4, 2, 6),
        "the returned layout belongs to this exact captured header"
    );
    assert_eq!(header.part_count(size)?, 2, "two outsourced mips");
    assert_eq!(
        header.mip_size(1),
        Some(384),
        "part 1 holds mip 1 of twelve faces"
    );
    assert_eq!(
        header.mip_size(0),
        Some(1536),
        "part 2 holds mip 0 of twelve faces"
    );
    assert_eq!(
        info.raw,
        DdsRawHeaderInfo::of(&raw),
        "authored words stay exact"
    );
    Ok(())
}

#[test]
fn joined_projection_preserves_unknown_layout_and_does_not_consume_invalid_raw() -> TestResult {
    let mut bytes = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 8)
        .mips(1)
        .header_bytes();
    set_word(&mut bytes, DXGI_FORMAT_OFFSET, u32::MAX)?;
    let unknown = DdsRawHeader::parse(&bytes)?;
    let (info, header) = DdsSourceInfo::from_raw_with_header(&unknown, u64::MAX)?;
    assert_eq!(
        info.header_source_bytes.get(),
        u64::MAX,
        "lossless source length"
    );
    assert!(
        info.normalized.block_layout.is_none(),
        "unknown layout stays explicit"
    );
    assert!(!header.format().is_decodable(), "no decoding is inferred");
    assert!(
        matches!(
            header.part_count(u64::MAX),
            Err(TextureError::UnsupportedFormat(_))
        ),
        "part-count unavailability remains distinct from zero"
    );
    set_word(&mut bytes, 16, 0)?;
    let invalid = DdsRawHeader::parse(&bytes)?;
    let authored = DdsRawHeaderInfo::of(&invalid);
    assert!(
        matches!(
            DdsSourceInfo::from_raw_with_header(&invalid, 148),
            Err(TextureError::InvalidHeader(_))
        ),
        "normalization failure is preserved"
    );
    assert_eq!(
        DdsRawHeaderInfo::of(&invalid),
        authored,
        "raw capture survives failure"
    );
    Ok(())
}

fn set_word(bytes: &mut [u8], offset: usize, value: u32) -> TestResult {
    bytes
        .get_mut(offset..offset + 4)
        .ok_or("DWORD source boundary")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[test]
fn historical_metadata_without_transfer_remains_readable() -> TestResult {
    let bytes = SplitDds::new(TestTextureFormat::LegacyDxt1, 8, 8)
        .mips(1)
        .header_bytes();
    let mut json = serde_json::to_value(DdsSourceInfo::parse(&bytes, 128)?)?;
    json.as_object_mut()
        .ok_or("source metadata")?
        .remove("transfer")
        .ok_or("current transfer")?;
    let validator = jsonschema::validator_for(&schemars::schema_for!(DdsSourceInfo).to_value())?;
    assert!(
        validator.is_valid(&json),
        "historical additive metadata remains valid"
    );
    let historical: DdsSourceInfo = serde_json::from_value(json)?;
    assert!(
        historical.transfer.is_none(),
        "absence carries no original declaration proof"
    );
    Ok(())
}
