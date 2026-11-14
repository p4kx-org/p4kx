// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The textures and vector graphics that screens draw, read from an Archive by the path of a
//! Record.

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{
    Archive, ArchiveImages, ConversionError, SourceEncryptionState, SourceLocation, SourceUses,
};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};

type TestResult = Result<(), Box<dyn Error>>;

const HEADER: &str = "Data\\UI\\Textures\\Test\\quad.dds";
const SVG: &str = "Data/UI/Textures/Vector/Test/box.svg";
const BOX: &[u8] = b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"/>";

fn write_archive(folder: &Path) -> Result<Archive, Box<dyn Error>> {
    let dds = SplitDds::new(TestTextureFormat::BC7_SRGB, 32, 16).split(3);
    let path = folder.join("Data.p4k");
    dds.entries(HEADER)
        .into_iter()
        .chain([TestEntry::stored(SVG, BOX.to_vec())])
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        &path,
        Some(p4kx_testkit::test_key()),
    )?)
}

#[test]
fn a_texture_is_read_by_the_path_of_a_record_in_any_case() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let images = ArchiveImages::new(&archive);
    let image = images.texture("ui/textures/TEST/quad.dds", 1000, u64::MAX)?;
    assert_eq!((image.width(), image.height()), (32, 16), "the largest mip");
    Ok(())
}

#[test]
fn the_smallest_mip_that_still_covers_the_screen_is_decoded() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let images = ArchiveImages::new(&archive);
    let image = images.texture("UI/Textures/Test/quad.dds", 8, u64::MAX)?;
    assert_eq!((image.width(), image.height()), (8, 4), "two levels down");
    let tiny = images.texture("UI/Textures/Test/quad.dds", 1, u64::MAX)?;
    assert!(tiny.width() >= 1 && tiny.height() >= 1, "never empty");
    Ok(())
}

#[test]
fn a_texture_the_archive_lacks_is_an_unknown_entry() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let result = ArchiveImages::new(&archive).texture("UI/Textures/Test/none.dds", 100, u64::MAX);
    assert!(
        matches!(result, Err(ConversionError::UnknownEntry(_))),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn a_vector_graphic_is_read_up_to_its_limit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let images = ArchiveImages::new(&archive);
    let bytes = images.vector("UI/Textures/Vector/Test/BOX.svg", 1 << 20)?;
    assert_eq!(bytes, BOX, "the file");
    let refused = images.vector("UI/Textures/Vector/Test/box.svg", 8);
    assert!(
        matches!(refused, Err(ConversionError::ReadEntry { .. })),
        "{refused:?}"
    );
    let missing = images.vector("UI/Textures/Vector/Test/none.svg", 100);
    assert!(
        matches!(missing, Err(ConversionError::UnknownEntry(_))),
        "{missing:?}"
    );
    Ok(())
}

fn split_screen_mip_fixture(folder: &Path) -> Result<Archive, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 32, 16)
        .split(3)
        .alpha(TestTextureFormat::R8);
    let selected_alpha = format!("{HEADER}.1a");
    let unused_alpha = format!("{HEADER}.3a");
    let entries = dds.entries(HEADER).into_iter().map(|entry| {
        if entry.path() == selected_alpha || entry.path() == unused_alpha {
            entry.encrypted()
        } else {
            entry
        }
    });
    entries
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        &path,
        Some(p4kx_testkit::test_key()),
    )?)
}

#[test]
fn screen_mip_receipts_name_only_the_decoded_color_and_alpha_spans() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = split_screen_mip_fixture(dir.path())?;
    let images = ArchiveImages::new(&archive);
    let name = "ui/textures/test/quad.dds";
    let observed = images.texture_observed(name, 8, u64::MAX)?;
    assert_eq!(observed.image(), &images.texture(name, 8, u64::MAX)?);
    let uses = SourceUses::new();
    let evidence = observed.evidence(&uses)?;
    let mut pixels = Vec::new();
    evidence.append_pixel_uses(&mut pixels)?;
    let inventory = uses.freeze()?;
    let mut parts = Vec::new();
    for usage in inventory.uses() {
        let SourceLocation::Subresource {
            namespace, indices, ..
        } = usage.location()
        else {
            continue;
        };
        if namespace != "ddsMipPayload" {
            continue;
        }
        let source = inventory
            .sources()
            .iter()
            .find(|source| source.id() == usage.source())
            .ok_or("mip source in inventory")?;
        let source = serde_json::to_value(source)?;
        let name = source
            .pointer("/path/value")
            .and_then(serde_json::Value::as_str)
            .ok_or("mip Entry path")?;
        assert_eq!(indices.first(), Some(&2), "selected mip level");
        assert_eq!(indices.get(1), Some(&0), "split Entry offset");
        parts.push((
            name.to_owned(),
            indices.get(2).copied().ok_or("mip length")?,
            usage.effective_encryption(),
        ));
    }
    parts.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        parts,
        [
            (
                format!("{HEADER}.1"),
                128,
                SourceEncryptionState::Unencrypted
            ),
            (format!("{HEADER}.1a"), 32, SourceEncryptionState::Encrypted),
        ],
        "the other encrypted sibling cannot mark this screen mip",
    );
    Ok(())
}

#[test]
fn screen_vector_receipt_keeps_the_first_physical_match_under_data() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(SVG, BOX.to_vec()).encrypted())
        .entry(TestEntry::stored(
            SVG.to_ascii_lowercase(),
            b"later".to_vec(),
        ))
        .entry(TestEntry::stored(
            "UI/Textures/Vector/Test/box.svg",
            b"without data".to_vec(),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let images = ArchiveImages::new(&archive);
    let observed = images.vector_observed("UI/Textures/Vector/Test/BOX.svg", 1024)?;
    assert_eq!(observed.bytes(), BOX);
    let origin = observed.origin().ok_or("physical SVG Entry")?;
    assert_eq!(origin.entry_index(), 0, "first matching directory row");
    assert_eq!(origin.entry().path(), SVG, "stored name");
    Ok(())
}
