// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The text presets of the powered MFD all have a size, and the page name in the tab bar keeps the
//! size its style entries give it.

use std::error::Error;
use std::ops::Range;

use p4kx_extraction::gfx::Localise;
use p4kx_extraction::{
    Archive, DataForgeEntry, Language, LocalizationEntry, UiMovie, decode_line_breaks,
};
use p4kx_gfx::{Background, CanvasSize, RasterOptions, RgbaImage};
use p4kx_screens::{ScreenComposer, TextSources, Variable};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const MFD: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/general/m_mfd_screen";
const POWER_PAGE: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/power/mc_s_power_master";
const WIDTH: u32 = 800;

/// The columns and rows the page name of the tab bar stands in, at 800 pixels wide.
const NAME_COLUMNS: Range<u32> = 250..550;
const NAME_ROWS: Range<u32> = 548..600;
/// The height in pixels of the capitals of the page name on LIVE, measured at 20: its size is set
/// by the style entries of the screen, so the presets must leave it as it is.
const NAME_HEIGHT: Range<u32> = 18..23;

fn is_ink(image: &RgbaImage, column: u32, row: u32) -> bool {
    let at = (row as usize * image.width() as usize + column as usize) * 4;
    matches!(image.pixels().get(at..at + 4), Some(&[red, green, blue, alpha])
        if alpha > 200 && red.max(green).max(blue) > 150)
}

/// Returns the number of rows between the first and the last row of the box that hold ink.
fn ink_height(image: &RgbaImage, columns: Range<u32>, rows: Range<u32>) -> u32 {
    let inked: Vec<u32> = rows
        .filter(|&row| columns.clone().any(|column| is_ink(image, column, row)))
        .collect();
    match (inked.first(), inked.last()) {
        (Some(first), Some(last)) => last - first + 1,
        _ => 0,
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_presets_of_the_powered_mfd_have_a_size_and_the_page_name_keeps_its_size() -> TestResult {
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
    let settings = [
        "powerstate=1".to_owned(),
        format!("landscapecanvasguid={POWER_PAGE}"),
        "mfdviewname=POWER MANAGEMENT".to_owned(),
    ];
    let variables = settings
        .iter()
        .map(|setting| Ok(setting.parse::<Variable>()?))
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let composed = ScreenComposer::new(&forge)
        .with_texts(TextSources {
            fonts: &fonts,
            localise: Some(&lookup as Localise<'_>),
        })
        .with_variables(&variables)
        .compose(MFD, Some("aegs"))?;
    let unresolved: Vec<&str> = composed
        .report()
        .unsupported
        .iter()
        .filter(|entry| entry.reason.contains("has no font size"))
        .map(|entry| entry.reason.as_str())
        .collect();
    assert!(
        unresolved.is_empty(),
        "presets without a size: {unresolved:?}"
    );
    let image = composed
        .rasterize(&RasterOptions {
            size: CanvasSize::Width(WIDTH),
            background: Background::Transparent,
        })?
        .image;
    let height = ink_height(&image, NAME_COLUMNS, NAME_ROWS);
    assert!(
        NAME_HEIGHT.contains(&height),
        "the page name is {height} pixels tall"
    );
    Ok(())
}
