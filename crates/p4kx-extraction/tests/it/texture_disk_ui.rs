// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Parallel disk UI conversion through mixed-case external texture paths.

#![cfg(target_os = "linux")]

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::gfx::{Background, CanvasSize, RasterOptions};
use p4kx_extraction::{ConversionError, Texture, UiAssetKind, UiMovie};
use p4kx_testkit::{
    SplitDds, TestFill, TestMovie, TestPlace, TestShape, TestShapeRecord, TestSignature,
    TestTextureFormat, external_image_body,
};

type TestResult = Result<(), Box<dyn Error>>;

fn shape(id: u16, bitmap: u16) -> TestShape {
    TestShape::new(id, 3, [0, 160, 0, 80])
        .fill(TestFill::ScaledBitmap {
            id: bitmap,
            scale: 20,
            at: (0, 0),
            repeating: false,
            smoothed: false,
        })
        .record(TestShapeRecord::Move {
            to: (0, 0),
            fill0: 0,
            fill1: 1,
            line: 0,
        })
        .record(TestShapeRecord::Line(160, 0))
        .record(TestShapeRecord::Line(0, 80))
        .record(TestShapeRecord::Line(-160, 0))
        .record(TestShapeRecord::Line(0, -80))
}

fn movie(folder: &Path) -> Result<UiMovie<'static>, Box<dyn Error>> {
    let textures = folder.join("Data/TeXtUrEs");
    fs::create_dir_all(&textures)?;
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 4)
        .split(2)
        .alpha(TestTextureFormat::R8);
    for stem in ["one", "two"] {
        dds.write_files(&textures, &format!("{stem}.dds"))?;
        for suffix in [".1", ".2", ".a", ".1a", ".2a"] {
            let name = format!("{stem}.dds{suffix}");
            fs::rename(
                textures.join(&name),
                textures.join(name.to_ascii_uppercase()),
            )?;
        }
    }
    let first = shape(10, 2);
    let second = shape(11, 3);
    let bytes = TestMovie::new(TestSignature::Cfx)
        .stage(16, 4)
        .tag(
            1009,
            &external_image_body(2, 2, 19, (8, 4), ("first", "../textures/ONE.tif")),
        )
        .tag(
            1009,
            &external_image_body(3, 2, 19, (8, 4), ("second", "../textures/TWO.tif")),
        )
        .tag(first.code(), &first.body())
        .tag(second.code(), &second.body())
        .tag(26, &TestPlace::new(1, 10).body2())
        .tag(26, &TestPlace::new(2, 11).at(160, 0).body2())
        .tag(1, &[])
        .bytes();
    let path = folder.join("Data/UI/panel.gfx");
    fs::create_dir_all(path.parent().ok_or("movie folder")?)?;
    fs::write(&path, bytes)?;
    Ok(UiMovie::from_file(&path)?)
}

#[test]
fn two_external_images_convert_in_parallel_through_mixed_case_folders_and_parts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let movie = movie(dir.path())?;

    let report = movie.extract_assets(&dir.path().join("output"))?;

    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        report
            .written
            .iter()
            .filter(|item| item.kind == UiAssetKind::Bitmap)
            .count(),
        2
    );
    assert!(report.tag_failures.is_empty());
    fs::copy(
        dir.path().join("Data/TeXtUrEs/ONE.DDS.2"),
        dir.path().join("Data/TeXtUrEs/One.Dds.2"),
    )?;

    let next_run = movie.extract_assets(&dir.path().join("next-output"))?;

    assert!(
        next_run.failures.iter().any(|failure| failure.id == 2
            && matches!(&failure.error, ConversionError::MissingPart { .. }))
    );
    Ok(())
}

#[test]
fn a_frame_opens_both_external_textures_through_the_shared_run() -> TestResult {
    let dir = tempfile::tempdir()?;
    let movie = movie(dir.path())?;
    let expected = Texture::from_file(&dir.path().join("Data/TeXtUrEs/one.dds"))?.decode_mip(0)?;

    let rendered = movie.render(&RasterOptions {
        size: CanvasSize::Stage,
        background: Background::Transparent,
    })?;

    assert!(
        rendered.bitmap_failures.is_empty(),
        "{:?}",
        rendered.bitmap_failures
    );
    assert!(rendered.skipped.is_empty(), "{:?}", rendered.skipped);
    assert_eq!((rendered.image.width(), rendered.image.height()), (16, 4));
    for x in [3, 11] {
        let at = (2 * 16 + x) * 4;
        let actual = rendered
            .image
            .pixels()
            .get(at..at + 4)
            .ok_or("frame pixel")?;
        let at = (2 * 8 + 3) * 4;
        let expected = expected.pixels().get(at..at + 4).ok_or("texture pixel")?;
        let alpha = expected.get(3).copied().ok_or("alpha channel")?.max(1);
        let tolerance = 2 + 255 / u16::from(alpha);
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| u16::from(actual.abs_diff(*expected)) <= tolerance)
        );
    }
    let ambiguous = dir.path().join("Data/TeXtUrEs/One.Dds.2");
    fs::copy(dir.path().join("Data/TeXtUrEs/ONE.DDS.2"), &ambiguous)?;
    let options = RasterOptions {
        size: CanvasSize::Stage,
        background: Background::Transparent,
    };

    let next_run = movie.render(&options)?;

    assert!(
        next_run
            .bitmap_failures
            .iter()
            .any(|failure| failure.id == 2
                && matches!(&failure.error, ConversionError::MissingPart { .. }))
    );
    fs::remove_file(ambiguous)?;
    assert!(movie.render(&options)?.bitmap_failures.is_empty());
    Ok(())
}
