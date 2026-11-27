// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_audio::{
    BankMusicTransitionId, BankMusicTransitionIds, BankMusicTransitionRule, BankMusicTransitionSide,
};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::source_evidence_error::SourceEvidenceError;

#[path = "hirc_music_transition_source_fields.rs"]
mod fields;

use super::super::super::origins::ObjectOrigins;
use super::Owner;
use super::dto::{IdTable, ReferenceWord, Rule, Side, TransitionRow};

pub(in crate::hirc_view) struct Rows<'view, 'input, R> {
    owner: &'view RefCell<Owner<'input, R>>,
    origins: &'view ObjectOrigins,
}

impl<'view, 'input, R> Rows<'view, 'input, R> {
    pub(super) const fn new(
        owner: &'view RefCell<Owner<'input, R>>,
        origins: &'view ObjectOrigins,
    ) -> Self {
        Self { owner, origins }
    }
}

impl<R: Read + Seek> Serialize for Rows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut owner = self.owner.try_borrow_mut().map_err(S::Error::custom)?;
        let mut sequence = serializer.serialize_seq(None)?;
        if let Some(mut rows) = owner.transition_rows() {
            while let Some(mut row) = rows.next_rule().map_err(S::Error::custom)? {
                let projected = rule(self.origins, &row).map_err(S::Error::custom)?;
                sequence.serialize_element(&TransitionRow::Rule(&projected))?;
                serialize_ids(&mut sequence, self.origins, &mut row)?;
            }
        }
        sequence.end()
    }
}

fn table(
    origins: &ObjectOrigins,
    list: &BankMusicTransitionIds,
    [count, range, complete]: [&'static str; 3],
) -> Result<IdTable, SourceEvidenceError> {
    let extent = list.declared_count.range.start..list.checked_range.end;
    Ok(IdTable {
        declared_count: origins.field(&list.declared_count, count)?,
        checked_range: origins.range(&list.checked_range, range)?,
        complete: origins.derived(list.is_complete(), &extent, complete)?,
    })
}

fn rule<R>(
    origins: &ObjectOrigins,
    row: &BankMusicTransitionRule<'_, R>,
) -> Result<Rule, SourceEvidenceError> {
    let original = &row.fields;
    let start = &original.source_ids.declared_count.range;
    Ok(Rule {
        ordinal: origins.derived(row.ordinal, start, "musicTransitionRulePosition")?,
        checked_range: origins.range(&row.checked_range, "musicTransitionRuleExtent")?,
        complete: origins.derived(
            row.issue.is_none(),
            &row.checked_range,
            "musicTransitionRuleCoverage",
        )?,
        source_ids: table(
            origins,
            &original.source_ids,
            [
                "musicTransitionSourceIdCount",
                "musicTransitionSourceIdsExtent",
                "musicTransitionSourceIdsCoverage",
            ],
        )?,
        destination_ids: original
            .destination_ids
            .as_ref()
            .map(|list| {
                table(
                    origins,
                    list,
                    [
                        "musicTransitionDestinationIdCount",
                        "musicTransitionDestinationIdsExtent",
                        "musicTransitionDestinationIdsCoverage",
                    ],
                )
            })
            .transpose()?,
        source: fields::source(origins, &original.source)?,
        destination: fields::destination(origins, &original.destination)?,
        allocation_flag: origins.optional(
            original.allocation_flag.as_ref(),
            "musicTransitionAllocationFlag",
        )?,
        transition_object: original
            .transition_object
            .as_ref()
            .map(|object| fields::transition_object(origins, object))
            .transpose()?,
    })
}

impl Side {
    const fn original(self) -> BankMusicTransitionSide {
        match self {
            Self::Source => BankMusicTransitionSide::Source,
            Self::Destination => BankMusicTransitionSide::Destination,
        }
    }

    fn count_range<'row, R>(
        self,
        row: &'row BankMusicTransitionRule<'_, R>,
    ) -> Option<&'row Range<u64>> {
        match self {
            Self::Source => Some(&row.fields.source_ids.declared_count.range),
            Self::Destination => row
                .fields
                .destination_ids
                .as_ref()
                .map(|list| &list.declared_count.range),
        }
    }
}

fn serialize_ids<R: Read + Seek, S: SerializeSeq>(
    sequence: &mut S,
    origins: &ObjectOrigins,
    row: &mut BankMusicTransitionRule<'_, R>,
) -> Result<(), S::Error> {
    let rule_position = row.fields.source_ids.declared_count.range.clone();
    let rule_ordinal = row.ordinal;
    for side in [Side::Source, Side::Destination] {
        let Some(count_range) = side.count_range(row).cloned() else {
            continue;
        };
        let ids = row.ids(side.original()).ok_or_else(|| {
            S::Error::custom("observed transition ReferenceWord list has no replay cursor")
        })?;
        let site = IdSite {
            rule_ordinal,
            rule_position: &rule_position,
            side,
            count_range,
        };
        for original in ids {
            let original = original.map_err(S::Error::custom)?;
            let projected = site.project(origins, &original).map_err(S::Error::custom)?;
            sequence.serialize_element(&TransitionRow::Id(&projected))?;
        }
    }
    Ok(())
}

struct IdSite<'row> {
    rule_ordinal: u32,
    rule_position: &'row Range<u64>,
    side: Side,
    count_range: Range<u64>,
}

impl IdSite<'_> {
    fn project(
        &self,
        origins: &ObjectOrigins,
        original: &BankMusicTransitionId,
    ) -> Result<ReferenceWord, SourceEvidenceError> {
        let range = &original.id.range;
        Ok(ReferenceWord {
            rule_ordinal: origins.derived(
                self.rule_ordinal,
                self.rule_position,
                "musicTransitionRulePosition",
            )?,
            side: origins.derived(self.side, &self.count_range, "musicTransitionIdList")?,
            ordinal: origins.derived(original.ordinal, range, "musicTransitionIdPosition")?,
            id: origins.field(&original.id, "musicTransitionReferenceId")?,
        })
    }
}
