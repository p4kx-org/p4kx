// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankHircContainerNode, BankHircField, BankHircObject, BankSwitchContainerFields,
    BankSwitchExtent,
};

use crate::audio_bank_container_node;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declarations::{Coverage, Declarations, Disposition};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_switch_configuration;
use crate::audio_bank_switch_container_payload::{
    SUMMARY_NAMESPACE, SwitchContainerObservations, SwitchContainerPayload, SwitchGroupKind,
    SwitchTable, SwitchTableCoverage,
};
use crate::audio_bank_switch_membership;
use crate::{MediaError, MediaWriter, Subresource};

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    container: &mut BankHircContainerNode<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let declarations = Declarations::new(projection, container.object(), version);
    let disposition = audio_bank_container_node::disposition(container.status())?;
    summary(
        &declarations,
        disposition,
        container.fields().switch.as_ref(),
        writer,
    )?;
    if let Some(rows) = container.switch_groups() {
        audio_bank_switch_membership::project(&declarations, rows, writer)?;
    }
    if let Some(rows) = container.switch_configurations() {
        audio_bank_switch_configuration::project(&declarations, rows, writer)?;
    }
    Ok(())
}

pub(crate) fn unsupported(
    projection: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let declarations = Declarations::new(projection, original, version);
    summary(&declarations, Disposition::Unsupported, None, writer)
}

fn summary(
    declarations: &Declarations<'_, '_>,
    disposition: Disposition<'_>,
    original: Option<&BankSwitchContainerFields>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::SwitchContainer);
    let mut prepared = sites.record(declarations.bank.input, writer, None)?;
    let observed = if let Some(fields) = original {
        observations(&sites, &mut prepared, fields)?
    } else {
        SwitchContainerObservations::default()
    };
    let coverage = Coverage::read(
        original.map(|fields| fields.start),
        original.and_then(BankSwitchContainerFields::complete_range),
    );
    let (payload, status, issue) = declarations.summary(
        &sites,
        &mut prepared,
        disposition,
        &coverage,
        observed,
        "audio.bank.invalid-switch-container",
    )?;
    prepared.finish().write(
        writer,
        &Subresource::new(SUMMARY_NAMESPACE, sites.indices().to_vec()),
        status,
        &SwitchContainerPayload::Summary(&payload),
        issue.as_slice(),
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankSwitchContainerFields,
) -> Result<SwitchContainerObservations, MediaError> {
    let groups = table(
        sites,
        prepared,
        fields.groups.as_ref(),
        "groups",
        "switchMembershipGroupCount",
    )?;
    let configurations = table(
        sites,
        prepared,
        fields.configurations.as_ref(),
        "configurations",
        "switchConfigurationCount",
    )?;
    let group_kind = group_kind(sites, prepared, fields.group_type.as_ref())?;
    Ok(SwitchContainerObservations {
        group_kind,
        group_type: sites.scalar(
            prepared,
            "observed/group_type",
            fields.group_type.as_ref(),
            "switchGroupType",
        )?,
        group_id: sites.scalar(
            prepared,
            "observed/group_id",
            fields.group_id.as_ref(),
            "switchGroupId",
        )?,
        default_value_id: sites.scalar(
            prepared,
            "observed/default_value_id",
            fields.default_value_id.as_ref(),
            "switchDefaultValueId",
        )?,
        continuous_validation: sites.scalar(
            prepared,
            "observed/continuous_validation",
            fields.continuous_validation.as_ref(),
            "switchContinuousValidation",
        )?,
        groups,
        configurations,
    })
}

fn group_kind(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    group_type: Option<&BankHircField<u8>>,
) -> Result<Option<SwitchGroupKind>, MediaError> {
    let Some(original) = group_type else {
        return Ok(None);
    };
    let kind = match original.value {
        0 => SwitchGroupKind::Switch,
        1 => SwitchGroupKind::State,
        _ => return Ok(None),
    };
    sites.converted(
        prepared,
        "observed/group_kind",
        &original.range,
        "switchGroupKind",
    )?;
    Ok(Some(kind))
}

fn table(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    extent: Option<&BankSwitchExtent>,
    name: &str,
    count_label: &'static str,
) -> Result<Option<SwitchTable>, MediaError> {
    let Some(extent) = extent else {
        return Ok(None);
    };
    let path = format!("observed/{name}");
    let count = &extent.declared_count;
    let range = &extent.checked_range;
    sites.field(
        prepared,
        &format!("{path}/declared_count"),
        &count.range,
        count_label,
    )?;
    sites.range(prepared, &format!("{path}/checked_range"), range)?;
    let coverage = if extent.is_complete() {
        SwitchTableCoverage::Complete
    } else {
        SwitchTableCoverage::Truncated
    };
    sites.converted(
        prepared,
        &format!("{path}/coverage"),
        &(count.range.start..range.end),
        "switchTableCoverage",
    )?;
    Ok(Some(SwitchTable {
        declared_count: count.into(),
        checked_range: range.into(),
        coverage,
    }))
}
