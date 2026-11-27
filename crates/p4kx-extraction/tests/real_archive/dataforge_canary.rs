// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Canary for the DataForge format details no tool interprets: when a game update changes one,
//! these tests fail or report it before the reader reads wrong data.

use std::path::Path;

use p4kx_extraction::{Archive, DataForgeEntry, FormatSurvey};
use p4kx_testkit::{real_archive_b_path, real_archive_path};

use super::{TestResult, report};

fn survey_of(path: &Path) -> Result<(FormatSurvey, usize, usize), Box<dyn std::error::Error>> {
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let survey = forge.survey()?;
    report(&format!(
        "{}: DataForge version {}, {} Records, {survey:?}, {} issues",
        path.display(),
        forge.version(),
        forge.record_count(),
        forge.issue_count()
    ));
    Ok((survey, forge.record_count(), forge.issue_count()))
}

fn check(path: &Path) -> TestResult {
    let (survey, records, issues) = survey_of(path)?;

    assert_eq!(
        survey.pointers.dangling, 0,
        "every pointer is null or names an instance"
    );
    assert_eq!(
        survey.flagged_properties, 0,
        "the conversion type of every property fits the low byte"
    );
    assert_eq!(survey.unreadable_structs, 0, "every struct layout is read");
    assert_eq!(
        survey.trailing.equals_struct_size, records,
        "the last 16 bits of every Record row are the size of its struct"
    );
    assert_eq!(survey.header_words, [0, 0], "the header words are zero");
    assert_eq!(
        issues,
        survey.flagged_properties + survey.records_without_name + survey.records_without_tag,
        "every deviation is an issue"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_dataforge_format_details_hold_in_a_real_archive() -> TestResult {
    check(&real_archive_path()?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_dataforge_format_details_hold_in_a_second_real_archive() -> TestResult {
    real_archive_b_path().map_or(Ok(()), |path| check(&path))
}
