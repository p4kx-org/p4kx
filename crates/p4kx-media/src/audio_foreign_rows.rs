// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::BankHircObject;
use schemars::JsonSchema;
use serde::Serialize;

use crate::bank_candidate_index::Resolution;
use crate::origin_input::MediaOriginInput;
use crate::{CandidateInventory, DirectoryContext, MediaError, MediaWriter, Status, Subresource};

use super::bank::{self, Bank, HircRead};
use super::{Blocker, Header, HeaderRead, Query, QueryKey, Selector, borrowed};

#[path = "audio_foreign_row_origins.rs"]
mod origins;

pub(crate) const SEARCH: &str = "audio-bank-foreign-target-resolution";
pub(crate) const CANDIDATE: &str = "audio-bank-foreign-target-candidate";
pub(crate) const BLOCKED: &str = "audio-bank-foreign-target-blocker";

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum ForeignPayload {
    #[serde(rename = "bank-foreign-target-resolution")]
    Search(ForeignSearchRecord),
    #[serde(rename = "bank-foreign-target-candidate")]
    Candidate(ForeignCandidateRecord),
    #[serde(rename = "bank-foreign-target-blocker")]
    Blocker(ForeignBlockerRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ForeignSearchRecord {
    owner_entry_index: DecimalU64,
    selector: Selector,
    target_id: u32,
    directory_complete: bool,
    candidates_complete: bool,
    observed_candidates: DecimalU64,
    matching_candidates: DecimalU64,
    blocked_banks: DecimalU64,
    resolution: Resolution,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ForeignCandidateRecord {
    target_entry_index: DecimalU64,
    target_bkhd_section_ordinal: DecimalU64,
    target_hirc_section_ordinal: DecimalU64,
    target_object_ordinal: DecimalU64,
    target_type_code: u8,
    matches_target_kind: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ForeignBlockerRecord {
    target_entry_index: DecimalU64,
    reason: Blocker,
    filtered: bool,
    key_required: bool,
}

pub(crate) fn project(
    context: &DirectoryContext<'_, '_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let observed = context
        .lookup
        .index
        .foreign_banks
        .observed
        .try_borrow()
        .map_err(|_| borrowed())?;
    if observed.queries.is_empty() {
        return Ok(());
    }
    let owner = writer.origins.directory().cloned().ok_or_else(|| {
        MediaError::Contract("foreign soundbank search has no owning Archive origin".into())
    })?;
    let directory_complete = matches!(
        context.lookup.scan_inventory(writer)?,
        CandidateInventory::Complete
    );
    let banks = bank::collect(context, &observed, writer)?;
    for (index, query) in observed.queries.iter().enumerate() {
        project_search(
            context,
            &owner,
            directory_complete,
            index as u64,
            query,
            &banks,
            writer,
        )?;
    }
    Ok(())
}

struct Seen {
    candidates: u64,
    matching: u64,
    blocked: u64,
}

fn project_search(
    context: &DirectoryContext<'_, '_, '_>,
    owner: &MediaOriginInput,
    directory_complete: bool,
    index: u64,
    query: &Query,
    banks: &[Bank<'_>],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut seen = Seen {
        candidates: 0,
        matching: 0,
        blocked: 0,
    };
    let mut evidence = writer.capture_origins();
    evidence.include(&query.evidence, writer)?;
    evidence.include(
        &crate::origin_lookup::directory(Some(owner), writer)?,
        writer,
    )?;
    for bank in banks {
        bank::work(context, writer)?;
        if bank.entry.index() == query.key.owner_entry {
            continue;
        }
        let relevant = bank_relevant(bank, query.key);
        origins::bank(&mut evidence, bank, relevant, writer)?;
        if relevant {
            project_bank(owner, index, query, bank, &mut seen, writer)?;
        }
    }
    let complete = directory_complete && seen.blocked == 0;
    let prepared = origins::search(owner, query, evidence.finish(), complete, writer)?;
    prepared.write(
        writer,
        &Subresource::new(SEARCH, vec![index]),
        Status::Complete,
        &ForeignPayload::Search(ForeignSearchRecord {
            owner_entry_index: DecimalU64::new(query.key.owner_entry as u64),
            selector: query.key.selector,
            target_id: query.key.target_id,
            directory_complete,
            candidates_complete: complete,
            observed_candidates: DecimalU64::new(seen.candidates),
            matching_candidates: DecimalU64::new(seen.matching),
            blocked_banks: DecimalU64::new(seen.blocked),
            resolution: Resolution::from_inventory(complete, seen.matching),
        }),
        &[],
    )
}

const fn bank_relevant(bank: &Bank<'_>, query: QueryKey) -> bool {
    !matches!(
        (bank.header, query.selector),
        (HeaderRead::Known(header), Selector::Bank { bank_id }) if header.value.id != bank_id
    )
}

fn project_bank(
    owner: &MediaOriginInput,
    index: u64,
    query: &Query,
    bank: &Bank<'_>,
    seen: &mut Seen,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let blocked = match &bank.hirc {
        HircRead::Unavailable(reason) => Some(*reason),
        HircRead::Observed(objects) => {
            let HeaderRead::Known(header) = bank.header else {
                return Err(MediaError::Contract(
                    "foreign objects lack a proved BKHD".into(),
                ));
            };
            for object in objects.candidates(query.key.target_id) {
                write_candidate(owner, index, query, (bank, header), object, seen, writer)?;
            }
            (!objects.is_complete()).then_some(Blocker::IncompleteHirc)
        }
        HircRead::NotRequested => {
            return Err(MediaError::Contract(
                "foreign HIRC interest was not admitted".into(),
            ));
        }
    };
    if let Some(reason) = blocked {
        let prepared = origins::blocker(owner, query, bank, writer)?;
        prepared.write(
            writer,
            &Subresource::new(BLOCKED, vec![index, seen.blocked]),
            Status::Complete,
            &ForeignPayload::Blocker(ForeignBlockerRecord {
                target_entry_index: DecimalU64::new(bank.entry.index() as u64),
                reason,
                filtered: bank.filtered,
                key_required: bank.key_required,
            }),
            &[],
        )?;
        seen.blocked += 1;
    }
    Ok(())
}

fn write_candidate(
    owner: &MediaOriginInput,
    index: u64,
    query: &Query,
    target: (&Bank<'_>, Header),
    object: &BankHircObject,
    seen: &mut Seen,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let (bank, header) = target;
    let matched = bank.matches(object, query.key.selector);
    let prepared = origins::candidate(owner, query, bank, header, object, writer)?;
    prepared.write(
        writer,
        &Subresource::new(CANDIDATE, vec![index, seen.candidates]),
        Status::Complete,
        &ForeignPayload::Candidate(ForeignCandidateRecord {
            target_entry_index: DecimalU64::new(bank.entry.index() as u64),
            target_bkhd_section_ordinal: DecimalU64::new(header.section),
            target_hirc_section_ordinal: DecimalU64::new(object.section_ordinal()),
            target_object_ordinal: DecimalU64::new(object.ordinal()),
            target_type_code: object.type_code(),
            matches_target_kind: matched,
        }),
        &[],
    )?;
    seen.candidates += 1;
    seen.matching += u64::from(matched);
    Ok(())
}
