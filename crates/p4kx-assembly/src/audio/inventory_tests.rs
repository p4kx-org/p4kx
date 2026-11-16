// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_archive::Archive;
use p4kx_extraction::CancelToken;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry};

use super::{AudioCandidateState, AudioIndex, AudioSurvey, AudioSurveyKind};
use crate::audio::{AtlDefinitions, MAX_ATL_LEN, index_read};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn audio_surveys_keep_real_reads_capacity_and_physical_aliases() -> TestResult {
    const BNK: &str = "Data/Sounds/wwise/Same.bnk";
    const ATL: &str = "Data/Libs/GameAudio/Same.xml";
    let bank = TestBank::new(1).bytes();
    let definition = TestAtl::new("test").trigger("Fire", &["Event"]).to_xml();
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(BNK, bank.as_slice()))
        .entry(TestEntry::stored(ATL, b"too large").with_uncompressed_size(MAX_ATL_LEN + 1))
        .entry(TestEntry::stored(BNK, b"broken"))
        .entry(TestEntry::stored(ATL, definition.as_bytes()))
        .entry(TestEntry::stored("Data/Unrelated.xml", b"untouched"))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let index = AudioIndex::build(&archive);
    assert_survey(
        &index,
        &archive,
        AudioSurveyKind::Soundbanks,
        &[
            (
                0,
                AudioCandidateState::Admitted,
                Some(bank.len() as u64),
                true,
            ),
            (2, AudioCandidateState::Unavailable, Some(6), false),
        ],
    )?;
    assert_survey(
        &index,
        &archive,
        AudioSurveyKind::AtlDefinitions,
        &[
            (1, AudioCandidateState::CapacityLimited, None, false),
            (
                3,
                AudioCandidateState::Admitted,
                Some(definition.len() as u64),
                true,
            ),
        ],
    )?;
    let reopen = Archive::open(&path)?;
    assert!(
        index.survey(&reopen, AudioSurveyKind::Soundbanks).is_none(),
        "an independently opened copy cannot supply these candidates"
    );
    assert_issue_sources(&index, &archive, &reopen)
}

fn assert_issue_sources(index: &AudioIndex, archive: &Archive, reopen: &Archive) -> TestResult {
    let mut issue_sources = Vec::new();
    for issue in index.issues() {
        let source = issue.source(archive).ok_or("original issue Entry")?;
        assert!(
            issue.source(reopen).is_none(),
            "an independently opened copy cannot supply this issue"
        );
        issue_sources.push((source.entry_index(), issue.is_capacity_limit()));
    }
    assert_eq!(
        issue_sources,
        [(1, true), (2, false)],
        "only the failing physical Entries contribute issues"
    );
    Ok(())
}

fn assert_survey(
    index: &AudioIndex,
    archive: &Archive,
    kind: AudioSurveyKind,
    expected: &[(u32, AudioCandidateState, Option<u64>, bool)],
) -> TestResult {
    let survey = index.survey(archive, kind).ok_or("original root survey")?;
    assert_candidates(survey, expected);
    Ok(())
}

fn assert_candidates(
    survey: AudioSurvey<'_, '_>,
    expected: &[(u32, AudioCandidateState, Option<u64>, bool)],
) {
    assert!(
        !survey.complete(),
        "the failed candidate prevents an absence claim"
    );
    let observed: Vec<_> = survey
        .candidates()
        .map(|candidate| {
            (
                candidate.entry().entry_index(),
                candidate.state(),
                candidate.decoded_bytes(),
                candidate.has_retained_input(),
            )
        })
        .collect();
    assert_eq!(
        observed, expected,
        "physical candidates and exact read results"
    );
}

#[test]
fn an_atl_capacity_failure_retains_evidence_only_when_a_prefix_was_used() -> TestResult {
    let first = TestAtl::new("first")
        .trigger("first_admitted", &[])
        .trigger("first_omitted", &[])
        .to_xml();
    let second = TestAtl::new("second")
        .trigger("second_omitted", &[])
        .to_xml();
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/First.xml",
            first.as_bytes(),
        ))
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/Second.xml",
            second.as_bytes(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entries: Vec<_> = archive.entries().iter().enumerate().collect();
    let reads = index_read::read_definitions(&archive, &entries, &CancelToken::new())
        .ok_or("completed ATL reads")?;
    let mut definitions = AtlDefinitions::default();
    definitions.add_definitions(&archive, &entries, reads, 1);
    let survey = AudioSurvey {
        archive: &archive,
        candidates: definitions.candidates(),
        directory_complete: true,
    };
    assert_candidates(
        survey,
        &[
            (
                0,
                AudioCandidateState::CapacityLimited,
                Some(first.len() as u64),
                true,
            ),
            (
                1,
                AudioCandidateState::CapacityLimited,
                Some(second.len() as u64),
                false,
            ),
        ],
    );
    assert!(definitions.trigger("first_admitted").is_some());
    assert!(definitions.trigger("first_omitted").is_none());
    assert!(definitions.trigger("second_omitted").is_none());
    Ok(())
}

#[test]
fn empty_surveys_distinguish_a_completed_search_from_a_default_index() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Unrelated.xml", b"untouched"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let index = AudioIndex::build(&archive);
    for kind in [AudioSurveyKind::AtlDefinitions, AudioSurveyKind::Soundbanks] {
        let survey = index
            .survey(&archive, kind)
            .ok_or("completed root search")?;
        assert!(
            survey.complete(),
            "no matching Entries in a complete directory"
        );
        assert_eq!(
            survey.candidates().count(),
            0,
            "no candidate payload was read"
        );
        assert!(
            AudioIndex::default().survey(&archive, kind).is_none(),
            "a default index cannot claim it surveyed the Archive"
        );
    }
    Ok(())
}
