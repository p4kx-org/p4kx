// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Which Entry an import URL names when several have its file name.

use std::error::Error;

use p4kx_extraction::{ImportIssue, Language, UiMovie};
use p4kx_testkit::TestEntry;

use crate::ui_import_support::{
    CLIENT, GREEN, TestResult, archive, client, font_movie, options, pixel,
};

fn picked(language: Option<Language>, url: &str, paths: &[&str]) -> Result<String, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let mut entries = vec![TestEntry::zstd(CLIENT, client(url, "$Font"))];
    entries.extend(
        paths
            .iter()
            .map(|path| TestEntry::zstd(*path, font_movie("$Font"))),
    );
    let archive = archive(dir.path(), entries)?;
    let mut movie = UiMovie::from_archive(&archive, CLIENT)?;
    if let Some(language) = language {
        movie = movie.with_font_language(language);
    }
    Ok(movie.import_source(url)?.name().to_owned())
}

#[test]
fn the_exact_path_beats_every_other_place() -> TestResult {
    let chosen = picked(
        None,
        "X.swf",
        &[
            "Data\\UI\\Q\\X.swf",
            "Data\\Localization\\english\\X.swf",
            "Data\\UI\\Screens\\SWF\\X.swf",
        ],
    )?;
    assert_eq!(chosen, "Data\\UI\\Screens\\SWF\\X.swf");
    Ok(())
}

#[test]
fn the_places_rank_tail_below_ui_then_below_the_language_then_the_file_there_then_any_ui_file()
-> TestResult {
    let tail_ui = "Data\\UI\\z\\a\\X.swf";
    let tail_language = "Data\\Localization\\english\\a\\X.swf";
    let file_language = "Data\\Localization\\english\\X.swf";
    let any_ui = "Data\\UI\\z\\X.swf";
    let other = "Data\\Other\\a\\X.swf";
    let url = "a/X.swf";
    for (paths, expected) in [
        (
            vec![other, any_ui, file_language, tail_language, tail_ui],
            tail_ui,
        ),
        (
            vec![other, any_ui, file_language, tail_language],
            tail_language,
        ),
        (vec![other, any_ui, file_language], file_language),
        (vec![other, any_ui], any_ui),
        (vec![other], other),
    ] {
        assert_eq!(picked(None, url, &paths)?, expected, "{paths:?}");
    }
    Ok(())
}

#[test]
fn the_english_localisation_folder_is_the_fallback_for_a_movie_only_it_holds() -> TestResult {
    let chosen = picked(
        None,
        "HUD_Font_LocFont.swf",
        &[
            "Data\\Other\\HUD_Font_LocFont.swf",
            "Data\\Localization\\english\\HUD_Font_LocFont.swf",
        ],
    )?;
    assert_eq!(chosen, "Data\\Localization\\english\\HUD_Font_LocFont.swf");
    Ok(())
}

#[test]
fn the_font_language_picks_its_localisation_folder() -> TestResult {
    let english = "Data\\Localization\\english\\X.swf";
    let german = "Data\\Localization\\german_(germany)\\X.swf";
    let other = "Data\\Other\\X.swf";
    let paths = [other, english, german];
    assert_eq!(
        picked(None, "X.swf", &paths)?,
        english,
        "English by default"
    );
    assert_eq!(
        picked(Some(Language::German), "X.swf", &paths)?,
        german,
        "the language given"
    );
    assert_eq!(picked(Some(Language::English), "X.swf", &paths)?, english);
    Ok(())
}

#[test]
fn the_urls_own_extension_comes_before_its_twin() -> TestResult {
    let both = ["Data\\UI\\Q\\X.gfx", "Data\\UI\\Q\\X.swf"];
    assert_eq!(picked(None, "X.swf", &both)?, "Data\\UI\\Q\\X.swf");
    assert_eq!(picked(None, "X.gfx", &both)?, "Data\\UI\\Q\\X.gfx");
    assert_eq!(
        picked(None, "X.swf", &["Data\\UI\\Q\\X.gfx"])?,
        "Data\\UI\\Q\\X.gfx",
        "the twin stands in when the file itself is missing"
    );
    assert_eq!(
        picked(None, "X.swf", &["Data\\Other\\X.swf", "Data\\UI\\Q\\X.gfx"])?,
        "Data\\UI\\Q\\X.gfx",
        "a twin below the UI folder beats the file elsewhere"
    );
    Ok(())
}

#[test]
fn entries_that_differ_only_in_case_sort_by_their_place_in_the_archive() -> TestResult {
    for paths in [
        ["Data\\UI\\F\\Font.gfx", "Data\\UI\\F\\font.gfx"],
        ["Data\\UI\\F\\font.gfx", "Data\\UI\\F\\Font.gfx"],
    ] {
        let dir = tempfile::tempdir()?;
        let entries = std::iter::once(TestEntry::zstd(CLIENT, client("font.gfx", "$Font")))
            .chain(
                paths
                    .iter()
                    .map(|path| TestEntry::zstd(*path, font_movie("$Font"))),
            )
            .collect();
        let archive = archive(dir.path(), entries)?;
        let first = archive
            .entries()
            .iter()
            .find(|entry| entry.path().eq_ignore_ascii_case(paths[0]))
            .map(|entry| entry.path().to_owned())
            .ok_or("no font Entry")?;
        let movie = UiMovie::from_archive(&archive, CLIENT)?;
        assert_eq!(movie.import_source("font.gfx")?.name(), first, "{paths:?}");
    }
    Ok(())
}

type Outcome = (Vec<String>, Vec<ImportIssue>, [u8; 4]);

fn named_movie(url: &str, paths: &[(&str, &str)]) -> Result<Outcome, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let mut entries = vec![TestEntry::zstd(CLIENT, client(url, "$Font"))];
    entries.extend(
        paths
            .iter()
            .map(|(path, name)| TestEntry::zstd(*path, font_movie(name))),
    );
    let archive = archive(dir.path(), entries)?;
    let render = UiMovie::from_archive(&archive, CLIENT)?.render(&options())?;
    let issues = render
        .imports
        .unresolved
        .iter()
        .map(|import| import.issue.clone())
        .collect();
    let drawn = pixel(render.image.pixels(), (6, 36)).try_into()?;
    Ok((render.imports.movies, issues, drawn))
}

#[test]
fn ranked_candidates_are_tried_until_one_exports_the_requested_font() -> TestResult {
    let best = "Data\\UI\\Screens\\SWF\\X.swf";
    let second = "Data\\UI\\Q\\X.swf";
    let third = "Data\\UI\\R\\X.swf";
    let (movies, issues, drawn) = named_movie(
        "X.swf",
        &[(best, "$Other"), (second, "$Font"), (third, "$Font")],
    )?;
    assert_eq!(movies, [best, second]);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(drawn, GREEN);
    let (movies, issues, drawn) = named_movie(
        "X.swf",
        &[(best, "$Other"), (second, "$Other"), (third, "$Font")],
    )?;
    assert_eq!(
        movies,
        [best, second, third],
        "the third owns the requested export"
    );
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(drawn, GREEN, "the successful candidate supplied the glyph");
    Ok(())
}
