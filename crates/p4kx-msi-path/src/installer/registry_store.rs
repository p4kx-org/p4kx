// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use winreg::enums::{self, RegType};
use winreg::transaction::Transaction;
use winreg::{RegKey, RegValue};

use crate::installer::binding;
use crate::installer::sequence;

const ENVIRONMENT: &str = "Environment";
const PATH_VALUE: &str = "Path";
const STATE_KEY: &str = r"Software\P4KX-Installer\PathOwnership";
const STATE_VALUE: &str = "StateV1";
const BINDING_KEY: &str = r"Software\P4KX-Installer\PathOwnership\Invocation";
const BINDING_VALUE: &str = "Binding";
const SEQUENCE_VALUE: &str = "Sequence";
const MAX_PATH_BYTES: usize = 1 << 20;
const MAX_STATE_BYTES: usize = 16 << 20;
const RIGHTS: u32 = enums::KEY_READ | enums::KEY_WRITE | enums::KEY_WOW64_64KEY;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawValue {
    pub(crate) kind: RegType,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RegistrySnapshot {
    pub(crate) path: Option<RawValue>,
    pub(crate) state: Option<Vec<u8>>,
    pub(crate) sequence: Option<RawValue>,
    pub(crate) binding: Option<RawValue>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Stage {
    Begin,
    OpenPath,
    OpenState,
    OpenBinding,
    ReadSize,
    ReadValue,
    WritePath,
    WriteState,
    WriteBinding,
    Commit,
}

#[derive(Debug)]
pub(crate) enum RegistryFault {
    Io { stage: Stage, source: io::Error },
    Limit,
    UnsupportedType,
    Conflict,
}

impl std::fmt::Display for RegistryFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { stage, .. } => {
                write!(formatter, "installer registry operation failed: {stage:?}")
            }
            Self::Limit => formatter.write_str("installer registry input exceeds its byte bound"),
            Self::UnsupportedType => {
                formatter.write_str("installer registry input has an unsupported type")
            }
            Self::Conflict => {
                formatter.write_str("installer registry input changed before atomic commit")
            }
        }
    }
}

impl std::error::Error for RegistryFault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub(crate) fn snapshot() -> Result<RegistrySnapshot, RegistryFault> {
    let transaction = Transaction::new().map_err(|source| fault(Stage::Begin, source))?;
    let state = read(&transaction)?;
    transaction
        .commit()
        .map_err(|source| fault(Stage::Commit, source))?;
    Ok(state)
}

pub(crate) fn compare_replace(
    expected: &RegistrySnapshot,
    next: &RegistrySnapshot,
) -> Result<(), RegistryFault> {
    validate(next)?;
    let transaction = Transaction::new().map_err(|source| fault(Stage::Begin, source))?;
    if &read(&transaction)? != expected {
        return Err(RegistryFault::Conflict);
    }
    let root = RegKey::predef(enums::HKEY_CURRENT_USER);
    if next.path != expected.path {
        let (environment, _) = root
            .create_subkey_transacted_with_flags(ENVIRONMENT, &transaction, RIGHTS)
            .map_err(|source| fault(Stage::OpenPath, source))?;
        replace(
            &environment,
            PATH_VALUE,
            next.path.as_ref(),
            Stage::WritePath,
        )?;
    }
    if next.state != expected.state || next.sequence != expected.sequence {
        let (state, _) = root
            .create_subkey_transacted_with_flags(STATE_KEY, &transaction, RIGHTS)
            .map_err(|source| fault(Stage::OpenState, source))?;
        let encoded = next.state.as_ref().map(|bytes| RawValue {
            kind: enums::REG_BINARY,
            bytes: bytes.clone(),
        });
        replace(&state, STATE_VALUE, encoded.as_ref(), Stage::WriteState)?;
        replace(
            &state,
            SEQUENCE_VALUE,
            next.sequence.as_ref(),
            Stage::WriteState,
        )?;
    }
    if next.binding != expected.binding {
        let (binding, _) = root
            .create_subkey_transacted_with_flags(BINDING_KEY, &transaction, RIGHTS)
            .map_err(|source| fault(Stage::OpenBinding, source))?;
        replace(
            &binding,
            BINDING_VALUE,
            next.binding.as_ref(),
            Stage::WriteBinding,
        )?;
    }
    transaction
        .commit()
        .map_err(|source| fault(Stage::Commit, source))
}

