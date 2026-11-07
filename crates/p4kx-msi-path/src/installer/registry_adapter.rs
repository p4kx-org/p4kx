// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use winreg::enums;

use crate::installer::fault::Fault;
use crate::installer::journal_codec;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::registry_store;
use crate::installer::registry_store::{RawValue, RegistryFault, RegistrySnapshot};
use crate::installer::sequence;
use crate::installer::state::{Observed, Snapshot, Store};

pub(crate) struct Registry;

#[derive(Debug)]
pub(crate) enum AdapterFault {
    Core(Fault),
    Registry(RegistryFault),
}

impl From<Fault> for AdapterFault {
    fn from(fault: Fault) -> Self {
        Self::Core(fault)
    }
}

impl From<RegistryFault> for AdapterFault {
    fn from(fault: RegistryFault) -> Self {
        Self::Registry(fault)
    }
}

impl std::fmt::Display for AdapterFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(error) => error.fmt(formatter),
            Self::Registry(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AdapterFault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Core(error) => Some(error),
            Self::Registry(error) => Some(error),
        }
    }
}

impl Store for Registry {
    type Revision = RegistrySnapshot;
    type Error = AdapterFault;

    fn load(&self) -> Result<Observed<RegistrySnapshot>, AdapterFault> {
        let revision = registry_store::snapshot()?;
        let mut state = stored(&revision)?;
        state.path = path(revision.path.as_ref())?;
        Ok(Observed { state, revision })
    }

    fn compare_replace(
        &mut self,
        expected: &Observed<RegistrySnapshot>,
        replacement: Snapshot,
    ) -> Result<(), AdapterFault> {
        let encoded = journal_codec::encode(&replacement)?;
        let sequence = RawValue {
            kind: enums::REG_SZ,
            bytes: sequence::encode(replacement.sequence),
        };
        let path = match replacement.path {
            RawPath::Absent => None,
            RawPath::Present { kind, bytes } => Some(RawValue {
                kind: match kind {
                    StringKind::Plain => enums::REG_SZ,
                    StringKind::Expandable => enums::REG_EXPAND_SZ,
                },
                bytes,
            }),
        };
        let next = RegistrySnapshot {
            path,
            state: Some(encoded),
            sequence: Some(sequence),
        };
        registry_store::compare_replace(&expected.revision, &next)?;
        Ok(())
    }
}

fn stored(revision: &RegistrySnapshot) -> Result<Snapshot, Fault> {
    match (&revision.state, &revision.sequence) {
        (None, None) => Ok(Snapshot {
            path: RawPath::Absent,
            owner: None,
            journal: None,
            sequence: 0,
        }),
        (Some(bytes), Some(scalar)) => {
            let state = journal_codec::decode(bytes)?;
            if sequence::decode(&scalar.bytes)? != state.sequence {
                return Err(Fault::CorruptState);
            }
            Ok(state)
        }
        _ => Err(Fault::CorruptState),
    }
}

fn path(value: Option<&RawValue>) -> Result<RawPath, Fault> {
    let Some(value) = value else {
        return Ok(RawPath::Absent);
    };
    let kind = match value.kind {
        enums::REG_SZ => StringKind::Plain,
        enums::REG_EXPAND_SZ => StringKind::Expandable,
        _ => return Err(Fault::InvalidRawPath),
    };
    let path = RawPath::Present {
        kind,
        bytes: value.bytes.clone(),
    };
    path.split()?;
    Ok(path)
}
