// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::write::Outcome;
use crate::{EntryFailure, ExtractionReport, TextureArtifacts};

/// The outcome of one Entry of an Extraction: its position, its path and how it ended.
pub(crate) type EntryEnded = (usize, String, Outcome);

/// How one item of an Extraction ended.
pub(crate) enum Ended {
    /// An Entry written on its own.
    Entry(EntryEnded),
    /// The Entries of a texture, and its failure by position when it was not fully converted.
    Texture {
        entries: Vec<EntryEnded>,
        unconverted: Option<(usize, EntryFailure)>,
        artifacts: Option<(usize, TextureArtifacts)>,
    },
    /// A sound or soundbank Entry, the failures of what could not be converted and the names of
    /// the sounds of a soundbank the cancelled run left unwritten.
    Sound {
        entry: EntryEnded,
        unconverted: Vec<EntryFailure>,
        cancelled: Vec<String>,
    },
}

pub(crate) fn summarise(ended: Vec<Ended>, convert: bool) -> ExtractionReport {
    let mut outcomes = Vec::with_capacity(ended.len());
    let mut unconverted = Vec::new();
    let mut unwritten = Vec::new();
    let mut texture_artifacts = Vec::new();
    for item in ended {
        match item {
            Ended::Entry(outcome) => outcomes.push(outcome),
            Ended::Texture {
                entries,
                unconverted: failure,
                artifacts,
            } => {
                outcomes.extend(entries);
                unconverted.extend(failure);
                texture_artifacts.extend(artifacts);
            }
            Ended::Sound {
                entry,
                unconverted: failures,
                cancelled: sounds,
            } => {
                let index = entry.0;
                outcomes.push(entry);
                unconverted.extend(failures.into_iter().map(|failure| (index, failure)));
                unwritten.extend(sounds.into_iter().map(|sound| (index, sound)));
            }
        }
    }
    outcomes.sort_unstable_by_key(|(index, _, _)| *index);
    unwritten.sort_by_key(|(index, _)| *index);
    unconverted.sort_by_key(|(index, _)| *index);
    texture_artifacts.sort_unstable_by_key(|(index, _)| *index);
    let mut report = ExtractionReport {
        texture_artifacts: texture_artifacts
            .into_iter()
            .map(|(_, artifacts)| artifacts)
            .collect(),
        converted: convert.then_some(0),
        unconverted: unconverted
            .into_iter()
            .map(|(_, failure)| failure)
            .collect(),
        ..ExtractionReport::default()
    };
    record_outcomes(&mut report, outcomes);
    report
        .cancelled
        .extend(unwritten.into_iter().map(|(_, sound)| sound));
    report
}

fn record_outcomes(report: &mut ExtractionReport, outcomes: Vec<EntryEnded>) {
    for (_, path, outcome) in outcomes {
        match outcome {
            Outcome::Written => report.extracted += 1,
            Outcome::Converted => {
                report.extracted += 1;
                report.converted = report.converted.map(|count| count + 1);
            }
            Outcome::Kept => report.skipped.push(path),
            Outcome::NotConverted => report.not_converted += 1,
            Outcome::Cancelled => report.cancelled.push(path),
            Outcome::Failed(error) if crate::guard_cancellation::extraction_cancelled(&error) => {
                report.cancelled.push(path);
            }
            Outcome::Failed(error) => report.failures.push(EntryFailure { path, error }),
        }
    }
}
