// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::FieldOrigin;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::scope::{Candidate, Survey};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct Lookup {
    pub(super) status: Status,
    contains_encrypted_source: Option<bool>,
    source_set_complete: bool,
    pub(super) matches: Vec<Candidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) enum Status {
    Resolved,
    Missing,
    Ambiguous,
    Incomplete,
    EmptyToken,
    UnsupportedSpelling,
    NoFragmentDefinition,
    NoFragmentTagDefinition,
}

pub(super) fn lookup(
    spelling: &str,
    own: &FieldOrigin,
    survey: Option<&Survey>,
    context: Option<(&FieldOrigin, Status)>,
) -> Lookup {
    if spelling.is_empty() {
        return Lookup::new(Status::EmptyToken, [], [Some(own), None, None]);
    }
    if !spelling.is_ascii() {
        return Lookup::new(Status::UnsupportedSpelling, [], [Some(own), None, None]);
    }
    let Some(survey) = survey else {
        let (origin, status) = context.map_or((None, Status::Incomplete), |(origin, status)| {
            (Some(origin), status)
        });
        return Lookup::new(status, [], [Some(own), origin, None]);
    };
    let matches = survey.candidates(spelling);
    let status = match matches {
        [_, _, ..] => Status::Ambiguous,
        _ if !survey.output.origin.source_set_complete() => Status::Incomplete,
        [_] => Status::Resolved,
        [] => Status::Missing,
    };
    Lookup::new(
        status,
        matches.iter().cloned(),
        [Some(own), Some(&survey.output.origin), context.map(|c| c.0)],
    )
}

impl Lookup {
    fn new(
        status: Status,
        matches: impl IntoIterator<Item = Candidate>,
        origins: [Option<&FieldOrigin>; 3],
    ) -> Self {
        let mut complete = true;
        let mut encrypted = Some(false);
        for origin in origins.into_iter().flatten() {
            complete &= origin.source_set_complete();
            encrypted = match (encrypted, origin.contains_encrypted_source()) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None,
            };
        }
        Self {
            status,
            contains_encrypted_source: encrypted,
            source_set_complete: complete,
            matches: matches.into_iter().collect(),
        }
    }
}
