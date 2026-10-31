// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The powered MFD of the Avenger Titan: the page its variables choose and its tab bar.

use std::error::Error;
use std::ops::Range;

use p4kx_extraction::{Archive, ArchiveImages, DataForgeEntry};
use p4kx_gfx::{Background, CanvasSize, RasterOptions, RgbaImage};
use p4kx_screens::{Composed, ImageSources, MAX_SVG_BYTES, ScreenComposer, Variable};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const MFD: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/general/m_mfd_screen";
const POWER_PAGE: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/power/mc_s_power_master";
const WIDTH: u32 = 800;

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

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_powered_mfd_draws_no_solid_block_and_a_tab_bar_on_every_page() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
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
    let named = format!("landscapecanvasguid={POWER_PAGE}");
    let pages: [(&str, &[&str]); 2] = [
        ("the default page", &["powerstate=1"]),
        (
            "the power page",
            &["powerstate=1", &named, "mfdviewname=POWER MANAGEMENT"],
        ),
    ];
    let mut widgets = Vec::new();
    for brand in ["aegs", "rsi"] {
        for (page, settings) in pages {
            let composed = ScreenComposer::new(&forge)
                .with_images(ImageSources {
                    texture: &texture,
                    vector: &vector,
                })
                .with_variables(&variables(settings)?)
                .compose(MFD, Some(brand))?;
            let image = raster(&composed)?;
            if settings.len() == 1 {
                assert_eq!(alpha_at(&image, BACKDROP), 0, "{brand}: no backdrop");
                assert_eq!(alpha_at(&image, DIAGRAM), 0, "{brand}: no diagram box");
            }
            assert_eq!(composed.report().unknown_variables().count(), 0);
            if brand == "aegs" {
                assert!(
                    alpha_at(&image, TAB_BAR) > 0,
                    "{page}: the tab bar is drawn"
                );
                assert!(
                    has_bright_ink(&image, ARROW_COLUMNS, ARROW_ROWS),
                    "{page}: the arrows of the tab bar take a colour of the style"
                );
                widgets.push(composed.report().widgets);
            }
        }
    }
    assert!(
        widgets.first() != widgets.last(),
        "the variable chose another page: {widgets:?}"
    );
    Ok(())
}
