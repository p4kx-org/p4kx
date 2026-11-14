// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `Still` over a synthetic Archive with the text canvas, wired to the sources of the Archive the
//! way the front-ends do.

use std::path::PathBuf;

use p4kx_dataforge::DataForge;
use p4kx_extraction::{
    Archive, ArchiveImages, CancelToken, DataForgeEntry, Language, ScreenTexts, SourceLocation,
    decode_line_breaks,
};
use p4kx_gfx::{Background, CanvasSize, Localise, RasterOptions};
use p4kx_screens::{
    ArchiveScreen, ImageSources, MAX_SVG_BYTES, ScreenComposer, ScreenError, Still, StillFormat,
    StillRequest, TextSources,
};
use p4kx_testkit::{
    ArchiveBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
    screens_font_movie, screens_text_sample, test_key,
};
use tempfile::TempDir;

use crate::support::{TestResult, near, pixel};

const CANVAS: &str = "libs/foundry/records/ui/buildingblocks/test/text_sample";
const RED: [u8; 4] = [255, 0, 0, 255];

fn archive(dir: &TempDir, fonts: bool) -> Result<Archive, Box<dyn std::error::Error>> {
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new().text("test_hello", "AB").build(),
    ));
    if fonts {
        builder = builder.entry(TestEntry::stored(
            "Data\\UI\\fonts\\Shared\\fonts_en.gfx",
            screens_font_movie(),
        ));
    }
    let path: PathBuf = dir.path().join("Data.p4k");
    builder.write_to(&path)?;
    Ok(Archive::open_with_key(&path, Some(test_key()))?)
}

fn texts_of(archive: &Archive, forge: &DataForge<'_>, language: Option<Language>) -> ScreenTexts {
    let font_file = ScreenComposer::new(forge).font_file();
    ScreenTexts::load(archive, &font_file, language)
}

fn draw(
    archive: &Archive,
    forge: &DataForge<'_>,
    texts: &ScreenTexts,
    request: &StillRequest<'_>,
) -> Result<Still, ScreenError> {
    let mut warnings = Vec::new();
    let ini = texts.parsed(&mut warnings);
    let lookup = |key: &str| {
        ini.as_ref()?
            .resolve(key)
            .map(|text| decode_line_breaks(text).into_owned())
    };
    let images = ArchiveImages::new(archive);
    let read_texture = |path: &str, side: u32, pixels: u64| {
        images
            .texture(path, side, pixels)
            .map_err(|error| error.to_string())
    };
    let read_vector = |path: &str| {
        images
            .vector(path, MAX_SVG_BYTES as u64)
            .map_err(|error| error.to_string())
    };
    let sources = texts.fonts().map(|fonts| TextSources {
        fonts,
        localise: ini.as_ref().map(|_| &lookup as Localise<'_>),
    });
    Still::render(
        forge,
        sources,
        Some(ImageSources {
            texture: &read_texture,
            vector: &read_vector,
        }),
        request,
    )
}

const fn request(format: StillFormat) -> StillRequest<'static> {
    StillRequest {
        screen: CANVAS,
        brand: None,
        variables: &[],
        raster: RasterOptions {
            size: CanvasSize::Stage,
            background: Background::Transparent,
        },
        format,
    }
}

#[test]
fn a_png_still_draws_the_fonts_and_resolves_the_keys_of_the_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir, true)?;
    let bytes = screens_text_sample().build();
    let forge = DataForge::open(&bytes)?;
    let texts = texts_of(&archive, &forge, Some(Language::English));
    assert!(texts.has_fonts(), "the font movie was read");
    assert!(texts.has_texts(), "the global.ini was read");
    assert!(texts.warnings().is_empty(), "nothing was left out");

    let still = draw(&archive, &forge, &texts, &request(StillFormat::Png))?;

    let image = still.image().ok_or("a PNG still has pixels")?;
    assert_eq!(
        (still.width(), still.height()),
        (200, 100),
        "the canvas size"
    );
    assert!(near(pixel(image, 2, 4), RED, 2), "the text of the font");
    assert!(near(pixel(image, 2, 64), RED, 2), "the text of the key");
    assert_eq!(still.format(), StillFormat::Png, "the format");
    assert!(
        still.encode()?.starts_with(&[0x89, b'P', b'N', b'G']),
        "a PNG"
    );
    Ok(())
}

#[test]
fn an_svg_still_writes_one_path_per_shape_and_the_screen_as_title() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir, true)?;
    let bytes = screens_text_sample().build();
    let forge = DataForge::open(&bytes)?;
    let texts = texts_of(&archive, &forge, Some(Language::English));

    let still = draw(&archive, &forge, &texts, &request(StillFormat::Svg))?;

    assert!(still.image().is_none(), "an SVG still has no pixels");
    let svg = String::from_utf8(still.encode()?.into_owned())?;
    assert!(svg.starts_with("<?xml") || svg.starts_with("<svg"), "SVG");
    assert!(svg.contains("<path"), "the glyphs are paths");
    assert!(svg.contains(&format!("<title>{CANVAS}</title>")), "title");
    assert_eq!((still.width(), still.height()), (200, 100), "the size");
    Ok(())
}

