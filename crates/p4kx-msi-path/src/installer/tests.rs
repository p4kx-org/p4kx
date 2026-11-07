// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::caller::Caller;
use crate::installer::fault::Fault;
use crate::installer::lease::Lease;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::state::{Observed, Phase, Request, Snapshot, Store};
use crate::installer::transaction;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const PRODUCT: u128 = 1;
const NEXT_PRODUCT: u128 = 2;
const ENTRY: &str = r"C:\P4KX\private-lease-123";
const UPGRADED_ENTRY: &str = r"C:\P4KX Renamed\private-lease-456";

struct Memory {
    state: Snapshot,
    interleave: Option<RawPath>,
}

impl Store for Memory {
    type Revision = u64;
    type Error = Fault;
    fn load(&self) -> Result<Observed<u64>, Fault> {
        Ok(Observed {
            state: self.state.clone(),
            revision: self.state.sequence,
        })
    }
    fn compare_replace(
        &mut self,
        expected: &Observed<u64>,
        replacement: Snapshot,
    ) -> Result<(), Fault> {
        if let Some(foreign) = self.interleave.take() {
            self.state.path = foreign;
        }
        if self.state != expected.state || self.state.sequence != expected.revision {
            return Err(Fault::Conflict);
        }
        self.state = replacement;
        Ok(())
    }
}

fn bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn path(text: &str, kind: StringKind) -> RawPath {
    let mut raw = bytes(text);
    raw.extend_from_slice(&[0, 0]);
    RawPath::Present { kind, bytes: raw }
}

fn lease() -> Result<Lease, Fault> {
    Lease::try_new(bytes(ENTRY))
}

fn memory(original: RawPath) -> Memory {
    Memory {
        state: Snapshot {
            path: original,
            owner: None,
            journal: None,
            sequence: 0,
        },
        interleave: None,
    }
}

fn begin(store: &mut Memory, request: Request) -> Result<u64, Fault> {
    let sequence = store.state.sequence;
    begin_at(store, sequence, request)
}

fn begin_at(store: &mut Memory, expected: u64, request: Request) -> Result<u64, Fault> {
    let caller = context::for_request(store, &request)?;
    transaction::begin_at(store, expected, &caller, request)
}

fn bound_caller(store: &Memory) -> Result<Caller, Fault> {
    Ok(store
        .state
        .journal
        .as_ref()
        .ok_or(Fault::WrongTransaction)?
        .caller
        .clone())
}

fn apply(store: &mut Memory, id: u64) -> Result<(), Fault> {
    let caller = bound_caller(store)?;
    transaction::apply(store, id, &caller).map(|_| ())
}

fn commit(store: &mut Memory, id: u64) -> Result<(), Fault> {
    let caller = bound_caller(store)?;
    transaction::commit(store, id, &caller)
}

fn rollback(store: &mut Memory, id: u64) -> Result<(), Fault> {
    let caller = bound_caller(store)?;
    transaction::rollback(store, id, &caller).map(|_| ())
}

fn rollback_at(store: &mut Memory, expected: u64) -> Result<(), Fault> {
    let caller = bound_caller(store)?;
    transaction::rollback_at(store, expected, &caller).map(|_| ())
}

fn install(store: &mut Memory) -> Result<u64, Fault> {
    let id = begin(
        store,
        Request::Install {
            product: PRODUCT,
            lease: lease()?,
        },
    )?;
    apply(store, id)?;
    commit(store, id)?;
    Ok(id)
}

pub(crate) fn installed(original: RawPath) -> Result<Snapshot, Fault> {
    installation_at(original, Phase::Committed)
}

pub(crate) fn installation_at(original: RawPath, phase: Phase) -> Result<Snapshot, Fault> {
    let mut store = memory(original);
    let id = begin(
        &mut store,
        Request::Install {
            product: PRODUCT,
            lease: lease()?,
        },
    )?;
    if phase != Phase::Prepared {
        apply(&mut store, id)?;
    }
    match phase {
        Phase::Committed => commit(&mut store, id)?,
        Phase::RolledBack => rollback(&mut store, id)?,
        Phase::Prepared | Phase::Applied => {}
    }
    Ok(store.state)
}

fn remove(store: &mut Memory, product: u128) -> Result<u64, Fault> {
    let id = begin(store, Request::Remove { product })?;
    apply(store, id)?;
    commit(store, id)?;
    Ok(id)
}

#[path = "fixture_caller.rs"]
mod context;
#[path = "driver_tests.rs"]
mod driver;
#[path = "journal_claim_tests.rs"]
mod journal_claim;
#[path = "lifecycle_tests.rs"]
mod lifecycle;
#[path = "path_tests.rs"]
mod path;
#[path = "path_bytes_tests.rs"]
mod path_bytes;
#[path = "recovery_tests.rs"]
mod recovery;
#[path = "rollback_claim_tests.rs"]
mod rollback_claim;
#[path = "rollback_receipt_tests.rs"]
mod rollback_receipt;
#[path = "upgrade_tests.rs"]
mod upgrade;
