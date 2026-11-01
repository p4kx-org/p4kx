// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! What the Linux desktop reads from the repository: the desktop file, the icon set and the
//! features of the windowing, dialog and clipboard dependencies.
//!
//! `P4KX_WRITE_LOGO_ASSETS=1` regenerates the icon set in `packaging/linux/icons` before it is
//! checked.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_gui::theme::logo::{self, Variant};
use p4kx_gui::window::APP_ID;

use crate::logo::{decode, encode};

type TestResult = Result<(), Box<dyn Error>>;

/// The sides of the PNG icons of the `hicolor` theme, in pixels.
const ICON_SIZES: [u32; 8] = [16, 24, 32, 48, 64, 128, 256, 512];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn linux() -> PathBuf {
    repository().join("packaging/linux")
}

fn png_icon(size: u32) -> PathBuf {
    linux()
        .join("icons/hicolor")
        .join(format!("{size}x{size}/apps/p4kx.png"))
}

fn svg_icon() -> PathBuf {
    linux().join("icons/hicolor/scalable/apps/p4kx.svg")
}

/// Returns the value of `key` in the `[Desktop Entry]` group of `desktop`.
fn entry<'a>(desktop: &'a str, key: &str) -> Option<&'a str> {
    desktop
        .lines()
        .take_while(|line| !line.starts_with('[') || *line == "[Desktop Entry]")
        .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
}

#[test]
fn the_desktop_file_launches_the_window_under_the_application_id() -> TestResult {
    let desktop = std::fs::read_to_string(linux().join("p4kx.desktop"))?;

    assert_eq!(entry(&desktop, "Type"), Some("Application"), "type");
    assert_eq!(entry(&desktop, "Name"), Some("P4KX"), "name");
    assert_eq!(entry(&desktop, "Exec"), Some("p4kx-gui %f"), "command");
    assert_eq!(entry(&desktop, "Icon"), Some("p4kx"), "icon name");
    assert_eq!(entry(&desktop, "Terminal"), Some("false"), "no terminal");
    assert_eq!(
        entry(&desktop, "StartupWMClass"),
        Some(APP_ID),
        "the window class the window sets"
    );
    assert!(
        entry(&desktop, "Categories").is_some_and(|categories| categories.ends_with(';')),
        "categories end with a semicolon"
    );
    Ok(())
}

fn write_icons_when_asked() -> TestResult {
    if std::env::var_os("P4KX_WRITE_LOGO_ASSETS").is_none() {
        return Ok(());
    }
    for size in ICON_SIZES {
        let file = png_icon(size);
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::write(file, encode(size, &logo::rgba(Variant::Tile, size))?)?;
    }
    std::fs::copy(repository().join("docs/design/p4kx-logo.svg"), svg_icon())?;
    Ok(())
}

#[test]
fn the_icon_set_is_the_rasterised_logo_at_every_size() -> TestResult {
    write_icons_when_asked()?;

    for size in ICON_SIZES {
        let (width, height, pixels) = decode(&std::fs::read(png_icon(size))?)?;
        assert_eq!((width, height), (size, size), "{size} px icon");
        assert_eq!(pixels, logo::rgba(Variant::Tile, size), "{size} px pixels");
    }
    assert_eq!(
        std::fs::read_to_string(svg_icon())?,
        std::fs::read_to_string(repository().join("docs/design/p4kx-logo.svg"))?,
        "the scalable icon is the logo"
    );
    Ok(())
}

/// Returns the line of `manifest` that declares `dependency`.
fn declaration<'a>(manifest: &'a str, dependency: &str) -> Option<&'a str> {
    manifest
        .lines()
        .find(|line| line.starts_with(&format!("{dependency} = ")))
}

#[test]
fn the_window_runs_on_x11_and_wayland_through_wgpu() -> TestResult {
    let manifest = std::fs::read_to_string(repository().join("Cargo.toml"))?;
    let eframe = declaration(&manifest, "eframe").ok_or("eframe is declared")?;

    for feature in ["\"wgpu\"", "\"x11\"", "\"wayland\""] {
        assert!(eframe.contains(feature), "eframe enables {feature}");
    }
    Ok(())
}

#[test]
fn the_clipboard_reaches_other_programs_on_unix() -> TestResult {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))?;
    let unix = manifest
        .split("[target.'cfg(unix)'.dependencies]")
        .nth(1)
        .and_then(|rest| rest.split("\n[").next())
        .ok_or("a unix dependency table")?;

    assert!(
        declaration(unix, "eframe").is_some_and(|eframe| eframe.contains("\"clipboard\"")),
        "eframe enables the clipboard"
    );
    Ok(())
}
