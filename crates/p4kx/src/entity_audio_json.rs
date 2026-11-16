// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::audio::ExportReport;
use serde::Serialize;

use crate::cli::EntityAudioArgs;
use crate::json;
use crate::wwise_extract::{self, ExportJson};
use crate::wwise_json::LanguageJson;

use super::origins;
use super::{Counts, EntityJson, IssueJson, TriggerJson, entity_json, issues_json, trigger_json};

#[derive(Serialize)]
struct AudioJson<'a> {
    archive: String,
    entry: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<LanguageJson<'a>>,
    entity: EntityJson<'a>,
    triggers: Vec<TriggerJson<'a>>,
    counts: Counts,
    #[serde(skip_serializing_if = "Option::is_none")]
    extraction: Option<ExportJson>,
    issues: Vec<IssueJson<'a>>,
    #[serde(flatten)]
    evidence: origins::Evidence,
}

pub(super) fn print(
    input: &origins::Input<'_, '_>,
    entry: &str,
    counts: Counts,
    args: &EntityAudioArgs,
    report: Option<&ExportReport>,
) -> Result<()> {
    let evidence = origins::capture(input)?;
    json::print(&AudioJson {
        archive: json::path(input.archive.path()),
        entry,
        language: input.language.map(LanguageJson::of),
        entity: entity_json(input.record),
        triggers: input
            .resolved
            .triggers()
            .iter()
            .enumerate()
            .map(|(position, trigger)| {
                trigger_json(trigger, input.sounds.get(position).and_then(Option::as_ref))
            })
            .collect(),
        counts,
        extraction: report
            .zip(args.extract.as_deref())
            .map(|(report, out)| wwise_extract::export_json(report, out, &args.export)),
        issues: issues_json(input.index),
        evidence,
    })?;
    Ok(())
}
