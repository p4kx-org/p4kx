// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::io::Read;

use p4kx_extraction::{Archive, ConversionError, DdsHeader, Entry, TextureGroups};

pub(super) struct Sample<'a> {
    pub(super) position: usize,
    pub(super) entry: &'a Entry,
    pub(super) header: DdsHeader,
}

pub(super) fn has_suffix(path: &str, suffix: &str) -> bool {
    path.len()
        .checked_sub(suffix.len())
        .and_then(|start| path.get(start..))
        .is_some_and(|end| end.eq_ignore_ascii_case(suffix))
}

fn key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Topology {
    Plane,
    Array,
    Volume,
    Cube,
    CubeArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Stratum {
    topology: Topology,
    high_range: bool,
    image_supported: bool,
}

/// Returns whether an existing image decoder supports the source format.
pub(super) const fn supports_images(header: &DdsHeader) -> bool {
    header.format().is_decodable() || header.format().is_high_range()
}

const fn stratum(header: &DdsHeader) -> Stratum {
    let topology = match (
        header.depth() > 1,
        header.faces() > 1,
        header.array_size() > 1,
    ) {
        (true, _, _) => Topology::Volume,
        (_, true, true) => Topology::CubeArray,
        (_, true, false) => Topology::Cube,
        (_, false, true) => Topology::Array,
        _ => Topology::Plane,
    };
    Stratum {
        topology,
        high_range: header.format().is_high_range(),
        image_supported: supports_images(header),
    }
}

pub(super) fn select(
    archive: &Archive,
    size: usize,
    tails_only: bool,
) -> Result<Vec<Sample<'_>>, Box<dyn Error>> {
    let tails: HashSet<_> = archive
        .entries()
        .iter()
        .filter(|entry| has_suffix(entry.path(), ".dds.a"))
        .filter_map(|entry| entry.path().get(..entry.path().len() - 2))
        .map(key)
        .collect();
    let mut seen = HashSet::new();
    let mut strata = BTreeMap::<Stratum, Vec<Sample<'_>>>::new();
    for (position, entry) in archive.entries().iter().enumerate() {
        if !has_suffix(entry.path(), ".dds")
            || !seen.insert(key(entry.path()))
            || (tails_only && !tails.contains(&key(entry.path())))
        {
            continue;
        }
        let mut prefix = Vec::with_capacity(p4kx_texture::MAX_HEADER_LEN);
        archive
            .open_entry(entry)?
            .take(p4kx_texture::MAX_HEADER_LEN as u64)
            .read_to_end(&mut prefix)?;
        let header = DdsHeader::parse(&prefix).map_err(|source| ConversionError::Texture {
            texture: entry.path().to_owned(),
            source,
        })?;
        strata.entry(stratum(&header)).or_default().push(Sample {
            position,
            entry,
            header,
        });
    }
    let present = strata.len();
    let limit = size.div_ceil(present.max(1)).max(1);
    let mut selected = Vec::new();
    for stratum in strata.into_values() {
        let stride = stratum.len().div_ceil(limit).max(1);
        selected.extend(stratum.into_iter().step_by(stride).take(limit));
    }
    Ok(selected)
}

