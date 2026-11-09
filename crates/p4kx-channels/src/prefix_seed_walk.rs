// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use crate::wine::WineEnvironment;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SeedKind {
    Direct,
    Steam,
    Library,
    Bottles,
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
    if let Some(home) = home
        && !candidate(
            &mut visit,
            SeedKind::Direct,
            &home.join("Games/star-citizen"),
        )
    {
        return;
    }
    if let Some(path) = environment.compat_data.as_deref()
        && !candidate(&mut visit, SeedKind::Direct, &Path::new(path).join("pfx"))
    {
        return;
    }
    if !explicit(&mut visit, SeedKind::Steam, &environment.steam_roots) {
        return;
    }
    let data = environment
        .data_home
        .as_deref()
        .filter(|path| path.is_absolute())
        .map(Path::to_path_buf)
        .or_else(|| home.map(|path| path.join(".local/share")));
    if !steam_defaults(&mut visit, home, data.as_deref()) {
        return;
    }
    if !explicit(&mut visit, SeedKind::Library, &environment.steam_libraries) {
        return;
    }
    if !explicit(&mut visit, SeedKind::Bottles, &environment.bottles_roots) {
        return;
    }
    if let Some(data) = data
        && !candidate(&mut visit, SeedKind::Bottles, &data.join("bottles/bottles"))
    {
        return;
    }
    if let Some(home) = home {
        if !candidate(
            &mut visit,
            SeedKind::Bottles,
            &home.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
        ) {
            return;
        }
        candidate(&mut visit, SeedKind::Games, &home.join("Games"));
    }
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
    paths: &[std::path::PathBuf],
) -> bool {
    paths.iter().all(|path| candidate(visit, kind, path))
}

fn candidate(visit: &mut impl FnMut(SeedKind, &Path) -> bool, kind: SeedKind, path: &Path) -> bool {
    !path.is_absolute() || visit(kind, path)
}
