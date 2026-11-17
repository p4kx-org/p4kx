// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::MediaError;

use super::{Family, TestResult};

#[derive(Debug)]
pub(super) struct VerificationCheck(pub(super) &'static str);

impl Display for VerificationCheck {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for VerificationCheck {}

pub(super) fn checked<T, E: Into<Box<dyn Error>>>(
    family: Family,
    stage: &'static str,
    result: Result<T, E>,
) -> TestResult<T> {
    result.map_err(|cause| {
        let cause = cause.into();
        report(family, stage, cause.as_ref());
        cause
    })
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in census reports only technical failure categories"
)]
pub(super) fn report(family: Family, stage: &'static str, cause: &(dyn Error + 'static)) {
    eprintln!("{}", Failure::new(family, stage, cause));
}

struct Failure {
    family: Family,
    stage: &'static str,
    category: Category,
}

enum Category {
    Error(&'static str),
    Limit(Option<&'static str>),
    Check(&'static str),
}

impl Failure {
    fn new(family: Family, stage: &'static str, cause: &(dyn Error + 'static)) -> Self {
        let category = cause
            .downcast_ref::<MediaError>()
            .map(media_category)
            .or_else(|| {
                cause
                    .downcast_ref::<VerificationCheck>()
                    .map(|check| Category::Check(check.0))
            })
            .unwrap_or(Category::Error("verification"));
        Self {
            family,
            stage,
            category,
        }
    }
}

impl Display for Failure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "animation failure family={} stage={}",
            self.family.suffix(),
            self.stage
        )?;
        match self.category {
            Category::Error(class) => write!(formatter, " class={class}"),
            Category::Limit(Some(budget)) => write!(formatter, " class=limit budget={budget}"),
            Category::Limit(None) => formatter.write_str(" class=limit"),
            Category::Check(reason) => write!(formatter, " class=check reason={reason:?}"),
        }
    }
}

fn media_category(cause: &MediaError) -> Category {
    let class = match cause {
        MediaError::Source { cause, .. }
        | MediaError::ExtractionInterrupted { cause, .. }
        | MediaError::RetainedOutput { cause, .. }
        | MediaError::InputCheckFailed {
            operation: cause, ..
        }
        | MediaError::OutputCheckFailed {
            operation: cause, ..
        }
        | MediaError::CleanupFailed { primary: cause, .. } => return media_category(cause),
        MediaError::Output { cause, .. } => return media_category(cause.as_ref()),
        MediaError::Limit { budget, .. } => return Category::Limit(known_budget(budget)),
        MediaError::IncompleteInventory { .. } => "inventory",
        MediaError::ProjectionLoss { .. } => "projection",
        MediaError::Extraction { .. } => "extraction",
        MediaError::Archive(_) => "archive",
        MediaError::Metadata(_) => "metadata",
        MediaError::Unsupported { .. } => "unsupported",
        MediaError::ReferenceIndex(_) => "reference-index",
        MediaError::Cancelled => "cancelled",
        MediaError::Changed(_) => "archive-changed",
        MediaError::Protected(_) => "write-protection",
        MediaError::Cleanup(_) => "cleanup",
        MediaError::OwnedOutput(_) => "owned-output",
        MediaError::Io { .. } => "io",
        MediaError::Destination(_) => "destination",
        MediaError::Contract(_) => "contract",
        MediaError::Json(_) => "json",
    };
    Category::Error(class)
}

fn known_budget(budget: &'static str) -> Option<&'static str> {
    match budget {
        "root_bytes"
        | "sources"
        | "source_bytes"
        | "metadata_work"
        | "entry_bytes"
        | "records"
        | "record_bytes"
        | "chunk_bytes"
        | "output_bytes"
        | "depth"
        | "value_depth"
        | "references"
        | "reference_candidates"
        | "hash_bytes"
        | "artifacts"
        | "index_bytes" => Some(budget),
        _ => None,
    }
}

#[test]
fn reports_traversal_failure_without_source_details() {
    const PRIVATE: &str = "PRIVATE_SOURCE_PATH_AND_RECORD_SENTINEL";
    let cause = MediaError::InputCheckFailed {
        operation: Box::new(MediaError::Limit {
            budget: "records",
            limit: 12_345,
        }),
        checkpoint: Box::new(MediaError::Contract(PRIVATE.to_owned())),
    };
    assert_eq!(
        Failure::new(Family::Caf, "traversal", &cause).to_string(),
        "animation failure family=caf stage=traversal class=limit budget=records",
        "the traversal diagnostic retains only the known budget name"
    );

    let cause = MediaError::Limit {
        budget: PRIVATE,
        limit: 12_345,
    };
    assert_eq!(
        Failure::new(Family::Dba, "traversal", &cause).to_string(),
        "animation failure family=dba stage=traversal class=limit",
        "unrecognised budget names stay private"
    );

    let cause = MediaError::Contract(PRIVATE.to_owned());
    assert_eq!(
        Failure::new(Family::Aim, "reconcile-entry", &cause).to_string(),
        "animation failure family=aim stage=reconcile-entry class=contract",
        "contract messages stay private"
    );
}
