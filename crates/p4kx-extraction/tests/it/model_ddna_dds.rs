// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::texture_preview::{DdnaMipReason, ModelDdsRefusal, ObservedDdnaDds};
use p4kx_extraction::{Archive, ArchiveTextures, PathIndex, SourceEncryptionState, SourceUses};
use p4kx_gltf::TextureLoader as _;
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat, test_key};
use p4kx_texture::{DdsHeader, DdsRawHeader, DdsTransfer, DxgiFormat, TextureFormat};

type TestResult = Result<(), Box<dyn Error>>;

const REQUEST: &str = "textures/fixture_ddna.dds";
const NORMAL_HEADER: &str = "Data/TEXTURES/fIXTURE_DDNA.DDS";
const TAIL_HEADER: &str = "data/textures/fixture_DDNA.dds.A";
const TAIL_PART: &str = "Data/Textures/Fixture_ddna.dds.1a";

const fn bc4() -> TestTextureFormat {
    TestTextureFormat::Dx10Block {
        dxgi_format: DxgiFormat::BC4_UNORM.0,
        block_bytes: 8,
    }
}

fn normal() -> SplitDds {
    SplitDds::new(TestTextureFormat::LegacyDxt5, 7, 5)
        .mips(3)
        .split(2)
}

fn tail() -> SplitDds {
    SplitDds::new(bc4(), 7, 5).mips(2).split(1)
}

fn entries(normal: &SplitDds, tail: &SplitDds) -> Vec<TestEntry> {
    let mut result = normal.entries(NORMAL_HEADER);
    result.extend([
        TestEntry::stored(TAIL_HEADER, tail.header_bytes()).encrypted(),
        TestEntry::stored(TAIL_PART, tail.part(1)).encrypted(),
    ]);
    result
}

fn archive(
    folder: &Path,
    entries: impl IntoIterator<Item = TestEntry>,
) -> Result<Archive, Box<dyn Error>> {
    let source = folder.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&source)?;
    Ok(Archive::open_with_key(source, Some(test_key()))?)
}

#[test]
fn split_normal_and_encrypted_smoothness_use_the_same_mip_as_png() -> TestResult {
    let dir = tempfile::tempdir()?;
    let normal = normal();
    let tail = tail();
    let archive = archive(dir.path(), entries(&normal, &tail))?;
    let index = PathIndex::new(&archive);
    let loader = ArchiveTextures::new(&index);
    for (request, level, size, reason) in [
        (0, 0, (7, 5), DdnaMipReason::Requested),
        (99, 1, (3, 2), DdnaMipReason::SmoothnessChain),
    ] {
        let pair = loader.load_ddna_dds(REQUEST, request)?;
        assert_eq!(pair.requested_mip, request, "original request");
        assert_eq!(pair.reason, reason, "common selection reason");
        assert_eq!(pair.normal.read.mip.level(), level, "normal mip level");
        assert_eq!(
            pair.smoothness.read.mip.level(),
            level,
            "smoothness mip level"
        );
        assert_eq!(
            loader.dimensions(REQUEST, request)?,
            Some(size),
            "same dimensions as PNG route"
        );
        assert_eq!(
            loader.load_observed(REQUEST, request)?.level,
            level,
            "same level as PNG route"
        );
        verify_artifact(&pair.normal.read, &normal, level, size)?;
        verify_artifact(&pair.smoothness.read, &tail, level, size)?;
    }
    Ok(())
}

fn verify_artifact(
    read: &p4kx_texture::LocatedMip,
    fixture: &SplitDds,
    level: u32,
    size: (u32, u32),
) -> TestResult {
    let expected = fixture.mip(level);
    assert_eq!(read.mip.data(), expected, "exact selected block payload");
    let mut artifact = Vec::new();
    read.write_dds(&mut artifact)?;
    let header = DdsHeader::parse(&artifact)?;
    assert_eq!(
        (header.width(), header.height()),
        size,
        "normalized mip dimensions"
    );
    assert_eq!(header.mip_count(), 1, "single-mip output");
    assert_eq!(
        artifact.get(header.header_len()..),
        Some(expected.as_slice()),
        "untouched artifact blocks"
    );
    Ok(())
}

