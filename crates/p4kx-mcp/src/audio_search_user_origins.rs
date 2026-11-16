// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceLocation, SourceUseRole};

use crate::audio_shapes::EntitiesShape;
use crate::forge_origins::{EvidenceError, ForgeOrigins, site};

use super::super::super::occurrence_origins as occurrence;
use super::super::super::surveys::SurveyMode;
use super::super::UsersState;
use super::union;

pub(super) fn census(
    output: &mut ForgeOrigins,
    state: Option<&UsersState<'_, '_>>,
    skipped: bool,
    mode: SurveyMode,
) -> Result<FieldOrigin, EvidenceError> {
    let local = output.uses.origin(&[], !skipped);
    output.put("entities_skipped", local.clone())?;
    let Some(state) = state else {
        output.put("unreadable_records", local.clone())?;
        return Ok(local);
    };
    let record_count = state.users.record_count() as u64;
    let index = state.users.index();
    let mut facts = Vec::new();
    facts.push(site(
        &output.scope,
        "dataforgeRecordSurvey",
        "structIndices",
        vec![0, record_count],
        SourceUseRole::Selector,
    )?);
    if !index.failed_records().is_empty() {
        site(
            &output.scope,
            "dataforgeRecordSurvey",
            "canonicalRecords",
            ranges(index.failed_records()),
            SourceUseRole::Attempted,
        )?;
    }
    let failed = output
        .uses
        .origin(&facts, output.complete && index.failed_records().is_empty());
    output.put("unreadable_records", failed)?;
    let complete = matches!(mode, SurveyMode::Complete);
    if complete && !index.inspected_records().is_empty() {
        facts.push(site(
            &output.scope,
            "dataforgeRecordSurvey",
            "canonicalRecords",
            ranges(index.inspected_records()),
            SourceUseRole::Selector,
        )?);
    }
    Ok(output.uses.origin(
        &facts,
        output.complete && (complete || index.inspected_records().is_empty()) && index.complete(),
    ))
}

pub(super) struct Trigger<'a, 'users, 'forge> {
    pub(super) path: &'a str,
    pub(super) user_state: Option<&'a UsersState<'users, 'forge>>,
    pub(super) shape: Option<&'a EntitiesShape>,
    pub(super) lookup: &'a FieldOrigin,
    pub(super) census: &'a FieldOrigin,
    pub(super) name: &'a str,
    pub(super) mode: SurveyMode,
}

pub(super) fn trigger(
    output: &mut ForgeOrigins,
    trigger: &Trigger<'_, '_, '_>,
) -> Result<FieldOrigin, EvidenceError> {
    let Some((state, shape)) = trigger.user_state.zip(trigger.shape) else {
        return Ok(trigger.census.clone());
    };
    let path = format!("{}/entities", trigger.path);
    let mut listed = Vec::new();
    let mut evidence = Vec::new();
    let observe = match trigger.mode {
        SurveyMode::Witness => shape.listed.len(),
        SurveyMode::Complete => shape.count,
    };
    for (pos, (record, use_)) in state.users.observed(trigger.name).take(observe).enumerate() {
        let occurrence = if let Some(observation) = use_ {
            occurrence::collect(
                output,
                std::slice::from_ref(observation),
                SourceUseRole::Selector,
            )?
        } else {
            output.uses.origin(&[], false)
        };
        let selector = union(output, [&occurrence, trigger.lookup], true)?;
        if pos < shape.listed.len() {
            listed.push(write_reference(
                output,
                &format!("{path}/listed/{pos}"),
                record.index(),
                &selector,
            )?);
        }
        evidence.push(selector);
    }
    let count = union(
        output,
        [trigger.census, trigger.lookup]
            .into_iter()
            .chain(&evidence),
        evidence.len() == shape.count,
    )?;
    let list = union(
        output,
        std::iter::once(trigger.lookup).chain(&listed),
        listed.len() == shape.listed.len(),
    )?;
    let combined = union(output, [&count, &list], true)?;
    output.put(&format!("{path}/count"), count)?;
    output.put(&format!("{path}/listed"), list)?;
    Ok(combined)
}

fn write_reference(
    output: &mut ForgeOrigins,
    path: &str,
    record: u32,
    selector: &FieldOrigin,
) -> Result<FieldOrigin, EvidenceError> {
    let mut fields = Vec::new();
    for (label, field, role) in [
        ("guid", "guid", SourceUseRole::Payload),
        ("record_path", "fileName", SourceUseRole::Conversion),
        ("class_name", "recordName", SourceUseRole::Conversion),
    ] {
        let site = output.scope.at(
            SourceLocation::DataForge {
                record_index: record,
                field: field.into(),
            },
            role,
        )?;
        let fact = output.uses.origin(&[site], output.complete);
        let origin = union(output, [selector, &fact], true)?;
        output.put(&format!("{path}/{label}"), origin.clone())?;
        fields.push(origin);
    }
    let row = union(output, &fields, true)?;
    output.put(path, row.clone())?;
    Ok(row)
}

fn ranges(indices: &[u32]) -> Vec<u64> {
    let mut ranges = Vec::new();
    for index in indices.iter().copied().map(u64::from) {
        if let Some(end) = ranges.last_mut().filter(|end| **end == index) {
            *end += 1;
        } else {
            ranges.extend([index, index + 1]);
        }
    }
    ranges
}
