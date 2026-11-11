// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use p4kx_archive::Archive;
use p4kx_localization::Language;
use rayon::prelude::*;

const UI_FOLDER: &str = "data\\ui\\";
const ENGLISH_FOLDER: &str = "data\\localization\\english\\";

pub(crate) fn folder_of(path: &str) -> &str {
    path.rfind(['\\', '/']).map_or("", |at| &path[..at])
}

fn normalised(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split(['\\', '/']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("\\").to_ascii_lowercase()
}

fn swapped(name: &str) -> Option<String> {
    let (stem, extension) = name.rsplit_once('.')?;
    let other = match extension.to_ascii_lowercase().as_str() {
        "swf" => "gfx",
        "gfx" => "swf",
        _ => return None,
    };
    Some(format!("{stem}.{other}"))
}

/// Returns the URL as a relative path that stays inside the folder of the importing movie, or
/// `None` when it is empty, rooted, carries a drive or stream prefix, or climbs above that folder.
pub(super) fn contained(url: &str) -> Option<PathBuf> {
    let mut parts: Vec<&str> = Vec::new();
    for part in url.split(['\\', '/']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part if part.contains([':', '\0']) => return None,
            part => parts.push(part),
        }
    }
    if parts.is_empty() || url.starts_with(['\\', '/']) {
        return None;
    }
    Some(parts.iter().collect())
}

/// Returns the folders below `Data\Localization` that hold the movies of `language`: the folder
/// of its `global.ini` and the shorter name without the region (`german`) that the movie folders
/// of the Archive use.
fn language_folders(language: Language) -> Vec<String> {
    let full = language.folder();
    let short = full.split_once("_(").map_or(full, |(name, _)| name);
    let mut folders = vec![format!("data\\localization\\{full}\\")];
    if short != full {
        folders.push(format!("data\\localization\\{short}\\"));
    }
    folders
}

/// Returns the indexes of the Entries that a movie in `folder` may mean by the import
/// URL `url`, the best first: the exact path, the same tail below the UI folder or the
/// localisation folder of `language` and then of English, the same file name there, then
/// anywhere below the UI folder, then anywhere. A `.swf` and a
/// `.gfx` of the same name are alternatives, the one the URL names first; Entries that tie on
/// all of it keep the order of the Archive.
pub(crate) fn candidates(
    archive: &Archive,
    url: &str,
    folder: &str,
    language: Language,
) -> impl Iterator<Item = usize> {
    let language_folders = language_folders(language);
    let fallback = language != Language::English;
    let relative = normalised(&format!("{folder}\\{url}"));
    let file = url.rsplit(['\\', '/']).next().unwrap_or(url);
    let other = swapped(file);
    let direct = normalised(url);
    let suffix = format!("\\{direct}");
    let mut found: Vec<(usize, String, usize)> = archive
        .entries()
        .par_iter()
        .enumerate()
        .filter_map(|(at, entry)| {
            let name = entry.path().rsplit(['\\', '/']).next()?;
            let twin = if name.eq_ignore_ascii_case(file) {
                0
            } else if other
                .as_deref()
                .is_some_and(|other| name.eq_ignore_ascii_case(other))
            {
                1
            } else {
                return None;
            };
            let path = normalised(entry.path());
            let suffixed = format!("\\{path}").ends_with(&suffix);
            let place = if path == relative || path == direct {
                0
            } else if suffixed && path.starts_with(UI_FOLDER) {
                1
            } else if suffixed && language_folders.iter().any(|dir| path.starts_with(dir)) {
                2
            } else if language_folders
                .iter()
                .any(|dir| folder_of(&path) == dir.trim_end_matches('\\'))
            {
                3
            } else if fallback && suffixed && path.starts_with(ENGLISH_FOLDER) {
                4
            } else if fallback && folder_of(&path) == ENGLISH_FOLDER.trim_end_matches('\\') {
                5
            } else if path.starts_with(UI_FOLDER) {
                4 + 2 * usize::from(fallback)
            } else {
                5 + 2 * usize::from(fallback)
            };
            Some((twin + 2 * place, path, at))
        })
        .collect();
    found.sort_unstable_by(|left, right| {
        (left.0, &left.1, left.2).cmp(&(right.0, &right.1, right.2))
    });
    found.into_iter().map(|(_, _, at)| at)
}