#[test]
fn physical_payload_origins_are_independent_of_the_joint_selection() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), entries(&normal(), &tail()))?;
    let index = PathIndex::new(&archive);
    let loader = ArchiveTextures::new(&index);
    let pair = loader.load_ddna_dds(REQUEST, 0)?;
    let uses = SourceUses::new();
    let normal = pair.normal.evidence(&uses)?;
    let smoothness = pair.smoothness.evidence(&uses)?;
    assert_eq!(
        normal.color.mip().encryption(),
        SourceEncryptionState::Unencrypted,
        "normal payload origin"
    );
    assert_eq!(
        smoothness.color.mip().encryption(),
        SourceEncryptionState::Encrypted,
        "tail payload origin"
    );
    assert_eq!(
        pair.normal.read.payload.part_index(),
        Some(2),
        "normal sibling"
    );
    assert_eq!(
        pair.smoothness.read.payload.part_index(),
        Some(1),
        "tail sibling uses the numbered a suffix"
    );
    let coordinates = pair
        .smoothness
        .read
        .payload
        .spans(None)
        .ok_or("tail span")?;
    assert_eq!(
        (coordinates.offset, coordinates.length, coordinates.count),
        (0, 32, 1),
        "original BC4 bytes on an odd-sized mip"
    );
    assert_eq!(
        pair.smoothness.read.mip.header().format(),
        TextureFormat::Dxgi(DxgiFormat::BC4_UNORM),
        "stored type"
    );
    assert_eq!(
        pair.smoothness.read.raw_header.transfer().effective,
        DdsTransfer::Linear,
        "declared channel transfer"
    );
    verify_joint_selection(&pair, &uses, smoothness.color.mip())
}

fn verify_joint_selection(
    pair: &ObservedDdnaDds<'_>,
    uses: &SourceUses,
    declared_mip: &p4kx_extraction::SourceUse,
) -> TestResult {
    let selection = pair.selection_origin(uses)?;
    assert!(
        selection.source_set_complete(),
        "both selectors have original evidence"
    );
    assert_eq!(
        selection.contains_encrypted_source(),
        Some(true),
        "the shared decision uses the encrypted tail"
    );
    let inventory = uses.freeze()?;
    let joint_sources = inventory
        .uses()
        .iter()
        .filter(|used| selection.source_uses_refs().contains(used.id()))
        .map(|used| used.source().as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        joint_sources.len(),
        2,
        "both original DDS headers supplied the shared decision"
    );
    let (_, tail_part) = pair.smoothness.checksum_uses(uses)?;
    assert_eq!(
        tail_part
            .ok_or("verified split tail checksum")?
            .description()
            .source(),
        declared_mip.description().source(),
        "payload and verified checksum use the same physical tail Entry"
    );
    Ok(())
}

#[test]
fn unknown_or_unsupported_smoothness_layouts_do_not_become_direct_assets() -> TestResult {
    let cases = [
        (TestTextureFormat::R8, "R8"),
        (
            TestTextureFormat::Dx10Pixel {
                dxgi_format: 65,
                pixel_bytes: 1,
            },
            "A8",
        ),
    ];
    for (format, label) in cases {
        let dir = tempfile::tempdir()?;
        let tail = SplitDds::new(format, 7, 5).mips(2).split(1);
        let archive = archive(dir.path(), entries(&normal(), &tail))?;
        let index = PathIndex::new(&archive);
        let loader = ArchiveTextures::new(&index);
        assert!(
            matches!(
                loader.load_ddna_dds(REQUEST, 0),
                Err(ModelDdsRefusal::UnsupportedSmoothness)
            ),
            "no consumer evidence for {label}"
        );
        assert!(
            loader.load_observed(REQUEST, 0).is_ok(),
            "composed PNG still works for {label}"
        );
        assert!(
            matches!(
                loader.load_ddna_dds("textures/fixture_opac.dds", 0),
                Err(ModelDdsRefusal::NotDdna)
            ),
            "opacity is outside the pair API"
        );
    }
    Ok(())
}