pub(super) fn members<'a>(archive: &'a Archive, headers: &[Sample<'a>]) -> Vec<&'a Entry> {
    let groups = TextureGroups::build(archive);
    let mut selected = HashSet::new();
    for header in headers {
        selected.insert(header.position);
        if let Some(group) = groups.group_of(header.position) {
            selected.extend(group.members());
        }
    }
    archive
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(position, entry)| selected.contains(&position).then_some(entry))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use p4kx_testkit::{ArchiveBuilder, SplitDds, TestTextureFormat};

    const SAMPLE_SIZE: usize = 400;
    const UNSUPPORTED_COUNT: usize = SAMPLE_SIZE + 1;
    const RGBA: TestTextureFormat = TestTextureFormat::Dx10Pixel {
        dxgi_format: 28,
        pixel_bytes: 4,
    };
    const HDR: TestTextureFormat = TestTextureFormat::Dx10Block {
        dxgi_format: 95,
        block_bytes: 16,
    };

    #[test]
    fn sparse_topologies_and_hdr_are_selected_even_below_the_requested_sample_size()
    -> Result<(), Box<dyn Error>> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("Data.p4k");
        let plane = SplitDds::new(RGBA, 8, 8);
        let shapes = [
            plane.clone(),
            plane.clone().array(3),
            plane.clone().volume(3),
            plane.clone().cubemap(),
            plane.cubemap().array(2),
            SplitDds::new(HDR, 8, 8),
        ];
        let mut builder = ArchiveBuilder::new();
        for (index, shape) in shapes.iter().enumerate() {
            for entry in shape.entries(&format!("shape{index}.dds")) {
                builder = builder.entry(entry);
            }
        }
        builder.write_to(&path)?;
        let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
        let sample = select(&archive, 1, false)?;
        let topologies: HashSet<_> = sample
            .iter()
            .map(|image| stratum(&image.header).topology)
            .collect();
        assert_eq!(
            topologies,
            HashSet::from([
                Topology::Plane,
                Topology::Array,
                Topology::Volume,
                Topology::Cube,
                Topology::CubeArray
            ]),
            "every existing topology is mandatory"
        );
        assert_eq!(
            sample
                .iter()
                .filter(|image| image.header.format().is_high_range())
                .count(),
            1,
            "the sparse HDR stratum is mandatory"
        );
        assert!(
            select(&archive, 60, true)?.is_empty(),
            "missing alpha-tail classes are not invented"
        );
        Ok(())
    }

    #[test]
    fn alpha_members_use_exact_texture_groups_instead_of_a_shared_prefix()
    -> Result<(), Box<dyn Error>> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("Data.p4k");
        let texture = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
            .split(1)
            .alpha(TestTextureFormat::R8);
        let entries = texture.entries("shape.dds");
        let expected = entries.len();
        entries
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .entry(p4kx_testkit::TestEntry::stored(
                "shape.dds.unrelated",
                b"unrelated",
            ))
            .write_to(&path)?;
        let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
        let headers = select(&archive, 60, true)?;
        assert_eq!(headers.len(), 1, "the tailed header is selected");
        assert_eq!(
            members(&archive, &headers).len(),
            expected,
            "only actual header, parts and alpha members belong"
        );
        Ok(())
    }

    struct Fixture {
        folder: tempfile::TempDir,
        archive: Archive,
    }

    fn saturated_arrays(additional: &[(&str, SplitDds)]) -> Result<Fixture, Box<dyn Error>> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("Data.p4k");
        let unsupported = SplitDds::new(
            TestTextureFormat::Dx10Pixel {
                dxgi_format: 32,
                pixel_bytes: 4,
            },
            2,
            2,
        )
        .array(2);
        let mut builder = ArchiveBuilder::new();
        for index in 0..UNSUPPORTED_COUNT {
            for entry in unsupported.entries(&format!("unsupported{index:03}.dds")) {
                builder = builder.entry(entry);
            }
        }
        for (name, dds) in additional {
            for entry in dds.entries(name) {
                builder = builder.entry(entry);
            }
        }
        builder.write_to(&path)?;
        let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
        Ok(Fixture { folder, archive })
    }

    #[test]
    fn a_supported_array_after_401_unsupported_headers_is_reserved_and_converts()
    -> Result<(), Box<dyn Error>> {
        let fixture = saturated_arrays(&[("z-supported.dds", SplitDds::new(RGBA, 2, 2).array(2))])?;
        let sample = select(&fixture.archive, SAMPLE_SIZE, false)?;
        let supported: Vec<_> = sample
            .iter()
            .filter(|image| supports_images(&image.header))
            .collect();
        assert_eq!(supported.len(), 1, "the supported stratum is mandatory");
        assert_eq!(supported[0].entry.path(), "z-supported.dds");
        assert!(
            sample.iter().any(|image| !supports_images(&image.header)),
            "unsupported sources stay explicitly represented"
        );
        let entries: Vec<_> = sample.iter().map(|image| image.entry).collect();
        super::super::verify_sample(
            &fixture.archive,
            &sample,
            &entries,
            &fixture.folder.path().join("out"),
        )
    }

    #[test]
    fn png_and_hdr_array_representatives_survive_unsupported_dominated_sampling()
    -> Result<(), Box<dyn Error>> {
        let fixture = saturated_arrays(&[
            ("z-supported.dds", SplitDds::new(RGBA, 2, 2).array(2)),
            ("zz-hdr.dds", SplitDds::new(HDR, 8, 8).array(2)),
        ])?;
        let sample = select(&fixture.archive, SAMPLE_SIZE, false)?;
        let encodings: HashSet<_> = sample
            .iter()
            .filter(|image| supports_images(&image.header))
            .map(|image| image.header.format().is_high_range())
            .collect();
        assert_eq!(encodings, HashSet::from([false, true]));
        assert!(
            sample
                .iter()
                .all(|image| stratum(&image.header).topology == Topology::Array),
            "absent topologies are not invented"
        );
        let entries: Vec<_> = sample.iter().map(|image| image.entry).collect();
        super::super::verify_sample(
            &fixture.archive,
            &sample,
            &entries,
            &fixture.folder.path().join("out"),
        )
    }

    #[test]
    fn unsupported_only_sources_do_not_fabricate_a_supported_encoding_or_alpha_tail()
    -> Result<(), Box<dyn Error>> {
        let fixture = saturated_arrays(&[])?;
        let sample = select(&fixture.archive, SAMPLE_SIZE, false)?;
        assert!(
            !sample.is_empty(),
            "the unsupported stratum is still inventoried"
        );
        assert!(
            sample.iter().all(|image| !supports_images(&image.header)
                && !image.header.format().is_high_range()
                && stratum(&image.header).topology == Topology::Array),
            "no supported encoding or absent topology is reported"
        );
        assert!(
            select(&fixture.archive, SAMPLE_SIZE, true)?.is_empty(),
            "no alpha-tail class is reported without a source"
        );
        Ok(())
    }
}
