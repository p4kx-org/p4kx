// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Exporting the picture of the UI tab: the files are those the library calls behind
//! `p4kx ui render` write, and the outcome names what was left out.

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use p4kx_extraction::gfx::{Background, CanvasSize, Localise, RasterOptions};
use p4kx_extraction::{
    Archive, ArchiveImages, DataForgeEntry, Language as TextLanguage, LocalizationEntry,
    OriginDigestWriter, ScreenTexts, UiMovie, decode_line_breaks,
};
use p4kx_gui::ui::Dialog;
use p4kx_gui::{Language, UiExportStatus, UiFormat};
use p4kx_screens::{
    ImageSources, MAX_SVG_BYTES, ScreenComposer, Still, StillFormat, StillRequest, TextSources,
};
use serde_json::Value;

use crate::tagging::wait_until;
use crate::ui_tabbing::{
    PANEL, TestResult, Window, exported, listed, opened, pick_file, pick_ship,
};

const WINDOW: [f32; 2] = [1280.0, 800.0];
const MFD: &str = "libs/foundry/records/ui/buildingblocks/test/test_mfd";

const STAGE: RasterOptions = RasterOptions {
    size: CanvasSize::Stage,
    background: Background::Transparent,
};

fn export(window: &mut Window, file: &Path) -> TestResult {
    let started = window
        .harness
        .state_mut()
        .start_ui_export(file.to_path_buf(), || {});
    assert!(started, "the export started");
    wait_until(&mut window.harness, exported)
}

