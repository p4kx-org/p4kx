// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic acceptance for disk texture part case resolution on Linux.

#![cfg(target_os = "linux")]

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::{ConversionError, DiskTextures, Texture};
use p4kx_testkit::{SplitDds, TestTextureFormat};

type TestResult = Result<(), Box<dyn Error>>;

fn fixture(folder: &Path) -> Result<PathBuf, Box<dyn Error>> {
    fs::create_dir_all(folder)?;
    Ok(SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 4)
        .split(2)
        .alpha(TestTextureFormat::R8)
        .write_files(folder, "glass.dds")?)
}

fn rename_parts(folder: &Path) -> std::io::Result<()> {
    for (from, to) in [
        ("glass.dds.1", "GLASS.DDS.1"),
        ("glass.dds.2", "GLASS.DDS.2"),
        ("glass.dds.a", "gLaSs.DdS.A"),
        ("glass.dds.1a", "GLASS.DDS.1A"),
        ("glass.dds.2a", "GLASS.DDS.2A"),
    ] {
        fs::rename(folder.join(from), folder.join(to))?;
    }
    Ok(())
}

#[test]
fn colour_alpha_header_and_alpha_parts_match_the_exact_case_twin() -> TestResult {
    let dir = tempfile::tempdir()?;
    let exact = fixture(&dir.path().join("exact"))?;
    let folder = dir.path().join("TeXtUrEs");
    let mixed = fixture(&folder)?;
    rename_parts(&folder)?;
    let expected = Texture::from_file(&exact)?;
    let found = Texture::from_file(&mixed)?;

    let mut expected_dds = Vec::new();
    expected.merge_into(&mut expected_dds)?;
    let mut found_dds = Vec::new();
    found.merge_into(&mut found_dds)?;
    assert_eq!(found_dds, expected_dds);
    assert!(found.has_alpha_tail());
    for level in [0, 1, 3] {
        assert_eq!(
            found.decode_mip(level)?.pixels(),
            expected.decode_mip(level)?.pixels()
        );
    }
    Ok(())
}

#[test]
fn output_cannot_overwrite_any_actual_mixed_case_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = fixture(dir.path())?;
    rename_parts(dir.path())?;
    let texture = Texture::from_file(&header)?;

    for name in ["GLASS.DDS.1", "gLaSs.DdS.A", "GLASS.DDS.1A"] {
        let target = dir.path().join(name);
        let original = fs::read(&target)?;
        let merge = texture.merge_to_file(&target);
        assert!(
            matches!(merge, Err(ConversionError::OutputIsInput { .. })),
            "{merge:?}"
        );
        assert_eq!(fs::read(&target)?, original);
        let png = texture.png_to_file(&target, 0);
        assert!(
            matches!(png, Err(ConversionError::OutputIsInput { .. })),
            "{png:?}"
        );
        assert_eq!(fs::read(&target)?, original);
    }
    Ok(())
}

#[test]
fn an_exact_part_wins_over_another_case_sibling() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = fixture(dir.path())?;
    let expected = Texture::from_file(&header)?.decode_mip(1)?;
    fs::write(dir.path().join("GLASS.DDS.1"), b"different and truncated")?;

    let found = Texture::from_file(&header)?.decode_mip(1)?;

    assert_eq!(found.pixels(), expected.pixels());
    Ok(())
}

#[test]
fn ambiguous_parts_are_missing_instead_of_first_wins() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = fixture(dir.path())?;
    let exact = dir.path().join("glass.dds.1");
    fs::rename(&exact, dir.path().join("GLASS.DDS.1"))?;
    fs::copy(
        dir.path().join("GLASS.DDS.1"),
        dir.path().join("Glass.Dds.1"),
    )?;

    let found = Texture::from_file(&header)?.read_mip(1);

    assert!(
        matches!(found, Err(ConversionError::MissingPart { .. })),
        "{found:?}"
    );
    Ok(())
}