#[test]
fn missing_fonts_are_named_and_the_texts_are_listed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir, false)?;
    let bytes = screens_text_sample().build();
    let forge = DataForge::open(&bytes)?;

    let texts = texts_of(&archive, &forge, Some(Language::English));

    assert!(!texts.has_fonts(), "no font movie");
    assert_eq!(texts.warnings().len(), 1, "one source left out");
    let still = draw(&archive, &forge, &texts, &request(StillFormat::Png))?;
    assert!(
        still
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.element == "WidgetText"),
        "the text widgets are listed"
    );
    assert!(!still.is_complete(), "so the still is incomplete");
    Ok(())
}

#[test]
fn a_language_the_archive_lacks_is_a_warning_not_an_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir, true)?;
    let bytes = screens_text_sample().build();
    let forge = DataForge::open(&bytes)?;

    let texts = texts_of(&archive, &forge, Some(Language::German));

    assert!(texts.has_fonts(), "the fonts are there");
    assert!(!texts.has_texts(), "no German global.ini");
    assert_eq!(texts.warnings().len(), 1, "named");
    draw(&archive, &forge, &texts, &request(StillFormat::Png))?;
    Ok(())
}

#[test]
fn a_still_without_sources_lists_what_needs_them() -> TestResult {
    let bytes = screens_text_sample().build();
    let forge = DataForge::open(&bytes)?;

    let still = Still::render(&forge, None, None, &request(StillFormat::Png))?;

    assert!(
        still
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.element == "WidgetText"),
        "the text widgets are listed"
    );
    Ok(())
}

#[test]
fn an_unknown_screen_is_an_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir, true)?;
    let bytes = screens_text_sample().build();
    let forge = DataForge::open(&bytes)?;
    let texts = texts_of(&archive, &forge, None);
    let mut asked = request(StillFormat::Png);
    asked.screen = "libs/foundry/records/ui/buildingblocks/test/nothing";

    let error = draw(&archive, &forge, &texts, &asked).err();

    assert!(
        matches!(error, Some(ScreenError::UnknownCanvas { .. })),
        "{error:?}"
    );
    Ok(())
}

fn font_selector_archives(
    dir: &TempDir,
    selected: &str,
    alternate: &str,
) -> Result<(Archive, Archive), Box<dyn std::error::Error>> {
    let forge = screens_text_sample()
        .structure(TestStruct::new("UIElement").field("fontFile", TestType::String))
        .record(
            TestRecord::new(
                0x180a_68ad_c676_4a22_b2ec_e12a_82a1_e3d3,
                "UIElement",
                "UIElement.BuildingBlocks",
                "libs/foundry/records/ui/uielements/buildingblocks.xml",
            )
            .value("fontFile", TestValue::text(selected)),
        )
        .build();
    let ini = || {
        TestEntry::stored(
            "Data/Localization/english/global.ini",
            GlobalIniBuilder::new().text("test_hello", "AB").build(),
        )
    };
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", forge))
        .entry(TestEntry::stored(
            format!("Data/{selected}"),
            screens_font_movie(),
        ))
        .entry(TestEntry::zstd(alternate, screens_font_movie()).encrypted())
        .entry(ini())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let path = dir.path().join("Other.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(selected, screens_font_movie()).encrypted())
        .entry(ini())
        .write_to(&path)?;
    Ok((archive, Archive::open_with_key(&path, Some(test_key()))?))
}

#[test]
fn a_font_selector_requires_the_actual_record_path_and_owning_archive() -> TestResult {
    const SELECTED: &str = "UI/fonts/selected.gfx";
    const OVERRIDE: &str = "UI/fonts/override.gfx";
    let dir = tempfile::tempdir()?;
    let (archive, other) = font_selector_archives(&dir, SELECTED, OVERRIDE)?;
    let language = Some(Language::English);
    let source = DataForgeEntry::read(&archive)?;
    let screen = ArchiveScreen::new(&archive, &source)?;
    for (texts, expected_selector, encrypted) in [
        (screen.load_texts(language), true, None),
        (
            ScreenTexts::load_observed(&archive, OVERRIDE, language),
            false,
            Some(true),
        ),
        (
            ScreenTexts::load_observed(&other, SELECTED, language),
            false,
            Some(true),
        ),
    ] {
        let drawn = screen.render(
            &texts,
            &request(StillFormat::Png),
            &CancelToken::new(),
            &mut Vec::new(),
        )?;
        assert!(near(
            pixel(drawn.still().image().ok_or("PNG pixels")?, 2, 4),
            RED,
            2
        ));
        let origin = drawn.origin();
        assert_eq!(
            origin.provenance.uses().iter().any(|source| matches!(
                source.location(),
                SourceLocation::DataForge { field, .. } if field == "fontFile"
            )),
            expected_selector,
        );
        assert_eq!(origin.artifact.contains_encrypted_source(), encrypted);
    }
    Ok(())
}
