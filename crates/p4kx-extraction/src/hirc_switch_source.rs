// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{
    BankContainerFields, BankHircContainerNode, BankHircField, BankHircObject, BankObservedHeader,
    BankSwitchContainerFields, BankSwitchExtent,
};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::source_evidence_error::SourceEvidenceError;
use crate::source_value::SourcedValue;

#[path = "hirc_switch_source_dto.rs"]
mod dto;
#[path = "hirc_switch_source_rows.rs"]
mod rows;

pub(super) use self::dto::SwitchSourceSchema;
use self::dto::{
    SwitchFields, SwitchGroupKind, SwitchSourceDocument, SwitchSourceKind, SwitchSourceRow,
    SwitchSourceTable,
};
use self::rows::SwitchRows;
use super::container_layout::{self, CheckedExtent, ContainerSourceLayout};
use super::origins::ObjectOrigins;

pub(super) fn serialize<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    container: BankHircContainerNode<'_, R>,
) -> Result<S::Ok, S::Error> {
    let extent = CheckedExtent::observe(
        container.fields().switch.as_ref(),
        BankSwitchContainerFields::complete_range,
    );
    let (layout, error) = container_layout::coverage(container.status(), extent);
    let original = container.object();
    SwitchSourceDocument {
        kind: SwitchSourceKind::SwitchContainer,
        layout: origins
            .coverage(layout, header, original)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        error: error
            .map(|error| origins.coverage(error.to_string(), header, original))
            .transpose()
            .map_err(S::Error::custom)?,
        fields: observations(container.fields(), origins).map_err(S::Error::custom)?,
        rows: SwitchRows::new(container, origins),
        source_inventory: origins,
    }
    .serialize(serializer)
}

pub(super) fn unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    original: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    SwitchSourceDocument {
        kind: SwitchSourceKind::SwitchContainer,
        layout: origins
            .coverage(ContainerSourceLayout::Unsupported, header, original)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        error: None,
        fields: SwitchFields::default(),
        rows: [] as [SwitchSourceRow; 0],
        source_inventory: origins,
    }
    .serialize(serializer)
}

fn observations(
    container: &BankContainerFields,
    origins: &ObjectOrigins,
) -> Result<SwitchFields, SourceEvidenceError> {
    let Some(fields) = &container.switch else {
        return Ok(SwitchFields::default());
    };
    let child_range = container
        .children
        .as_ref()
        .map(|range| origins.range(range, "containerChildrenExtent"))
        .transpose()?;
    Ok(SwitchFields {
        group_type: origins.optional(fields.group_type.as_ref(), "switchGroupType")?,
        group_kind: group_kind(fields.group_type.as_ref(), origins)?,
        group_id: origins.optional(fields.group_id.as_ref(), "switchGroupId")?,
        default_value_id: origins
            .optional(fields.default_value_id.as_ref(), "switchDefaultValueId")?,
        continuous_validation: origins.optional(
            fields.continuous_validation.as_ref(),
            "switchContinuousValidation",
        )?,
        child_count: origins.optional(container.child_count.as_ref(), "containerChildCount")?,
        child_range,
        groups: table(
            fields.groups.as_ref(),
            origins,
            "switchMembershipGroupCount",
            "switchMembershipGroupsExtent",
        )?,
        configurations: table(
            fields.configurations.as_ref(),
            origins,
            "switchConfigurationCount",
            "switchConfigurationsExtent",
        )?,
    })
}

fn group_kind(
    field: Option<&BankHircField<u8>>,
    origins: &ObjectOrigins,
) -> Result<Option<SourcedValue<SwitchGroupKind>>, SourceEvidenceError> {
    let Some(field) = field else {
        return Ok(None);
    };
    let kind = match field.value {
        0 => SwitchGroupKind::Switch,
        1 => SwitchGroupKind::State,
        _ => return Ok(None),
    };
    origins
        .derived(kind, &field.range, "switchGroupType")
        .map(Some)
}

fn table(
    extent: Option<&BankSwitchExtent>,
    origins: &ObjectOrigins,
    count_label: &'static str,
    range_label: &'static str,
) -> Result<Option<SwitchSourceTable>, SourceEvidenceError> {
    let Some(extent) = extent else {
        return Ok(None);
    };
    let count = &extent.declared_count;
    let inspected = count.range.start..extent.checked_range.end;
    Ok(Some(SwitchSourceTable {
        declared_count: origins.field(count, count_label)?,
        checked_range: origins.range(&extent.checked_range, range_label)?,
        complete: origins.derived(extent.is_complete(), &inspected, "switchTableCoverage")?,
    }))
}
