// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The powered MFD of the Avenger Titan: the page its variables choose and its tab bar.

use std::error::Error;
use std::ops::Range;

use p4kx_extraction::gfx::Localise;
use p4kx_extraction::{
    Archive, ArchiveImages, DataForgeEntry, Language, LocalizationEntry, UiMovie,
    decode_line_breaks,
};
use p4kx_gfx::{Background, CanvasSize, RasterOptions, RgbaImage};
use p4kx_screens::{Composed, ImageSources, MAX_SVG_BYTES, ScreenComposer, TextSources, Variable};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const MFD: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/general/m_mfd_screen";
const POWER_PAGE: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/power/mc_s_power_master";
const SHIELDS_PAGE: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/shields/mc_s_shields_master";
const WIDTH: u32 = 800;

/// Where the RSI shields canvas draws the solid chip behind its `SHIELDS` label, which the Aegis
/// MFD must not show: its master styles no `aegs` brand, so the canvas without a brand style is
/// the generic one.
const SHIELDS_LABEL: (u32, u32) = (70, 55);
/// The most alpha the faint panel of the generic shields canvas has.
const FAINT: u8 = 64;

/// Inside the left backdrop of the turret layout, where a solid block was drawn.
const BACKDROP: (u32, u32) = (60, 420);
/// Inside the box of the ship diagram, where a solid rectangle was drawn.
const DIAGRAM: (u32, u32) = (400, 250);
/// Inside the bar of the tab bar, clear of its text and its accent.
const TAB_BAR: (u32, u32) = (200, 545);
/// The columns and the rows the left arrow of the tab bar sits in.
const ARROW_COLUMNS: Range<u32> = 45..80;
const ARROW_ROWS: Range<u32> = 550..585;

fn variables(settings: &[&str]) -> Result<Vec<Variable>, Box<dyn Error>> {
    settings
        .iter()
        .map(|setting| Ok(setting.parse::<Variable>()?))
        .collect()
}

fn raster(composed: &Composed) -> Result<RgbaImage, Box<dyn Error>> {
    Ok(composed
        .rasterize(&RasterOptions {
            size: CanvasSize::Width(WIDTH),
            background: Background::Transparent,
        })?
        .image)
}

fn alpha_at(image: &RgbaImage, (x, y): (u32, u32)) -> u8 {
    let at = (y as usize * image.width() as usize + x as usize) * 4 + 3;
    image.pixels().get(at).copied().unwrap_or_default()
}

/// Returns whether a pixel of the box is opaque and not dark: ink in a colour of the style, not
/// black.
fn has_bright_ink(image: &RgbaImage, columns: Range<u32>, rows: Range<u32>) -> bool {
    rows.flat_map(|row| columns.clone().map(move |column| (column, row)))
        .any(|(column, row)| {
            let at = (row as usize * image.width() as usize + column as usize) * 4;
            let pixel = image.pixels().get(at..at + 4);
            matches!(pixel, Some(&[red, green, blue, alpha])
                if alpha > 200 && red.max(green).max(blue) > 100)
        })
}

/// Widgets and canvases of the powered MFD on LIVE: the default page 37 in 5, the power page 155
/// in 8 (Aegis). The union of all modes, before the integer constants were read, was 206 in 18.
const DEFAULT_PAGE: (Range<usize>, usize) = (33..42, 5);
const POWER_PAGE_WIDGETS: Range<usize> = 148..163;
const POWER_PAGE_CANVASES: usize = 8;
/// The columns and rows of the footer of the RSI MFD where its page name stands.
const RSI_NAME_COLUMNS: Range<u32> = 260..540;
const RSI_NAME_ROWS: Range<u32> = 550..585;