#[test]
fn a_run_reuses_one_listing_across_two_texture_conversions() -> TestResult {
    let dir = tempfile::tempdir()?;
    let first = fixture(&dir.path().join("shared"))?;
    let folder = first.parent().ok_or("header folder")?;
    let template = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 4).split(1);
    let second = template.write_files(folder, "second.dds")?;
    let later_part = folder.join("second.dds.1");
    let bytes = fs::read(&later_part)?;
    fs::remove_file(&later_part)?;
    rename_parts(folder)?;
    let files = DiskTextures::new();
    files.open(&first)?.decode_mip(0)?;
    fs::write(folder.join("SECOND.DDS.1"), &bytes)?;

    let stale = files.open(&second)?.read_mip(0);

    assert!(
        matches!(stale, Err(ConversionError::MissingPart { .. })),
        "{stale:?}"
    );
    assert!(DiskTextures::new().open(&second)?.read_mip(0).is_ok());
    Ok(())
}

#[test]
fn an_unbound_exact_part_remains_available_after_a_listing_was_cached() -> TestResult {
    let dir = tempfile::tempdir()?;
    let first = fixture(dir.path())?;
    let template = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 4).split(1);
    let second = template.write_files(dir.path(), "second.dds")?;
    let part = dir.path().join("second.dds.1");
    let bytes = fs::read(&part)?;
    fs::remove_file(&part)?;
    rename_parts(dir.path())?;
    let files = DiskTextures::new();
    files.open(&first)?.decode_mip(0)?;
    fs::write(&part, &bytes)?;

    assert!(files.open(&second)?.read_mip(0).is_ok());
    Ok(())
}

#[test]
fn a_bound_part_stays_protected_after_an_exact_name_appears() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = fixture(dir.path())?;
    rename_parts(dir.path())?;
    let files = DiskTextures::new();
    let texture = files.open(&header)?;
    let original = texture.read_mip(0)?;
    let actual = dir.path().join("GLASS.DDS.2");
    let bytes = fs::read(&actual)?;
    fs::write(
        dir.path().join("glass.dds.2"),
        b"a different exact-name candidate",
    )?;

    let repeated = texture.read_mip(0)?;
    let refused = texture.merge_to_file(&actual);

    assert_eq!(repeated.data(), original.data());
    assert!(
        matches!(refused, Err(ConversionError::OutputIsInput { .. })),
        "{refused:?}"
    );
    assert_eq!(fs::read(&actual)?, bytes);
    Ok(())
}

#[test]
fn a_mixed_case_broken_alpha_stays_lazy_and_its_real_header_is_protected() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = fixture(dir.path())?;
    rename_parts(dir.path())?;
    let actual = dir.path().join("gLaSs.DdS.A");
    fs::write(&actual, b"not a DDS header")?;
    let texture = Texture::from_file(&header)?;

    assert!(texture.has_alpha_tail());
    assert!(matches!(texture.alpha_tail(), Some(Err(_))));
    texture.merge_to_file(&dir.path().join("colour.dds"))?;
    let refused = texture.merge_to_file(&actual);

    assert!(
        matches!(refused, Err(ConversionError::OutputIsInput { .. })),
        "{refused:?}"
    );
    assert_eq!(fs::read(&actual)?, b"not a DDS header");
    Ok(())
}

#[test]
fn an_unknown_byte_layout_keeps_captured_raw_metadata_available() -> TestResult {
    const DXGI_FORMAT_OFFSET: usize = 128;
    let dir = tempfile::tempdir()?;
    let mut bytes = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 8)
        .mips(1)
        .header_bytes();
    bytes
        .get_mut(DXGI_FORMAT_OFFSET..DXGI_FORMAT_OFFSET + 4)
        .ok_or("DX10 format word")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    let header = dir.path().join("unknown.dds");
    fs::write(&header, bytes)?;
    let texture = DiskTextures::new().open(&header)?;

    let metadata = texture.source_metadata();
    fs::remove_file(&header)?;

    assert!(metadata.normalized.block_layout.is_none());
    assert_eq!(texture.source_metadata(), metadata);
    assert!(texture.part_count().is_err());
    Ok(())
}
