// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The exclusion Filters of the Extraction setup: which Entries they leave out, what they leave
//! of a texture, and the problems they report.

use crate::common;
use crate::extraction_outputs::assert_receipted_outputs;

use common::{TestResult, check, files_below, finished, setup, setup_with};
use p4kx_gui::{ExtractionScope, Model, TargetKind};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat};

const PANEL: &str = "Data\\Textures\\panel_diff.dds";

fn start(model: &mut Model, scope: ExtractionScope, exclude: &str, convert: bool) -> bool {
    model.begin_extraction(scope);
    let Some(setup) = model.extraction_setup_mut() else {
        return false;
    };
    setup.set_kind(TargetKind::ExistingFolder);
    setup.set_exclude(exclude.to_owned());
    setup.set_convert(convert);
    model.submit_extraction(|| {})
}

/// Starts an Extraction of every shown Entry, which asks for confirmation, and confirms it.
fn confirmed_start(model: &mut Model, exclude: &str, convert: bool) {
    start(model, ExtractionScope::Shown, exclude, convert);
    model.submit_extraction(|| {});
}

fn textures() -> Vec<TestEntry> {
    let mut entries = SplitDds::new(TestTextureFormat::LegacyDxt1, 16, 8)
        .split(2)
        .entries(PANEL);
    entries.push(TestEntry::stored("readme.txt", "readme"));
    entries
}

#[test]
fn the_exclusion_leaves_matching_entries_out_of_the_checked_ones() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    model.set_extraction_folder(Some(out.clone()));
    check(&mut model, &["ship.cgf", "anchor.cgf", "readme.txt"])?;

    let started = start(&mut model, ExtractionScope::Selected, "Objects/SHIP", false);
    let summary = finished(&mut model)?;

    assert!(started, "two Entries remain");
    assert_eq!(summary.extracted(), 2, "the ship is left out");
    assert_receipted_outputs(&out, &["Data\\Objects\\anchor.cgf", "readme.txt"])
}

#[test]
fn one_filter_per_line_and_blank_lines_are_ignored() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    model.set_extraction_folder(Some(out.clone()));
    check(&mut model, &["ship.cgf", "anchor.cgf", "readme.txt"])?;

    start(
        &mut model,
        ExtractionScope::Selected,
        "ship\n\nREADME\r\n",
        false,
    );
    let summary = finished(&mut model)?;

    assert_eq!(summary.extracted(), 1, "only the anchor remains");
    assert_receipted_outputs(&out, &["Data\\Objects\\anchor.cgf"])
}

#[test]
fn the_confirmation_of_the_whole_archive_counts_what_remains() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    model.set_extraction_folder(Some(out));

    let first = start(&mut model, ExtractionScope::All, "Objects", false);

    assert!(!first, "a whole-Archive Extraction asks first");
    let asked = model
        .extraction_setup()
        .and_then(p4kx_gui::ExtractionSetup::confirmation)
        .ok_or("no confirmation")?;
    assert_eq!(asked.entries, 2, "config.xml and readme.txt remain");
    assert_eq!(asked.bytes, 12, "their sizes");
    Ok(())
}

#[test]
fn an_exclusion_that_leaves_nothing_does_not_start_and_says_so() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    model.set_extraction_folder(Some(out.clone()));
    check(&mut model, &["ship.cgf"])?;

    let started = start(&mut model, ExtractionScope::Selected, "ship", false);

    assert!(!started, "nothing would be extracted");
    assert!(model.extraction().is_none(), "no Extraction runs");
    let setup = model.extraction_setup_mut().ok_or("the setup stays open")?;
    assert!(setup.leaves_nothing(), "the setup says why");
    setup.set_exclude("other".to_owned());
    assert!(!setup.leaves_nothing(), "editing the text withdraws it");
    assert!(files_below(&out)?.is_empty(), "nothing was written");
    Ok(())
}

#[test]
fn a_filter_that_is_not_accepted_does_not_start() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    model.set_extraction_folder(Some(out));
    check(&mut model, &["readme.txt"])?;
    let too_long = "a".repeat(p4kx_extraction::MAX_FILTER_LEN + 1);

    let started = start(&mut model, ExtractionScope::Selected, &too_long, false);

    assert!(!started, "the Filter is refused");
    let setup = model.extraction_setup().ok_or("the setup stays open")?;
    assert!(
        matches!(
            setup.exclusion(),
            Err(p4kx_extraction::FilterError::TooLong { .. })
        ),
        "the problem is named"
    );
    Ok(())
}

#[test]
fn a_line_of_spaces_is_refused_while_an_empty_line_is_ignored() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    model.set_extraction_folder(Some(out));
    check(&mut model, &["readme.txt"])?;

    let started = start(&mut model, ExtractionScope::Selected, "ship\n   \n", false);

    assert!(
        !started,
        "a blank line would leave out every path with a space"
    );
    let setup = model.extraction_setup_mut().ok_or("the setup stays open")?;
    assert_eq!(
        setup.exclusion().err(),
        Some(p4kx_extraction::FilterError::EmptyExclusion),
        "the problem is named"
    );
    setup.set_exclude("ship\n\n".to_owned());
    assert!(setup.exclusion().is_ok(), "empty lines are ignored");
    Ok(())
}

#[test]
fn an_excluded_texture_header_takes_its_parts_with_it() -> TestResult {
    let (_dir, mut model, out) = setup_with(textures())?;
    model.set_extraction_folder(Some(out.clone()));

    confirmed_start(&mut model, "panel_diff", true);
    let summary = finished(&mut model)?;

    assert_eq!(summary.extracted(), 1, "only readme.txt");
    assert_eq!(summary.excluded_parts(), 0, "no texture stays");
    assert_receipted_outputs(&out, &["readme.txt"])
}

#[test]
fn an_excluded_part_of_a_kept_texture_is_counted_and_the_texture_is_still_built() -> TestResult {
    let (_dir, mut model, out) = setup_with(textures())?;
    model.set_extraction_folder(Some(out.clone()));

    confirmed_start(&mut model, "*.dds.?", true);
    let summary = finished(&mut model)?;

    assert_eq!(summary.excluded_parts(), 2, "both parts are named");
    let written = files_below(&out)?;
    assert!(
        written.contains(&"Data\\Textures\\panel_diff.dds".to_owned()),
        "the texture is merged from its parts: {written:?}"
    );
    Ok(())
}

#[test]
fn excluded_parts_are_not_reported_without_a_conversion() -> TestResult {
    let (_dir, mut model, out) = setup_with(textures())?;
    model.set_extraction_folder(Some(out.clone()));

    confirmed_start(&mut model, "*.dds.?", false);
    let summary = finished(&mut model)?;

    assert_eq!(
        summary.excluded_parts(),
        0,
        "the parts are simply not written"
    );
    assert_receipted_outputs(&out, &[PANEL, "readme.txt"])
}
