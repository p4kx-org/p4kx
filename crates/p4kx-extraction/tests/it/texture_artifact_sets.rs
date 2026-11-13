// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use p4kx_extraction::{
    Archive, ConflictPolicy, DdsHeader, ExtractOptions, ExtractionReport, TextureConversion,
    extract,
};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};

#[path = "../support/texture_artifacts.rs"]
mod artifacts;
const HEADER: &str = "Data\\Textures\\oracle.dds";

struct Fixture {
    _folder: tempfile::TempDir,
    archive: Archive,
    target: std::path::PathBuf,
}

impl Fixture {
    fn new(entries: Vec<TestEntry>) -> Result<Self, Box<dyn Error>> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("Data.p4k");
        entries
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(&path)?;
        let target = folder.path().join("out");
        fs::create_dir(&target)?;
        Ok(Self {
            archive: Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?,
            target,
            _folder: folder,
        })
    }
}

fn run_with(
    fixture: &Fixture,
    policy: ConflictPolicy,
    textures: TextureConversion,
) -> ExtractionReport {
    let entries: Vec<_> = fixture.archive.entries().iter().collect();
    extract(
        &fixture.archive,
        &entries,
        &fixture.target,
        &ExtractOptions {
            policy,
            convert: true,
            textures,
            ..ExtractOptions::default()
        },
    )
}

type TestResult = Result<(), Box<dyn Error>>;
const RGBA: TestTextureFormat = TestTextureFormat::Dx10Pixel {
    dxgi_format: 28,
    pixel_bytes: 4,
};
const HDR: TestTextureFormat = TestTextureFormat::Dx10Block {
    dxgi_format: 95,
    block_bytes: 16,
};

fn verify_conversion(dds: &SplitDds, mip: u32) -> TestResult {
    let fixture = Fixture::new(dds.entries(HEADER))?;
    let textures = TextureConversion {
        png: Some(mip),
        ..TextureConversion::default()
    };
    let report = run_with(&fixture, ConflictPolicy::Skip, textures);
    assert!(
        report.is_complete(),
        "every synthetic image converts: {report:?}"
    );
    let header = DdsHeader::parse(&dds.header_bytes())?;
    let level = mip.min(header.mip_count().saturating_sub(1));
    let expected = artifacts::expected(&fixture.target, HEADER, &header, level)?;
    artifacts::check(&fixture.target, &expected)?;
    artifacts::check_receipts(&fixture.target, &expected, &report)?;
    assert!(
        report
            .texture_artifacts()
            .iter()
            .flat_map(|texture| &texture.images)
            .all(|image| image.origin.contains_encrypted_source() == Some(false)),
        "every contributor of this synthetic Archive is unencrypted"
    );
    check_alpha_array(dds, &expected, level)?;
    for entry in fixture
        .archive
        .entries()
        .iter()
        .filter(|entry| entry.path() != HEADER)
    {
        assert!(
            !fixture
                .target
                .join(entry.path().replace('\\', "/"))
                .exists(),
            "converted companion {} is consumed",
            entry.path()
        );
    }
    let before = artifacts::snapshot(&fixture.target)?;
    let again = run_with(&fixture, ConflictPolicy::IfChanged, textures);
    assert!(again.is_complete(), "the repeat completes: {again:?}");
    assert!(
        again.texture_artifacts().is_empty(),
        "no image was rewritten"
    );
    assert_eq!(
        again.skipped().len(),
        fixture.archive.entries().len(),
        "every source entry keeps all outputs"
    );
    artifacts::check(&fixture.target, &expected)?;
    assert_eq!(
        artifacts::snapshot(&fixture.target)?,
        before,
        "IfChanged preserves every image byte"
    );
    Ok(())
}

fn check_alpha_array(
    dds: &SplitDds,
    expected: &[artifacts::ExpectedImage],
    level: u32,
) -> TestResult {
    let Some(tail) = dds.alpha_tail() else {
        return Ok(());
    };
    for image in expected {
        let mut reader =
            png::Decoder::new(BufReader::new(fs::File::open(&image.path)?)).read_info()?;
        let mut pixels = vec![0; reader.output_buffer_size().ok_or("PNG size")?];
        reader.next_frame(&mut pixels)?;
        let alpha: Vec<_> = pixels.iter().skip(3).step_by(4).copied().collect();
        assert_eq!(
            alpha,
            tail.surface_mip(image.image.layer, level),
            "every array image keeps its own alpha-tail mip"
        );
    }
    Ok(())
}

#[test]
fn every_supported_topology_has_a_complete_decodable_image_set_at_two_mips() -> TestResult {
    let plane = SplitDds::new(RGBA, 16, 8).split(2);
    let shapes = [
        plane.clone(),
        plane.clone().array(3),
        plane.clone().volume(4),
        plane.clone().cubemap(),
        plane.cubemap().array(2),
    ];
    for shape in shapes {
        verify_conversion(&shape, 0)?;
        verify_conversion(&shape, 2)?;
    }
    Ok(())
}

