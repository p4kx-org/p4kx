// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Mutex;

use p4kx_assembly::{
    LoadoutDocumentKind, LoadoutError, LoadoutFileError, LoadoutReadEvent, LoadoutReadObserver,
};
use p4kx_extraction::{DataForgeError, Record, RecordTree, ValueError};
use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum Fault {
    #[error("canonical_source_failure")]
    Canonical {
        record_index: u32,
        #[source]
        source: DataForgeError,
    },
    #[error("canonical_field_failure")]
    Field {
        record_index: u32,
        #[source]
        source: ValueError,
    },
    #[error("required_file_read_failure")]
    FileRead {
        entry_index: Option<usize>,
        path: String,
        kind: LoadoutDocumentKind,
        #[source]
        source: LoadoutFileError,
    },
    #[error("required_document_projection_failure")]
    FileProjection {
        entry_index: Option<usize>,
        path: String,
        kind: LoadoutDocumentKind,
        reason: String,
    },
    #[error("required_record_rejected")]
    Rejected {
        record_index: u32,
        #[source]
        source: LoadoutError,
    },
    #[error("required_inventory_incomplete")]
    IncompleteInventory,
    #[error("poisoned_fixture_evidence")]
    PoisonedEvidence,
}

impl Fault {
    pub(super) const fn category(&self) -> &'static str {
        match self {
            Self::Canonical { .. } => "canonical_source_failure",
            Self::Field { .. } => "canonical_field_failure",
            Self::FileRead { .. } => "required_file_read_failure",
            Self::FileProjection { .. } => "required_document_projection_failure",
            Self::Rejected { .. } => "required_record_rejected",
            Self::IncompleteInventory => "required_inventory_incomplete",
            Self::PoisonedEvidence => "poisoned_fixture_evidence",
        }
    }
}

#[derive(Default)]
pub(super) struct Observer {
    fault: Mutex<Option<Fault>>,
}

impl Observer {
    pub(super) fn finish(&self) -> super::Result<()> {
        let fault = self
            .fault
            .lock()
            .map_err(|_| Fault::PoisonedEvidence)?
            .take();
        fault.map_or(Ok(()), |fault| Err(fault.into()))
    }
}

impl LoadoutReadObserver for Observer {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let fault = event_fault(&event);
        let Ok(mut first) = self.fault.lock() else {
            return ControlFlow::Break(());
        };
        if first.is_none() {
            *first = fault;
        }
        let stop = first.is_some();
        drop(first);
        if stop {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

fn event_fault(event: &LoadoutReadEvent<'_, '_>) -> Option<Fault> {
    match *event {
        LoadoutReadEvent::CanonicalRead { record, outcome } => match outcome {
            Ok(tree) => malformed(record, tree),
            Err(error) => Some(Fault::Canonical {
                record_index: record.index(),
                source: error.clone(),
            }),
        },
        LoadoutReadEvent::Rejected { record, error } => Some(Fault::Rejected {
            record_index: record.index(),
            source: error.clone(),
        }),
        LoadoutReadEvent::FileRead {
            source,
            kind,
            outcome: Err(error),
        } if !matches!(error, LoadoutFileError::NotFound) => Some(Fault::FileRead {
            entry_index: source.identity().entry_index,
            path: source.identity().path.to_owned(),
            kind,
            source: error.clone(),
        }),
        LoadoutReadEvent::FileProjection {
            source,
            kind,
            outcome: Err(reason),
        } => Some(Fault::FileProjection {
            entry_index: source.identity().entry_index,
            path: source.identity().path.to_owned(),
            kind,
            reason: reason.to_owned(),
        }),
        LoadoutReadEvent::InventoryIncomplete => Some(Fault::IncompleteInventory),
        LoadoutReadEvent::ProvidedTree { .. }
        | LoadoutReadEvent::Reused { .. }
        | LoadoutReadEvent::FileRead { .. }
        | LoadoutReadEvent::FileProjection { .. }
        | LoadoutReadEvent::FileReused { .. } => None,
    }
}

pub(super) fn canonical<'f>(record: &Record<'f>) -> super::Result<RecordTree<'f>> {
    let tree = record.tree().map_err(|source| Fault::Canonical {
        record_index: record.index(),
        source,
    })?;
    if let Some(fault) = malformed(record, &tree) {
        return Err(fault.into());
    }
    Ok(tree)
}
fn malformed(record: &Record<'_>, tree: &RecordTree<'_>) -> Option<Fault> {
    let mut errors = tree.errors().filter(|(_, error)| {
        !matches!(
            error,
            ValueError::DanglingReference { .. } | ValueError::OutsideRecord { .. }
        )
    });
    errors.next().map(|(_, source)| Fault::Field {
        record_index: record.index(),
        source: *source,
    })
}