fn read(transaction: &Transaction) -> Result<RegistrySnapshot, RegistryFault> {
    let root = RegKey::predef(enums::HKEY_CURRENT_USER);
    let path = match open_optional(&root, ENVIRONMENT, transaction, Stage::OpenPath)? {
        None => None,
        Some(key) => read_optional(&key, PATH_VALUE, MAX_PATH_BYTES)?,
    };
    let (raw_state, sequence) =
        match open_optional(&root, STATE_KEY, transaction, Stage::OpenState)? {
            None => (None, None),
            Some(key) => (
                read_optional(&key, STATE_VALUE, MAX_STATE_BYTES)?,
                read_optional(&key, SEQUENCE_VALUE, MAX_STATE_BYTES)?,
            ),
        };
    let state = match raw_state {
        Some(value) if value.kind == enums::REG_BINARY => Some(value.bytes),
        Some(_) => return Err(RegistryFault::UnsupportedType),
        None => None,
    };
    let binding = match open_optional(&root, BINDING_KEY, transaction, Stage::OpenBinding)? {
        None => None,
        Some(key) => read_optional(&key, BINDING_VALUE, binding::MAX_REGISTRY_BYTES)?,
    };
    let result = RegistrySnapshot {
        path,
        state,
        sequence,
        binding,
    };
    validate(&result)?;
    Ok(result)
}

fn open_optional(
    root: &RegKey,
    name: &str,
    transaction: &Transaction,
    stage: Stage,
) -> Result<Option<RegKey>, RegistryFault> {
    match root.open_subkey_transacted_with_flags(name, transaction, RIGHTS) {
        Ok(key) => Ok(Some(key)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(fault(stage, source)),
    }
}

fn read_optional(
    key: &RegKey,
    name: &str,
    maximum: usize,
) -> Result<Option<RawValue>, RegistryFault> {
    let info = key
        .query_info()
        .map_err(|source| fault(Stage::ReadSize, source))?;
    if usize::try_from(info.max_value_len).map_err(|_| RegistryFault::Limit)? > maximum {
        return Err(RegistryFault::Limit);
    }
    match key.get_raw_value(name) {
        Ok(value) if value.bytes.len() <= maximum => Ok(Some(RawValue {
            kind: value.vtype,
            bytes: value.bytes.into_owned(),
        })),
        Ok(_) => Err(RegistryFault::Limit),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(fault(Stage::ReadValue, source)),
    }
}

fn validate(snapshot: &RegistrySnapshot) -> Result<(), RegistryFault> {
    if let Some(path) = &snapshot.path {
        if !matches!(path.kind, enums::REG_SZ | enums::REG_EXPAND_SZ) {
            return Err(RegistryFault::UnsupportedType);
        }
        if path.bytes.len() > MAX_PATH_BYTES {
            return Err(RegistryFault::Limit);
        }
    }
    if snapshot
        .state
        .as_ref()
        .is_some_and(|bytes| bytes.len() > MAX_STATE_BYTES)
    {
        return Err(RegistryFault::Limit);
    }
    if let Some(value) = &snapshot.binding {
        if value.kind != enums::REG_SZ {
            return Err(RegistryFault::UnsupportedType);
        }
        if value.bytes.len() > binding::MAX_REGISTRY_BYTES {
            return Err(RegistryFault::Limit);
        }
    }
    if let Some(value) = &snapshot.sequence {
        if value.kind != enums::REG_SZ {
            return Err(RegistryFault::UnsupportedType);
        }
        if value.bytes.len() > sequence::MAX_BYTES {
            return Err(RegistryFault::Limit);
        }
    }
    Ok(())
}

fn replace(
    key: &RegKey,
    name: &str,
    value: Option<&RawValue>,
    stage: Stage,
) -> Result<(), RegistryFault> {
    let result = value.map_or_else(
        || match key.delete_value(name) {
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        },
        |value| {
            key.set_raw_value(
                name,
                &RegValue {
                    vtype: value.kind.clone(),
                    bytes: value.bytes.as_slice().into(),
                },
            )
        },
    );
    result.map_err(|source| fault(stage, source))
}

const fn fault(stage: Stage, source: io::Error) -> RegistryFault {
    RegistryFault::Io { stage, source }
}