/// Draws the screen with the calls `p4kx ui render --screen --lang en` makes.
fn screen_by_library(
    archive: &Path,
    format: StillFormat,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let archive = Archive::open(archive)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let texts = ScreenTexts::load(
        &archive,
        &ScreenComposer::new(&forge).font_file(),
        Some(TextLanguage::English),
    );
    let ini = texts.parsed(&mut Vec::new());
    let lookup = |key: &str| {
        ini.as_ref()?
            .resolve(key)
            .map(|text| decode_line_breaks(text).into_owned())
    };
    let images = ArchiveImages::new(&archive);
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
    let request = StillRequest {
        screen: MFD,
        brand: Some("test"),
        variables: &[],
        raster: STAGE,
        format,
    };
    let still = Still::render(
        &forge,
        texts.fonts().map(|fonts| TextSources {
            fonts,
            localise: ini.as_ref().map(|_| &lookup as Localise<'_>),
        }),
        Some(ImageSources {
            texture: &read_texture,
            vector: &read_vector,
        }),
        &request,
    )?;
    Ok(still.encode()?.into_owned())
}

fn summary(window: &Window) -> Result<&p4kx_gui::UiExportSummary, Box<dyn std::error::Error>> {
    match window
        .harness
        .state()
        .ui()
        .export()
        .map(p4kx_gui::UiExport::status)
    {
        Some(UiExportStatus::Done(summary)) => Ok(summary),
        other => Err(format!("no summary: {other:?}").into()),
    }
}

#[test]
fn a_screen_is_exported_as_the_png_the_cli_writes() -> TestResult {
    let mut window = opened(Language::BritishEnglish, WINDOW)?;
    pick_ship(&mut window)?;
    let out = window.dir.path().join("mfd.png");

    export(&mut window, &out)?;

    assert_eq!(
        fs::read(&out)?,
        screen_by_library(&window.archive, StillFormat::Png)?,
        "the same bytes as the library writes for the CLI"
    );
    assert_screen_receipt(&out)?;
    let done = summary(&window)?;
    assert_eq!((done.width, done.height), (200, 100), "the size");
    assert!(
        done.left_out > 0,
        "the widgets without a renderer are counted"
    );
    Ok(())
}

#[test]
fn a_screen_is_exported_as_the_svg_the_cli_writes() -> TestResult {
    let mut window = opened(Language::BritishEnglish, WINDOW)?;
    pick_ship(&mut window)?;
    window
        .harness
        .get_by_role_and_label(Role::Button, "SVG")
        .click();
    window.harness.run_steps(2);
    assert_eq!(
        window.harness.state().ui().settings().format,
        UiFormat::Svg,
        "the choice"
    );
    let out = window.dir.path().join("mfd.svg");

    export(&mut window, &out)?;

    assert_eq!(
        fs::read(&out)?,
        screen_by_library(&window.archive, StillFormat::Svg)?,
        "the same document as the library writes for the CLI"
    );
    assert_screen_receipt(&out)
}

fn assert_screen_receipt(output: &Path) -> TestResult {
    let name = output.file_name().ok_or("output basename")?;
    let mut companion = name.to_os_string();
    companion.push(".p4kx-origin.jsonl");
    let receipt = fs::read_to_string(output.with_file_name(companion))?;
    let rows: Vec<Value> = receipt
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let header = rows.first().ok_or("companion header")?;
    let mut digest = OriginDigestWriter::new(io::sink());
    digest.write_all(&fs::read(output)?)?;
    let expected = serde_json::to_value(digest.finish()?)?;
    assert_eq!(
        header
            .pointer("/binding/path/value")
            .and_then(Value::as_str),
        name.to_str(),
        "relative output name"
    );
    for field in ["sha256", "byteLength"] {
        assert_eq!(
            header.get("binding").and_then(|binding| binding.get(field)),
            expected.get(field),
            "committed output {field}"
        );
    }
    assert_eq!(
        rows.last()
            .and_then(|row| row.get("type"))
            .and_then(Value::as_str),
        Some("end"),
        "complete companion"
    );
    assert_screen_sources(&rows)
}

fn assert_screen_sources(rows: &[Value]) -> TestResult {
    let artifact = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("encoded artifact field")?;
    let references = artifact
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("artifact use references")?;
    for (namespace, path) in [
        ("dataForgeBytes", "Data/Game2.dcb"),
        ("gfxTag", "Data/UI/fonts/Shared/fonts_en.gfx"),
        ("globalIniBytes", "Data/Localization/english/global.ini"),
    ] {
        let source_use = rows
            .iter()
            .filter_map(|row| row.get("sourceUse"))
            .find(|usage| {
                usage.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                    && usage.get("id").is_some_and(|id| references.contains(id))
            })
            .ok_or_else(|| format!("artifact use {namespace}"))?;
        let physical = rows
            .iter()
            .filter_map(|row| row.get("source"))
            .find(|source| source.get("id") == source_use.get("source"))
            .and_then(|source| source.pointer("/path/value"))
            .and_then(Value::as_str)
            .ok_or("observed Archive Entry path")?;
        assert_eq!(
            physical.replace('\\', "/"),
            path,
            "actual source of {namespace}"
        );
    }
    Ok(())
}

#[test]
fn a_movie_is_exported_as_the_png_and_the_svg_the_cli_writes() -> TestResult {
    let mut window = listed(Language::BritishEnglish, WINDOW)?;
    pick_file(&mut window, "panel.gfx")?;
    let archive = Archive::open(&window.archive)?;
    for (format, name) in [(UiFormat::Png, "panel.png"), (UiFormat::Svg, "panel.svg")] {
        window.harness.state_mut().ui_mut().set_format(format);
        let out = window.dir.path().join(name);
        let expected = window.dir.path().join(format!("expected-{name}"));
        export(&mut window, &out)?;
        let movie = UiMovie::from_archive(&archive, PANEL)?
            .with_texts(LocalizationEntry::read(&archive, TextLanguage::English)?);
        match format {
            UiFormat::Png => movie.render_to_file(&STAGE, &expected).map(|_| ())?,
            UiFormat::Svg => movie.render_svg_to_file(&STAGE, &expected).map(|_| ())?,
        }

        assert_eq!(fs::read(&out)?, fs::read(&expected)?, "{name}");
        assert_eq!(summary(&window)?.left_out, 2, "{name}: as the report lists");
        window.harness.state_mut().ui_mut().dismiss_export();
    }
    Ok(())
}

#[test]
fn the_outcome_is_shown_and_can_be_dismissed() -> TestResult {
    let mut window = opened(Language::BritishEnglish, WINDOW)?;
    pick_ship(&mut window)?;
    let out = window.dir.path().join("mfd.png");

    export(&mut window, &out)?;
    window.harness.run_steps(3);

    assert!(
        window.harness.query_by_label_contains("Wrote ").is_some(),
        "the outcome names the file"
    );
    window
        .harness
        .get_by_role_and_label(Role::Button, "Close")
        .click();
    window.harness.run_steps(3);
    assert!(window.harness.state().ui().export().is_none(), "dismissed");
    Ok(())
}

#[test]
fn the_open_archive_is_never_overwritten() -> TestResult {
    let mut window = listed(Language::BritishEnglish, WINDOW)?;
    pick_file(&mut window, "panel.gfx")?;
    let before = fs::read(&window.archive)?;

    let archive = window.archive.clone();
    export(&mut window, &archive)?;

    assert!(
        matches!(
            window
                .harness
                .state()
                .ui()
                .export()
                .map(p4kx_gui::UiExport::status),
            Some(UiExportStatus::IsArchive)
        ),
        "refused"
    );
    assert_eq!(fs::read(&window.archive)?, before, "untouched");
    Ok(())
}

#[test]
fn the_export_button_asks_for_a_file_once_something_is_picked() -> TestResult {
    let mut window = listed(Language::BritishEnglish, WINDOW)?;
    assert!(
        !window.harness.state().ui().export_available(),
        "nothing yet"
    );
    window
        .harness
        .get_by_role_and_label(Role::Button, "Export…")
        .click();
    window.harness.run_steps(2);
    assert_eq!(window.asked.get(), None, "a disabled button asks nothing");

    pick_file(&mut window, "plain.swf")?;
    window
        .harness
        .get_by_role_and_label(Role::Button, "Export…")
        .click();
    window.harness.run_steps(2);

    assert_eq!(window.asked.get(), Some(Dialog::UiFile), "asks for a file");
    assert_eq!(
        window.harness.state().ui().proposed_file_name(),
        "plain.png",
        "the proposed name"
    );
    Ok(())
}
