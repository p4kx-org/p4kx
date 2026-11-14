// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::reference_error::{Code, ReferenceError, Result};
use super::reference_log_report::{Finding, Profile, Stream};
use super::reference_manifest::{Expected, Identity, MAX_LOGICAL_SOURCES, MAX_PATH_BYTES};

const MAX_FINDINGS: usize = 1000;
const MAX_LOG_LINE_BYTES: usize = 64 << 10;

#[derive(Clone, Copy)]
enum Line<'a> {
    Progress,
    Raw(&'a str),
    Started,
    Finished,
    Exports(usize),
    Error,
    Review,
}

#[derive(Default)]
pub(crate) struct Observed {
    pub(crate) errors: Vec<Finding>,
    pub(crate) review: Vec<Finding>,
    loaded: usize,
    done: usize,
    exports: Vec<usize>,
    raw_paths: Vec<String>,
}

fn known_error(line: &str) -> bool {
    [
        "Exception while extracting ",
        "There was a problem whilst attempting to report this error.",
        "This exception has been reported.",
        "Error converting ",
        "[ERR] create dir ",
        "Error writing ",
        "Error exporting ",
        "error: ",
        "Unhandled Exception:",
        "Unhandled exception.",
        "[mem] ABORT:",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
        || (line.starts_with("thread '") && line.contains(" panicked at "))
}

fn progress_bar(line: &str) -> bool {
    let Some((_, rest)) = line
        .strip_prefix('[')
        .and_then(|line| line.split_once("] "))
    else {
        return false;
    };
    let Some((counts, duration)) = rest.split_once(" (") else {
        return false;
    };
    let Some((current, total)) = counts.split_once('/') else {
        return false;
    };
    current.parse::<u64>().is_ok()
        && total.parse::<u64>().is_ok()
        && duration.ends_with(')')
        && duration.contains(", ETA ")
}

fn classify(profile: Profile, stream: Stream, line: &str) -> Line<'_> {
    if known_error(line) {
        return Line::Error;
    }
    match (profile, stream) {
        (Profile::Unp4kB492, Stream::Stdout) => {
            let parts: Vec<_> = line.splitn(3, " | ").collect();
            if parts.len() == 3
                && parts
                    .get(1)
                    .is_some_and(|part| matches!(*part, "Plain" | "Crypt"))
            {
                return parts.get(2).map_or(Line::Review, |path| Line::Raw(path));
            }
        }
        (Profile::UnforgeB492, Stream::Stdout)
            if line.starts_with("Converting ") || line.ends_with(" already in XML format") =>
        {
            return Line::Progress;
        }
        (
            Profile::Starbreaker450 | Profile::Starbreaker7d3 | Profile::Starbreaker305,
            Stream::Stderr,
        ) => {
            if line == "DataCore loaded." {
                return Line::Started;
            }
            if line == "Done." {
                return Line::Finished;
            }
            if let Some(count) = line
                .strip_prefix("Exporting ")
                .and_then(|line| line.strip_suffix(" records as json..."))
                .and_then(|number| number.parse().ok())
            {
                return Line::Exports(count);
            }
            if progress_bar(line) {
                return Line::Progress;
            }
        }
        _ => {}
    }
    Line::Review
}

pub(crate) fn scan(
    profile: Profile,
    stream: Stream,
    bytes: &[u8],
    observed: &mut Observed,
) -> Result<()> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        ReferenceError::new(
            Code::Incomplete,
            format!("review_required: log is not UTF-8: {error}"),
        )
    })?;
    for (index, line) in text.split(['\n', '\r']).enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_LOG_LINE_BYTES {
            return Err(ReferenceError::new(
                Code::Bounds,
                "producer log line exceeds its bound",
            ));
        }
        record(
            observed,
            stream,
            index as u64 + 1,
            line,
            classify(profile, stream, line),
        )?;
    }
    Ok(())
}

fn record(
    observed: &mut Observed,
    stream: Stream,
    line_number: u64,
    text: &str,
    line: Line<'_>,
) -> Result<()> {
    match line {
        Line::Error | Line::Review => {
            let finding = Finding {
                stream,
                line: line_number,
                message: text.chars().take(1024).collect(),
            };
            push_finding(observed, finding, matches!(line, Line::Error))?;
        }
        Line::Started => observed.loaded += 1,
        Line::Finished => observed.done += 1,
        Line::Exports(count) => {
            if observed.exports.len() >= MAX_LOGICAL_SOURCES {
                return Err(ReferenceError::new(
                    Code::Bounds,
                    "too many producer export markers",
                ));
            }
            observed.exports.push(count);
        }
        Line::Raw(path) => {
            if observed.raw_paths.len() >= MAX_LOGICAL_SOURCES || path.len() > MAX_PATH_BYTES {
                return Err(ReferenceError::new(
                    Code::Bounds,
                    "producer Entry log exceeds its bounds",
                ));
            }
            observed.raw_paths.push(path.to_owned());
        }
        Line::Progress => {}
    }
    Ok(())
}

fn push_finding(observed: &mut Observed, finding: Finding, error: bool) -> Result<()> {
    if observed.errors.len() + observed.review.len() >= MAX_FINDINGS {
        return Err(ReferenceError::new(
            Code::Bounds,
            "too many producer log findings",
        ));
    }
    if error {
        observed.errors.push(finding);
    } else {
        observed.review.push(finding);
    }
    Ok(())
}

pub(crate) fn finish(
    profile: Profile,
    selected: &[Expected],
    files: usize,
    observed: &mut Observed,
) -> Result<()> {
    let complete = match profile {
        Profile::Unp4kB492 => raw_selection(selected, observed),
        Profile::UnforgeB492 => true,
        Profile::Starbreaker450 | Profile::Starbreaker7d3 | Profile::Starbreaker305 => {
            observed.loaded == observed.exports.len()
                && observed.done == observed.exports.len()
                && !observed.exports.is_empty()
                && observed
                    .exports
                    .iter()
                    .try_fold(0_usize, |total, count| total.checked_add(*count))
                    == Some(files)
        }
    };
    if !complete {
        push_finding(observed, Finding { stream: Stream::Stderr, line: 0,
            message: "review_required: producer progress/completion markers do not match declared payloads".to_owned() }, false)?;
    }
    Ok(())
}

fn raw_selection(selected: &[Expected], observed: &Observed) -> bool {
    use std::collections::BTreeSet;
    let expected = selected
        .iter()
        .map(|source| match &source.identity {
            Identity::Entry { path } => super::reference_paths::key(path),
            Identity::Record { .. } => Err(ReferenceError::new(
                Code::Identity,
                "not an Entry selection",
            )),
        })
        .collect::<Result<BTreeSet<_>>>();
    let actual = observed
        .raw_paths
        .iter()
        .map(|path| super::reference_paths::key(path))
        .collect::<Result<BTreeSet<_>>>();
    expected
        .ok()
        .zip(actual.ok())
        .is_some_and(|(expected, actual)| {
            expected == actual && observed.raw_paths.len() == selected.len()
        })
}
