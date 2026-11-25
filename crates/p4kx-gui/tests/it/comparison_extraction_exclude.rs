// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The exclusion Filters of the Extraction setup for the changes of one side of a Comparison.

use std::error::Error;

use crate::common::files_below;
use crate::comparing::{Pair, TestResult, pair, write};
use crate::comparison_extraction::{compared, poll_until};
use crate::extraction_outputs::assert_receipted_outputs;

use p4kx_gui::{ComparisonSide, Extraction, ExtractionScope, Model, TargetKind};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};

const PANEL: &str = "Data\\Textures\\panel_diff.dds";

/// Starts the Extraction of the changes of the new side with the exclusion `exclude`; returns
/// whether it started.
fn start(
    model: &mut Model,
    out: &std::path::Path,
    exclude: &str,
    convert: bool,
) -> Result<bool, Box<dyn Error>> {
    model.begin_extraction(ExtractionScope::Changes(ComparisonSide::New));
    let setup = model.extraction_setup_mut().ok_or("no setup")?;
    setup.set_kind(TargetKind::ExistingFolder);
    setup.set_folder(out.to_path_buf());
    setup.set_exclude(exclude.to_owned());
    setup.set_convert(convert);
    Ok(model.submit_extraction(|| {}))
}

fn finish(model: &mut Model) -> TestResult {
    poll_until(model, |model| {
        !model.extraction().is_some_and(Extraction::is_running)
    })
}

/// Returns a Comparison whose new side holds the changed texture of `PANEL` and one added file.
fn textures() -> Result<Pair, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let small = SplitDds::new(TestTextureFormat::LegacyDxt1, 16, 8)
        .split(2)
        .entries(PANEL);
    let large = SplitDds::new(TestTextureFormat::LegacyDxt1, 32, 16)
        .split(2)
        .entries(PANEL);
    let old = small
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry);
    let new = large
        .into_iter()
        .chain([TestEntry::stored("readme.txt", "readme")])
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry);
    let old = write(dir.path(), "LIVE", "4.3.1.10203", &old)?;
    let new = write(dir.path(), "PTU", "4.3.2.10400", &new)?;
    Ok(Pair { dir, old, new })
}

#[test]
fn filter_text_that_is_refused_selects_no_change_and_saves_no_report() -> TestResult {
    let pair = pair()?;
    let report = pair.dir.path().join("report.json");

    for text in ["a".repeat(2000), "*a".repeat(40)] {
        let mut model = compared(&pair, &text)?;
        let view = model.comparison_mut().view_mut().ok_or("no view")?;
        assert_eq!(view.change_count(ComparisonSide::New), 0, "no change");
        assert!(view.shown().is_empty(), "nothing shown");
        view.save_report(report.clone(), || {});
        assert!(view.report_status().is_none(), "no report started");
    }
    assert!(!report.exists(), "no report written");
    Ok(())
}

#[test]
fn the_exclusion_leaves_matching_changes_out() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out");

    assert!(start(&mut model, &out, "CRC\n\nengine/ADDED", false)?);
    finish(&mut model)?;

    assert_receipted_outputs(&out, &["Data\\Libs\\size.xml"])
}

#[test]
fn the_exclusion_narrows_what_the_filter_of_the_comparison_selected() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "Libs")?;
    let out = pair.dir.path().join("out");

    assert!(start(&mut model, &out, "size", false)?);
    finish(&mut model)?;

    assert_receipted_outputs(&out, &["Data\\Libs\\crc.xml"])
}

#[test]
fn an_exclusion_that_leaves_no_change_does_not_start() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out");

    let started = start(&mut model, &out, "xml\ncfg", false)?;

    assert!(!started, "nothing remains");
    let setup = model.extraction_setup().ok_or("the setup stays open")?;
    assert!(setup.leaves_nothing(), "the setup says why");
    assert!(!out.exists(), "nothing was written");
    Ok(())
}

#[test]
fn an_invalid_exclusion_does_not_start() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out");
    let long = "a".repeat(2000);

    for text in [
        long.as_str(),
        "  ",
        "*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a",
    ] {
        assert!(!start(&mut model, &out, text, false)?, "refused: {text:?}");
    }
    assert!(!out.exists(), "nothing was written");
    Ok(())
}

#[test]
fn an_excluded_texture_header_leaves_out_its_parts_too() -> TestResult {
    let pair = textures()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out");

    assert!(start(&mut model, &out, "*panel_diff.dds", false)?);
    finish(&mut model)?;

    assert_receipted_outputs(&out, &["readme.txt"])
}

#[test]
fn an_excluded_part_of_a_kept_texture_is_reported_and_still_builds_it() -> TestResult {
    let pair = textures()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out");

    assert!(start(&mut model, &out, "*.dds.1", true)?);
    finish(&mut model)?;

    let summary = model
        .extraction()
        .and_then(|extraction| match extraction.status() {
            p4kx_gui::ExtractionStatus::Finished(summary) => Some(summary.clone()),
            _ => None,
        })
        .ok_or("no summary")?;
    assert_eq!(
        summary.excluded_parts(),
        1,
        "the part is named in the summary"
    );
    let files = files_below(&out)?;
    assert!(
        files.iter().any(|path| path == PANEL),
        "the texture is merged from all its parts: {files:?}"
    );
    assert!(
        !files
            .iter()
            .any(|path| path == "Data\\Textures\\panel_diff.dds.1"),
        "the part is not written: {files:?}"
    );
    Ok(())
}