#[test]
fn alpha_tails_hdr_sets_and_clamped_mips_use_the_same_oracle() -> TestResult {
    let alpha = SplitDds::new(RGBA, 16, 8)
        .array(3)
        .split(2)
        .alpha(TestTextureFormat::R8);
    verify_conversion(&alpha, 0)?;
    verify_conversion(&alpha, 2)?;
    let hdr = SplitDds::new(HDR, 8, 8).cubemap().array(2).split(1);
    verify_conversion(&hdr, 0)?;
    verify_conversion(&hdr, u32::MAX)?;
    Ok(())
}

fn write_wrong_size(path: &Path, format: p4kx_extraction::TextureImageFormat) -> TestResult {
    match format {
        p4kx_extraction::TextureImageFormat::Png => {
            let mut encoder = png::Encoder::new(BufWriter::new(fs::File::create(path)?), 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&[0, 0, 0, 255])?;
            writer.finish()?;
        }
        p4kx_extraction::TextureImageFormat::Exr => {
            let image = p4kx_texture::HdrImage::new(1, 1, vec![0.0; 3])?;
            p4kx_texture::write_exr(&image, &mut fs::File::create(path)?)?;
        }
        _ => return Err("unverified test encoding".into()),
    }
    Ok(())
}

fn negative_set(dds: &SplitDds) -> TestResult {
    let fixture = Fixture::new(dds.entries(HEADER))?;
    let report = run_with(&fixture, ConflictPolicy::Skip, TextureConversion::default());
    assert!(
        report.is_complete(),
        "the original set is valid: {report:?}"
    );
    let header = DdsHeader::parse(&dds.header_bytes())?;
    let expected = artifacts::expected(&fixture.target, HEADER, &header, 0)?;
    artifacts::check(&fixture.target, &expected)?;
    let middle = &expected.get(1).ok_or("missing middle image")?.path;
    let original = fs::read(middle)?;
    fs::remove_file(middle)?;
    assert!(
        artifacts::check(&fixture.target, &expected).is_err(),
        "a missing middle image fails despite intact first and last images"
    );
    fs::write(middle, b"corrupt image")?;
    assert!(
        artifacts::check(&fixture.target, &expected).is_err(),
        "a present corrupt image fails"
    );
    let truncated = original
        .get(..original.len().checked_sub(8).ok_or("image too short")?)
        .ok_or("truncated image range")?;
    fs::write(middle, truncated)?;
    assert!(
        artifacts::check(&fixture.target, &expected).is_err(),
        "truncated encoded image data fails even with its original header"
    );
    let format = expected.get(1).ok_or("middle image")?.image.format;
    write_wrong_size(middle, format)?;
    assert!(
        artifacts::check(&fixture.target, &expected).is_err(),
        "a valid image with wrong dimensions fails"
    );
    fs::write(middle, original)?;
    let extra = fixture
        .target
        .join("unexpected")
        .with_extension(format.extension());
    fs::copy(&expected.first().ok_or("first image")?.path, &extra)?;
    assert!(
        artifacts::check(&fixture.target, &expected).is_err(),
        "an unplanned extra image fails"
    );
    fs::remove_file(extra)?;
    artifacts::check(&fixture.target, &expected)
}

#[test]
fn missing_middle_layers_slices_corruption_wrong_dimensions_and_extras_are_rejected() -> TestResult
{
    negative_set(&SplitDds::new(RGBA, 8, 8).array(3))?;
    negative_set(&SplitDds::new(RGBA, 8, 8).volume(3))?;
    negative_set(&SplitDds::new(HDR, 8, 8).array(3))
}

#[test]
fn only_explicitly_unsupported_codecs_can_be_classified_as_unconverted() {
    use p4kx_extraction::{
        ConversionError, DxgiFormat, ExtractionError, TextureError, TextureFormat,
    };
    let error = |source| {
        ExtractionError::Texture(ConversionError::Texture {
            texture: "synthetic.dds".into(),
            source,
        })
    };
    let unsupported = error(TextureError::UnsupportedDecode(TextureFormat::Dxgi(
        DxgiFormat(32),
    )));
    let supported = error(TextureError::UnsupportedDecode(TextureFormat::Dxgi(
        DxgiFormat(28),
    )));
    let corrupt = error(TextureError::InvalidHeader("synthetic corruption"));
    let invalid_mip = error(TextureError::NoSuchMip {
        level: 9,
        mip_count: 1,
    });
    assert!(
        artifacts::unsupported(&unsupported, TextureFormat::Dxgi(DxgiFormat(32))),
        "a codec with no decoder is explicit"
    );
    assert!(
        !artifacts::unsupported(&unsupported, TextureFormat::Dxgi(DxgiFormat(28))),
        "an error naming another codec cannot excuse a supported source"
    );
    assert!(
        !artifacts::unsupported(&supported, TextureFormat::Dxgi(DxgiFormat(28))),
        "a supported codec cannot excuse failure"
    );
    assert!(
        !artifacts::unsupported(&corrupt, TextureFormat::Dxgi(DxgiFormat(28))),
        "corrupt source is a failure"
    );
    assert!(
        !artifacts::unsupported(&invalid_mip, TextureFormat::Dxgi(DxgiFormat(28))),
        "an invalid selected mip is a failure"
    );
}
