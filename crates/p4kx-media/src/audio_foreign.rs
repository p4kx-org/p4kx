// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, TryReserveError};

use p4kx_extraction::bank::BankObservedHeader;
use p4kx_extraction::{BankHeader, DecimalU64};
use schemars::JsonSchema;
use serde::Serialize;

use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{Identity, MediaError, MediaWriter};

#[path = "audio_foreign_bank.rs"]
mod bank;
#[path = "audio_foreign_rows.rs"]
pub(crate) mod rows;

pub(crate) use rows::project;

#[derive(Default)]
pub(crate) struct Scope {
    observed: RefCell<Observations>,
}

#[derive(Default)]
struct Observations {
    headers: HashMap<usize, HeaderRead>,
    query_positions: HashMap<QueryKey, usize>,
    queries: Vec<Query>,
    interests: Interests,
}

#[derive(Clone, Copy)]
pub(crate) struct Header {
    value: BankHeader,
    section: u64,
    body: u64,
}

impl Header {
    pub(crate) const fn new(observed: &BankObservedHeader) -> Self {
        Self {
            value: observed.header(),
            section: observed.section().ordinal(),
            body: observed.section().body_offset(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum HeaderRead {
    Known(Header),
    Unavailable(Blocker),
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Blocker {
    Filtered,
    KeyRequired,
    UnprovedPath,
    Unopened,
    MalformedBank,
    IncompleteBankDirectory,
    MissingBankHeader,
    MalformedBankHeader,
    AmbiguousBankHeader,
    UnsupportedBankVersion,
    IncompleteHirc,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "ForeignTarget")]
pub(crate) enum Selector {
    Bank { bank_id: u32 },
    Event,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct QueryKey {
    owner_entry: usize,
    target_id: u32,
    selector: Selector,
}

struct Query {
    key: QueryKey,
    source: MediaOriginInput,
    evidence: OriginContributions,
}

#[derive(Default)]
struct Interests {
    banks: HashMap<u32, HashSet<u32>>,
    events: HashSet<u32>,
}

impl Scope {
    pub(crate) fn observe(
        &self,
        header: HeaderRead,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let owner = current_entry(writer)?;
        let mut observed = self.observed.try_borrow_mut().map_err(|_| borrowed())?;
        observed
            .headers
            .try_reserve(1)
            .map_err(|cause| allocation(&cause))?;
        if observed.headers.insert(owner, header).is_some() {
            return Err(MediaError::Contract(
                "bank header was observed more than once".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn request(
        &self,
        target_id: u32,
        selector: Selector,
        evidence: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<DecimalU64, MediaError> {
        writer.checkpoint()?;
        let key = QueryKey {
            owner_entry: current_entry(writer)?,
            target_id,
            selector,
        };
        let mut observed = self.observed.try_borrow_mut().map_err(|_| borrowed())?;
        if let Some(&index) = observed.query_positions.get(&key) {
            return Ok(DecimalU64::new(index as u64));
        }
        observed
            .query_positions
            .try_reserve(1)
            .map_err(|cause| allocation(&cause))?;
        observed
            .queries
            .try_reserve(1)
            .map_err(|cause| allocation(&cause))?;
        observed.interests.add(target_id, selector)?;
        let source = writer.source_origin.as_ref().ok_or_else(|| {
            MediaError::Contract("foreign soundbank query has no originating Entry".into())
        })?;
        let index = observed.queries.len();
        observed.query_positions.insert(key, index);
        observed.queries.push(Query {
            key,
            source: source.clone(),
            evidence,
        });
        Ok(DecimalU64::new(index as u64))
    }
}

impl Interests {
    fn add(&mut self, target: u32, selector: Selector) -> Result<(), MediaError> {
        let set = match selector {
            Selector::Bank { bank_id } => {
                self.banks
                    .try_reserve(1)
                    .map_err(|cause| allocation(&cause))?;
                self.banks.entry(bank_id).or_default()
            }
            Selector::Event => &mut self.events,
        };
        set.try_reserve(1).map_err(|cause| allocation(&cause))?;
        set.insert(target);
        Ok(())
    }
}

fn current_entry(writer: &MediaWriter<'_>) -> Result<usize, MediaError> {
    let Some(Identity::Entry { entry, .. }) = writer.source.as_ref().map(|source| &source.identity)
    else {
        return Err(MediaError::Contract(
            "soundbank observation or query outside its physical Entry".into(),
        ));
    };
    Ok(entry.entry_index)
}

fn borrowed() -> MediaError {
    MediaError::Contract("foreign bank directory is already in use".into())
}

fn allocation(cause: &TryReserveError) -> MediaError {
    MediaError::Contract(format!("cannot allocate foreign bank metadata: {cause}"))
}
