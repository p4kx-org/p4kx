// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The import budget of a render: movies, bytes (counted as the movie declares them, before
//! anything is decompressed) and fonts, with crafted movies that exceed it.

use p4kx_extraction::gfx::Movie;
use p4kx_extraction::{ImportIssue, MAX_IMPORTED_FONTS, MAX_IMPORTED_MOVIES, UiMovie};
use p4kx_testkit::{TestEntry, exports_body, font3_body, zlib};

use crate::ui_import_support::{CLIENT, TestResult, archive, client_of, glyph, options, stage};

const MIB: u32 = 1 << 20;

fn entry_path(name: &str) -> String {
    format!("Data\\UI\\Screens\\SWF\\{name}")
}

fn font_file(id: u16, name: &str) -> Vec<u8> {
    stage()
        .tag(75, &font3_body(id, "F", &[glyph(10_000)]))
        .tag(56, &exports_body(&[(id, name)]))
        .bytes()
}

fn declared(signature: [u8; 3], length: u32) -> Vec<u8> {
    let mut file = signature.to_vec();
    file.push(10);
    file.extend_from_slice(&length.to_le_bytes());
    file.extend_from_slice(&zlib(&[0; 16]));
    file
}

#[test]
fn a_render_reads_at_most_the_movie_budget_and_reports_the_rest() -> TestResult {
    let dir = tempfile::tempdir()?;
    let count = MAX_IMPORTED_MOVIES + 1;
    let urls: Vec<String> = (0..count).map(|at| format!("m{at}.gfx")).collect();
    let names: Vec<String> = (0..count).map(|at| format!("$F{at}")).collect();
    let imports: Vec<(&str, u16, &str)> = urls
        .iter()
        .zip(&names)
        .zip(10_u16..)
        .map(|((url, name), id)| (url.as_str(), id, name.as_str()))
        .collect();
    let fields: Vec<(u16, i32)> = (10_u16..).take(count).map(|id| (id, 0)).collect();
    let mut entries = vec![TestEntry::zstd(
        CLIENT,
        client_of(stage(), &imports, &fields),
    )];
    for (url, name) in urls.iter().zip(&names) {
        entries.push(TestEntry::zstd(entry_path(url), font_file(4, name)));
    }
    let archive = archive(dir.path(), entries)?;
    let render = UiMovie::from_archive(&archive, CLIENT)?.render(&options())?;
    assert_eq!(render.imports.movies.len(), MAX_IMPORTED_MOVIES);
    assert_eq!(render.imports.unresolved.len(), 1, "{:?}", render.imports);
    assert_eq!(render.imports.unresolved[0].issue, ImportIssue::OverBudget);
    assert_eq!(render.imports.unresolved[0].url, urls[MAX_IMPORTED_MOVIES]);
    Ok(())
}

#[test]
fn a_render_takes_at_most_the_font_budget_from_the_movies() -> TestResult {
    let dir = tempfile::tempdir()?;
    let count = MAX_IMPORTED_FONTS + 1;
    let names: Vec<String> = (0..count).map(|at| format!("$F{at}")).collect();
    let mut fonts = stage();
    let mut exports = Vec::new();
    for (name, id) in names.iter().zip(1_u16..) {
        fonts = fonts.tag(75, &font3_body(id, "F", &[glyph(10_000)]));
        exports.push((id, name.as_str()));
    }
    let fonts = fonts.tag(56, &exports_body(&exports)).bytes();
    let imports: Vec<(&str, u16, &str)> = names
        .iter()
        .zip(1_000_u16..)
        .map(|(name, id)| ("many.gfx", id, name.as_str()))
        .collect();
    let fields: Vec<(u16, i32)> = (1_000_u16..).take(count).map(|id| (id, 0)).collect();
    let archive = archive(
        dir.path(),
        vec![
            TestEntry::zstd(CLIENT, client_of(stage(), &imports, &fields)),
            TestEntry::zstd(entry_path("many.gfx"), fonts),
        ],
    )?;
    let render = UiMovie::from_archive(&archive, CLIENT)?.render(&options())?;
    assert_eq!(render.imports.movies.len(), 1);
    assert_eq!(render.imports.unresolved.len(), 1, "{:?}", render.imports);
    assert_eq!(render.imports.unresolved[0].issue, ImportIssue::OverBudget);
    assert_eq!(render.imports.unresolved[0].name, names[MAX_IMPORTED_FONTS]);
    Ok(())
}

