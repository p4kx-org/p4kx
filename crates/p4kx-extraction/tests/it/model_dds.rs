// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::texture_preview::ModelDdsRefusal;
use p4kx_extraction::{Archive, ArchiveTextures, PathIndex, SourceUses};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};

type TestResult = Result<(), Box<dyn Error>>;

const HEADER: &str = r"Data\Textures\fixture.dds";
const PART: &str = r"Data\Textures\fixture.dds.2";
const REQUEST: &str = "textures/fixture.dds";

fn split() -> SplitDds {
    SplitDds::new(TestTextureFormat::LegacyDxt5, 7, 5)
        .mips(3)
        .split(2)
}

fn archive(
    folder: &Path,
    entries: impl IntoIterator<Item = TestEntry>,
) -> Result<Archive, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        &path,
        Some(p4kx_testkit::test_key()),
    )?)
}

#[test]
fn a_selected_dds_binds_the_verified_split_and_tail_payloads() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = split();
    let entries = source.entries(HEADER).into_iter().map(|entry| {
        if entry.path() == PART {
            entry.encrypted()
        } else {
            entry
        }
    });
    let archive = archive(dir.path(), entries)?;
    let index = PathIndex::new(&archive);
    let loader = ArchiveTextures::new(&index);

    for (requested, expected, part) in [(0, 0, Some(2)), (99, 2, None)] {
        let observed = loader.load_dds(REQUEST, requested)?;
        assert_eq!(observed.read.mip.level(), expected);
        assert_eq!(observed.read.mip.data(), source.mip(expected));
        assert_eq!(observed.read.payload.part_index(), part);
        assert_eq!(observed.part_checksum.is_some(), part.is_some());

        let uses = SourceUses::new();
        let evidence = observed.evidence(&uses)?;
        assert!(evidence.is_complete());
        assert_eq!(observed.checksum_uses(&uses)?.1.is_some(), part.is_some());
        let mut artifact = Vec::new();
        observed.read.write_dds(&mut artifact)?;
        assert!(artifact.ends_with(&source.mip(expected)));
        assert!(artifact.starts_with(b"DDS "));
    }
    Ok(())
}

#[test]
fn a_wrong_checksum_or_overlong_selected_member_fails_even_with_its_mip_prefix() -> TestResult {
    let source = split();
    for path in [HEADER, PART] {
        for overlong in [false, true] {
            let dir = tempfile::tempdir()?;
            let entries = source.entries(HEADER).into_iter().map(|entry| {
                if entry.path() != path {
                    return entry;
                }
                if overlong {
                    let mut data = if path == HEADER {
                        source.header_bytes()
                    } else {
                        source.mip(0)
                    };
                    data.extend_from_slice(b"overlong");
                    entry.with_payload(data)
                } else {
                    entry.with_crc32c(0)
                }
            });
            let archive = archive(dir.path(), entries)?;
            let index = PathIndex::new(&archive);
            let loader = ArchiveTextures::new(&index);
            assert!(
                matches!(loader.load_dds(REQUEST, 0), Err(ModelDdsRefusal::Dds(_))),
                "the input is {path}, overlong={overlong}"
            );
        }
    }
    Ok(())
}

#[test]
fn a_duplicate_required_member_is_never_selected_by_case() -> TestResult {
    let source = split();
    for (path, bytes) in [(HEADER, source.header_bytes()), (PART, source.mip(0))] {
        let dir = tempfile::tempdir()?;
        let entries = source
            .entries(HEADER)
            .into_iter()
            .chain([TestEntry::stored(path.to_ascii_uppercase(), bytes)]);
        let archive = archive(dir.path(), entries)?;
        let index = PathIndex::new(&archive);
        let loader = ArchiveTextures::new(&index);
        assert!(matches!(
            loader.load_dds(REQUEST, 0),
            Err(ModelDdsRefusal::Ambiguous)
        ));
    }
    Ok(())
}

#[test]
fn a_separate_alpha_member_requires_the_composed_png() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = split().alpha(TestTextureFormat::R8);
    let archive = archive(dir.path(), source.entries(HEADER))?;
    let index = PathIndex::new(&archive);
    let loader = ArchiveTextures::new(&index);
    assert!(matches!(
        loader.load_dds(REQUEST, 0),
        Err(ModelDdsRefusal::SplitAlpha)
    ));
    Ok(())
}

#[test]
fn unsupported_cube_topology_does_not_silently_export_the_first_face() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = split().cubemap();
    let archive = archive(dir.path(), source.entries(HEADER))?;
    let index = PathIndex::new(&archive);
    let loader = ArchiveTextures::new(&index);
    assert!(
        matches!(loader.load_dds(REQUEST, 0), Err(ModelDdsRefusal::Topology)),
        "the model texture request refuses all cube faces"
    );
    Ok(())
}

#[test]
fn missing_selected_sibling_never_uses_a_neighbouring_mip() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = split();
    let entries = source
        .entries(HEADER)
        .into_iter()
        .filter(|entry| entry.path() != PART);
    let archive = archive(dir.path(), entries)?;
    let index = PathIndex::new(&archive);
    let loader = ArchiveTextures::new(&index);
    assert!(
        matches!(loader.load_dds(REQUEST, 0), Err(ModelDdsRefusal::Missing)),
        "the selected sibling is absent"
    );
    assert_eq!(
        loader.load_dds(REQUEST, 1)?.read.mip.data(),
        source.mip(1),
        "the neighbouring selection remains independent"
    );
    Ok(())
}
