// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use crate::wine::WineEnvironment;
use crate::wine_environment::PrefixDefaults;

const MACOS_BOTTLES: &str = "Library/Application Support/CrossOver/Bottles";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SeedKind {
    Direct,
    Steam,
    Library,
    Bottles,
    CrossOver,
    Games,
}

pub(crate) fn walk(environment: &WineEnvironment, mut visit: impl FnMut(SeedKind, &Path) -> bool) {
    let home = environment
        .home
        .as_deref()
        .filter(|path| path.is_absolute());
    if let Some(path) = environment.wine_prefix.as_deref()
        && !candidate(&mut visit, SeedKind::Direct, Path::new(path))
    {
        return;
    }
    if !explicit(&mut visit, SeedKind::Direct, &environment.wine_prefixes) {
        return;
    }
    if environment.defaults == PrefixDefaults::MacOs {
        macos(&mut visit, home, &environment.crossover_bottle_roots);
        return;
    }
    unix(&mut visit, home, environment);
}

fn unix(
    visit: &mut impl FnMut(SeedKind, &Path) -> bool,
    home: Option<&Path>,
    environment: &WineEnvironment,
) {
    if let Some(home) = home
        && !candidate(visit, SeedKind::Direct, &home.join("Games/star-citizen"))
    {
        return;
    }
    if let Some(path) = environment.compat_data.as_deref()
        && !candidate(visit, SeedKind::Direct, &Path::new(path).join("pfx"))
    {
        return;
    }
    if !explicit(visit, SeedKind::Steam, &environment.steam_roots) {
        return;
    }
    let data = environment
        .data_home
        .as_deref()
        .filter(|path| path.is_absolute())
        .map(Path::to_path_buf)
        .or_else(|| home.map(|path| path.join(".local/share")));
    if !steam_defaults(visit, home, data.as_deref()) {
        return;
    }
    if !explicit(visit, SeedKind::Library, &environment.steam_libraries) {
        return;
    }
    if !explicit(visit, SeedKind::Bottles, &environment.bottles_roots) {
        return;
    }
    if let Some(data) = data
        && !candidate(visit, SeedKind::Bottles, &data.join("bottles/bottles"))
    {
        return;
    }
    if !explicit(
        visit,
        SeedKind::CrossOver,
        &environment.crossover_bottle_roots,
    ) {
        return;
    }
    if let Some(home) = home {
        if !candidate(
            visit,
            SeedKind::Bottles,
            &home.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
        ) {
            return;
        }
        candidate(visit, SeedKind::Games, &home.join("Games"));
    }
}

fn macos(
    visit: &mut impl FnMut(SeedKind, &Path) -> bool,
    home: Option<&Path>,
    configured_bottles: &[PathBuf],
) {
    if let Some(home) = home
        && !candidate(visit, SeedKind::Direct, &home.join(".wine"))
    {
        return;
    }
    if !explicit(visit, SeedKind::CrossOver, configured_bottles) {
        return;
    }
    if let Some(home) = home
        && !candidate(visit, SeedKind::CrossOver, &home.join(MACOS_BOTTLES))
    {
        return;
    }
    candidate(
        visit,
        SeedKind::CrossOver,
        &Path::new("/").join(MACOS_BOTTLES),
    );
}

fn steam_defaults(
    visit: &mut impl FnMut(SeedKind, &Path) -> bool,
    home: Option<&Path>,
    data: Option<&Path>,
) -> bool {
    if let Some(data) = data
        && !candidate(visit, SeedKind::Steam, &data.join("Steam"))
    {
        return false;
    }
    if let Some(home) = home {
        for folder in [
            ".steam/steam",
            ".steam/root",
            ".local/share/Steam",
            ".var/app/com.valvesoftware.Steam/data/Steam",
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
        ] {
            if !candidate(visit, SeedKind::Steam, &home.join(folder)) {
                return false;
            }
        }
    }
    true
}

fn explicit(
    visit: &mut impl FnMut(SeedKind, &Path) -> bool,
    kind: SeedKind,
    paths: &[PathBuf],
) -> bool {
    paths.iter().all(|path| candidate(visit, kind, path))
}

fn candidate(visit: &mut impl FnMut(SeedKind, &Path) -> bool, kind: SeedKind, path: &Path) -> bool {
    !path.is_absolute() || visit(kind, path)
}