#[test]
fn a_movie_declaring_more_than_a_movie_may_be_is_refused_without_using_the_budget() -> TestResult {
    for length in [u32::MAX, 300 * MIB, 65 * MIB] {
        let dir = tempfile::tempdir()?;
        let bomb = declared(*b"CWS", length);
        assert_eq!(Movie::declared_len(&bomb)?, length);
        let archive = archive(
            dir.path(),
            vec![
                TestEntry::zstd(
                    CLIENT,
                    client_of(
                        stage(),
                        &[("bomb.gfx", 9, "$Bomb"), ("good.gfx", 10, "$Font")],
                        &[(9, 0), (10, 0)],
                    ),
                ),
                TestEntry::zstd(entry_path("bomb.gfx"), bomb),
                TestEntry::zstd(entry_path("good.gfx"), font_file(4, "$Font")),
            ],
        )?;
        let render = UiMovie::from_archive(&archive, CLIENT)?.render(&options())?;
        assert_eq!(render.imports.unresolved.len(), 1, "{length}");
        assert!(
            matches!(
                render.imports.unresolved[0].issue,
                ImportIssue::Unreadable(_)
            ),
            "{length}: {:?}",
            render.imports.unresolved[0].issue
        );
        assert_eq!(
            render.imports.movies,
            [entry_path("good.gfx")],
            "{length}: a later import still loads"
        );
    }
    Ok(())
}

#[test]
fn declared_lengths_add_up_across_the_movies_of_a_render() -> TestResult {
    let dir = tempfile::tempdir()?;
    let names: Vec<String> = (0..5).map(|at| format!("$B{at}")).collect();
    let urls: Vec<String> = (0..5).map(|at| format!("b{at}.gfx")).collect();
    let imports: Vec<(&str, u16, &str)> = urls
        .iter()
        .zip(&names)
        .zip(10_u16..)
        .map(|((url, name), id)| (url.as_str(), id, name.as_str()))
        .collect();
    let fields: Vec<(u16, i32)> = (10_u16..).take(5).map(|id| (id, 0)).collect();
    let mut entries = vec![TestEntry::zstd(
        CLIENT,
        client_of(stage(), &imports, &fields),
    )];
    for url in &urls {
        entries.push(TestEntry::zstd(
            entry_path(url),
            declared(*b"CWS", 60 * MIB),
        ));
    }
    let archive = archive(dir.path(), entries)?;
    let render = UiMovie::from_archive(&archive, CLIENT)?.render(&options())?;
    let issues: Vec<&ImportIssue> = render
        .imports
        .unresolved
        .iter()
        .map(|import| &import.issue)
        .collect();
    assert_eq!(issues.len(), 5, "{issues:?}");
    assert!(
        issues[..4]
            .iter()
            .all(|issue| **issue != ImportIssue::OverBudget),
        "four of 60 MiB fit into 256 MiB: {issues:?}"
    );
    assert_eq!(*issues[4], ImportIssue::OverBudget, "the fifth does not");
    Ok(())
}

#[test]
fn a_movie_with_a_header_too_short_to_declare_a_length_is_unreadable() -> TestResult {
    for bytes in [
        b"CWS".to_vec(),
        b"CWS\x0a\x04\0\0\0".to_vec(),
        b"FWS\x0a".to_vec(),
    ] {
        let dir = tempfile::tempdir()?;
        let archive = archive(
            dir.path(),
            vec![
                TestEntry::zstd(
                    CLIENT,
                    client_of(stage(), &[("bad.gfx", 9, "$Font")], &[(9, 0)]),
                ),
                TestEntry::zstd(entry_path("bad.gfx"), bytes),
            ],
        )?;
        let render = UiMovie::from_archive(&archive, CLIENT)?.render(&options())?;
        assert_eq!(render.imports.unresolved.len(), 1);
        assert!(matches!(
            render.imports.unresolved[0].issue,
            ImportIssue::Unreadable(_)
        ));
    }
    Ok(())
}

#[test]
fn ranked_fallback_is_still_bounded_by_the_existing_movie_budget() -> TestResult {
    let dir = tempfile::tempdir()?;
    let url = "ranked.gfx";
    let wanted = "$Wanted";
    let paths: Vec<_> = (0..=MAX_IMPORTED_MOVIES)
        .map(|rank| format!("Data\\UI\\Rank\\{rank:02}\\{url}"))
        .collect();
    let mut entries = vec![TestEntry::zstd(
        CLIENT,
        client_of(stage(), &[(url, 9, wanted)], &[(9, 0)]),
    )];
    for (rank, path) in paths.iter().enumerate() {
        let name = if rank == MAX_IMPORTED_MOVIES {
            wanted
        } else {
            "$Other"
        };
        entries.push(TestEntry::zstd(path, font_file(4, name)));
    }
    let archive = archive(dir.path(), entries)?;
    let render = UiMovie::from_archive(&archive, CLIENT)?.render(&options())?;
    assert_eq!(
        render.imports.movies,
        paths[..MAX_IMPORTED_MOVIES],
        "the resolver reads the permitted candidates in ranking order"
    );
    let unresolved = render
        .imports
        .unresolved
        .first()
        .ok_or("budget diagnostic")?;
    assert_eq!(render.imports.unresolved.len(), 1);
    assert_eq!(
        unresolved.issue,
        ImportIssue::OverBudget,
        "the matching export is outside the read budget"
    );
    Ok(())
}