#[test]
fn physical_tail_failures_do_not_silently_select_another_member() -> TestResult {
    let normal = normal();
    let tail = tail();
    for candidate in [
        Vec::new(),
        vec![TestEntry::stored(TAIL_PART, tail.part(1)).with_crc32c(0)],
        vec![
            TestEntry::stored(TAIL_PART, tail.part(1)),
            TestEntry::stored(TAIL_PART.to_ascii_uppercase(), tail.part(1)),
        ],
    ] {
        let count = candidate.len();
        let dir = tempfile::tempdir()?;
        let archive = archive(
            dir.path(),
            entries(&normal, &tail)
                .into_iter()
                .filter(|entry| entry.path() != TAIL_PART)
                .chain(candidate),
        )?;
        let index = PathIndex::new(&archive);
        let loader = ArchiveTextures::new(&index);
        assert!(
            matches!(
                (count, loader.load_ddna_dds(REQUEST, 0)),
                (0, Err(ModelDdsRefusal::Missing))
                    | (1, Err(ModelDdsRefusal::Dds(_)))
                    | (2, Err(ModelDdsRefusal::Ambiguous))
            ),
            "missing, failed verification and ambiguous selected tails have distinct results"
        );
    }
    Ok(())
}

#[test]
fn a_tail_without_dimensions_or_with_different_dimensions_is_refused() -> TestResult {
    let normal = normal();
    let wrong_size = SplitDds::new(bc4(), 9, 5).mips(2).split(1);
    let dir = tempfile::tempdir()?;
    let archive_wrong_size = archive(dir.path(), entries(&normal, &wrong_size))?;
    let index = PathIndex::new(&archive_wrong_size);
    assert!(
        matches!(
            ArchiveTextures::new(&index).load_ddna_dds(REQUEST, 0),
            Err(ModelDdsRefusal::MismatchedDimensions)
        ),
        "selected dimensions must agree"
    );
    let dir = tempfile::tempdir()?;
    let tail = tail();
    let headerless = entries(&normal, &tail).into_iter().map(|entry| {
        if entry.path() == TAIL_HEADER {
            TestEntry::stored(TAIL_HEADER, tail.mip(1))
        } else {
            entry
        }
    });
    let archive_headerless = archive(dir.path(), headerless)?;
    let index = PathIndex::new(&archive_headerless);
    assert!(
        matches!(
            ArchiveTextures::new(&index).load_ddna_dds(REQUEST, 0),
            Err(ModelDdsRefusal::Dds(_))
        ),
        "raw block length cannot establish source format"
    );
    Ok(())
}

#[test]
fn magicless_but_fully_headered_bc4_tiny_tail_remains_typed() -> TestResult {
    let combined = SplitDds::new(TestTextureFormat::LegacyDxt5, 1, 1).alpha(bc4());
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), combined.entries(NORMAL_HEADER))?;
    let index = PathIndex::new(&archive);
    let loader = ArchiveTextures::new(&index);
    let pair = loader.load_ddna_dds(REQUEST, 8)?;
    assert_eq!(
        pair.reason,
        DdnaMipReason::BothChains,
        "both one-level textures constrain the mip"
    );
    assert_eq!(
        pair.smoothness.read.mip.data().len(),
        8,
        "one aligned BC4 block"
    );
    let mut artifact = Vec::new();
    pair.smoothness.read.write_dds(&mut artifact)?;
    let parsed = DdsRawHeader::parse(&artifact)?;
    assert_eq!(
        parsed.dx10().ok_or("normalised DX10 header")?.dxgi_format,
        DxgiFormat::BC4_UNORM.0,
        "the typed tail keeps its format"
    );
    assert!(
        artifact.starts_with(b"DDS "),
        "standalone artifact has magic"
    );
    Ok(())
}