/// Checks one page of the MFD of a brand: no old block, the arrows and the bar of the tab bar, the
/// size of the page, the footer of the RSI.
fn assert_page(
    brand: &str,
    (page, on_default): (&str, bool),
    composed: &Composed,
    image: &RgbaImage,
) {
    let report = composed.report();
    if on_default {
        assert_eq!(alpha_at(image, BACKDROP), 0, "{brand}: no backdrop");
        assert_eq!(alpha_at(image, DIAGRAM), 0, "{brand}: no diagram box");
    }
    assert_eq!(
        report.unknown_variables().count(),
        0,
        "{brand} {page}: every variable is read"
    );
    if brand != "aegs" {
        assert_eq!(
            has_bright_ink(image, RSI_NAME_COLUMNS, RSI_NAME_ROWS),
            !on_default,
            "{page}: the RSI footer names the page only when told"
        );
        return;
    }
    assert!(alpha_at(image, TAB_BAR) > 0, "{page}: the tab bar is drawn");
    assert!(
        has_bright_ink(image, ARROW_COLUMNS, ARROW_ROWS),
        "{page}: the arrows of the tab bar take a colour of the style"
    );
    let (widgets, canvases) = if on_default {
        (DEFAULT_PAGE.0, DEFAULT_PAGE.1)
    } else {
        (POWER_PAGE_WIDGETS, POWER_PAGE_CANVASES)
    };
    assert!(
        widgets.contains(&report.widgets) && report.canvases == canvases,
        "{page}: {} widgets in {} canvases",
        report.widgets,
        report.canvases
    );
}

/// Composes the powered MFD of a brand with the given game variables.
type Compose<'a> = dyn Fn(&str, &[&str]) -> Result<Composed, Box<dyn Error>> + 'a;

/// Opens the real Archive and calls `run` with a function that composes the MFD.
fn with_mfd(run: impl FnOnce(&Compose<'_>) -> TestResult) -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let font_file = ScreenComposer::new(&forge).font_file();
    let movie = UiMovie::from_archive(&archive, &format!("Data/{font_file}"))?;
    let fonts = movie.movie().assets().fonts;
    let texts = LocalizationEntry::read(&archive, Language::English)?;
    let ini = texts.parse()?;
    let lookup = |key: &str| {
        ini.resolve(key)
            .map(|text| decode_line_breaks(text).into_owned())
    };
    let images = ArchiveImages::new(&archive);
    let texture = |path: &str, side: u32, pixels: u64| {
        images
            .texture(path, side, pixels)
            .map_err(|error| error.to_string())
    };
    let vector = |path: &str| {
        images
            .vector(path, MAX_SVG_BYTES as u64)
            .map_err(|error| error.to_string())
    };
    run(&|brand, settings| {
        Ok(ScreenComposer::new(&forge)
            .with_texts(TextSources {
                fonts: &fonts,
                localise: Some(&lookup as Localise<'_>),
            })
            .with_images(ImageSources {
                texture: &texture,
                vector: &vector,
            })
            .with_variables(&variables(settings)?)
            .compose(MFD, Some(brand))?)
    })
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_powered_mfd_draws_no_solid_block_and_a_tab_bar_on_every_page() -> TestResult {
    let named = format!("landscapecanvasguid={POWER_PAGE}");
    let pages: [(&str, &[&str]); 2] = [
        ("the default page", &["powerstate=1"]),
        (
            "the power page",
            &["powerstate=1", &named, "mfdviewname=POWER MANAGEMENT"],
        ),
    ];
    with_mfd(|compose| {
        for brand in ["aegs", "rsi"] {
            for (page, settings) in pages {
                let composed = compose(brand, settings)?;
                assert_page(
                    brand,
                    (page, settings.len() == 1),
                    &composed,
                    &raster(&composed)?,
                );
            }
        }
        Ok(())
    })
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_aegis_shields_page_takes_the_default_canvas_and_draws_no_label_chip() -> TestResult {
    let named = format!("landscapecanvasguid={SHIELDS_PAGE}");
    let settings = ["powerstate=1", &named, "mfdviewname=SHIELDS"];
    with_mfd(|compose| {
        let composed = compose("aegs", &settings)?;
        assert!(
            alpha_at(&raster(&composed)?, SHIELDS_LABEL) < FAINT,
            "the shields label of the RSI canvas sits on a solid chip"
        );
        Ok(())
    })
}
