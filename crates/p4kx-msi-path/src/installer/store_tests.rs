// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use crate::installer::caller::{Caller, Goal};
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::state::{Observed, Request, Snapshot, Store};
use crate::installer::tests;
use crate::installer::transaction;

#[derive(Clone, PartialEq, Eq)]
struct Revision {
    stored: Snapshot,
    actual_path: RawPath,
}

struct VersionedMemory {
    revision: Revision,
}

impl Store for VersionedMemory {
    type Revision = Revision;
    type Error = Fault;

    fn load(&self) -> Result<Observed<Revision>, Fault> {
        let mut state = self.revision.stored.clone();
        state.path = self.revision.actual_path.clone();
        Ok(Observed {
            state,
            revision: self.revision.clone(),
        })
    }

    fn compare_replace(
        &mut self,
        expected: &Observed<Revision>,
        replacement: Snapshot,
    ) -> Result<(), Fault> {
        if expected.revision != self.revision {
            return Err(Fault::Conflict);
        }
        self.revision = Revision {
            actual_path: replacement.path.clone(),
            stored: replacement,
        };
        Ok(())
    }
}

fn path(text: &str) -> RawPath {
    RawPath::Present {
        kind: StringKind::Expandable,
        bytes: text
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect(),
    }
}

#[test]
fn a_foreign_path_change_does_not_require_reencoding_the_observed_wal_token() -> Result<(), Fault> {
    let stored = tests::installed(path("A;B"))?;
    let actual_path = path(r"Z;;C:\P4KX\private-lease-123;A;B;;Q");
    let mut memory = VersionedMemory {
        revision: Revision {
            stored,
            actual_path,
        },
    };
    let expected = memory.revision.stored.sequence;
    let caller = Caller {
        product: 1,
        package: 101,
        nonce: InvocationNonce::from_bytes([2; 32])?,
        predecessor: None,
        goal: Goal::Remove,
    };
    let id = transaction::begin_at(
        &mut memory,
        expected,
        &caller,
        Request::Remove { product: 1 },
    )?;
    transaction::apply(&mut memory, id, &caller)?;
    transaction::commit(&mut memory, id, &caller)?;
    assert_eq!(
        memory.revision.actual_path,
        path("Z;;A;B;;Q"),
        "raw revision comparison preserves foreign edits without a false stale-WAL conflict"
    );
    Ok(())
}

#[derive(Debug)]
enum OriginalFault {
    Core(Fault),
    Io(io::Error),
}

impl From<Fault> for OriginalFault {
    fn from(fault: Fault) -> Self {
        Self::Core(fault)
    }
}

struct Unreadable;

impl Store for Unreadable {
    type Revision = ();
    type Error = OriginalFault;

    fn load(&self) -> Result<Observed<()>, OriginalFault> {
        Err(OriginalFault::Io(io::Error::from_raw_os_error(5)))
    }

    fn compare_replace(
        &mut self,
        _expected: &Observed<()>,
        _replacement: Snapshot,
    ) -> Result<(), OriginalFault> {
        Err(OriginalFault::Core(Fault::Conflict))
    }
}

#[test]
fn the_core_preserves_a_typed_original_adapter_error() -> Result<(), Fault> {
    let caller = Caller {
        product: 1,
        package: 101,
        nonce: InvocationNonce::from_bytes([2; 32])?,
        predecessor: None,
        goal: Goal::Remove,
    };
    let result = transaction::begin_at(&mut Unreadable, 0, &caller, Request::Remove { product: 1 });
    assert!(
        matches!(result, Err(OriginalFault::Io(ref error)) if error.raw_os_error() == Some(5)),
        "native IO identity is not converted into text or a state fault"
    );
    let converted = OriginalFault::from(Fault::WrongPhase);
    assert!(
        matches!(converted, OriginalFault::Core(Fault::WrongPhase)),
        "core errors remain distinguishable from adapter errors"
    );
    Ok(())
}
