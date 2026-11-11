// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankSwitchGroup, BankSwitchGroupRows, BankSwitchMember, BankSwitchMembership,
};

use crate::audio_bank_node_declaration_payload::NodeReferenceStatus;
use crate::audio_bank_node_declarations::Declarations;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_switch_container_payload::{
    GROUP_NAMESPACE, MEMBER_NAMESPACE, SwitchContainerPayload, SwitchMemberRecord,
    SwitchMembershipGroupRecord,
};
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) fn project<R: Read + Seek>(
    declarations: &Declarations<'_, '_>,
    rows: BankSwitchGroupRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::SwitchContainer);
    for row in rows {
        let row = row.map_err(|cause| declarations.bank.replay_failure(cause))?;
        match row {
            BankSwitchMembership::Group(row) => group(declarations, &sites, &row, writer)?,
            BankSwitchMembership::Member(row) => member(declarations, &sites, &row, writer)?,
            _ => {
                return Err(MediaError::Contract(
                    "unexpected Switch membership row".into(),
                ));
            }
        }
    }
    Ok(())
}

fn group(
    declarations: &Declarations<'_, '_>,
    sites: &NodeSites,
    row: &BankSwitchGroup,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(declarations.bank.input, writer, Some(&row.value_id.range))?;
    sites.converted(
        &mut prepared,
        "layout",
        &row.checked_range,
        "switchMembershipGroupCoverage",
    )?;
    let record = group_record(sites, &mut prepared, row)?;
    let (status, issue) = if matches!(record, SwitchMembershipGroupRecord::Malformed { .. }) {
        sites.issues(&mut prepared, &row.checked_range)?;
        (
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.truncated-switch-membership-group",
                "HIRC",
                Severity::Error,
                "the original membership group ends before the declared member list fits",
            )),
        )
    } else {
        (Status::Complete, None)
    };
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            GROUP_NAMESPACE,
            vec![section, object, u64::from(row.ordinal)],
        ),
        status,
        &SwitchContainerPayload::Group(&record),
        issue.as_slice(),
    )
}

fn group_record(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    row: &BankSwitchGroup,
) -> Result<SwitchMembershipGroupRecord, MediaError> {
    sites.field(
        prepared,
        "value_id",
        &row.value_id.range,
        "switchMembershipValueId",
    )?;
    let value_id = (&row.value_id).into();
    let member_count = sites.scalar(
        prepared,
        "member_count",
        row.member_count.as_ref(),
        "switchMembershipCount",
    )?;
    let members_range = if let Some(range) = &row.members_range {
        sites.range(prepared, "members_range", range)?;
        Some(range.into())
    } else {
        None
    };
    match (row.complete, member_count, members_range) {
        (true, Some(member_count), Some(members_range)) => {
            Ok(SwitchMembershipGroupRecord::Decoded {
                value_id,
                member_count,
                members_range,
            })
        }
        (false, member_count, members_range) => Ok(SwitchMembershipGroupRecord::Malformed {
            value_id,
            member_count,
            members_range,
        }),
        _ => Err(MediaError::Contract(
            "complete Switch group lacks its member extent".into(),
        )),
    }
}

fn member(
    declarations: &Declarations<'_, '_>,
    sites: &NodeSites,
    row: &BankSwitchMember,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let range = &row.child_id.range;
    let mut prepared = sites.record(declarations.bank.input, writer, Some(range))?;
    sites.index(
        &mut prepared,
        2,
        &row.group_value_range,
        "switchMembershipGroupPosition",
    )?;
    sites.index(&mut prepared, 3, range, "switchMembershipPosition")?;
    sites.field(&mut prepared, "child_id", range, "switchMembershipChildId")?;
    sites.converted(
        &mut prepared,
        "reference_resolution",
        range,
        "switchReferenceUnchecked",
    )?;
    let record = SwitchMemberRecord {
        child_id: (&row.child_id).into(),
        reference_resolution: NodeReferenceStatus::Unchecked,
    };
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            MEMBER_NAMESPACE,
            vec![
                section,
                object,
                u64::from(row.group_ordinal),
                u64::from(row.ordinal),
            ],
        ),
        Status::Complete,
        &SwitchContainerPayload::Member(&record),
        &[],
    )
}
