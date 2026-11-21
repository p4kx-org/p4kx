// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use winreg::enums;
use winreg::transaction::Transaction;
use winreg::{RegKey, RegValue};

use super::super::{self as store, Stage};
use super::{FixtureGuard, FixtureImage, FixtureKeys, FixtureValue, KeyPresence};

pub(super) fn capture(guard: &FixtureGuard) -> io::Result<FixtureImage> {
    guard.active()?;
    let transaction = Transaction::new()?;
    let result = read(&transaction)?;
    transaction.commit()?;
    Ok(result)
}

pub(super) fn replace(
    guard: &FixtureGuard,
    expected: &FixtureImage,
    desired: &FixtureImage,
) -> io::Result<FixtureImage> {
    guard.active()?;
    expected.validate()?;
    desired.validate()?;
    let transaction = Transaction::new()?;
    if read(&transaction)? != *expected {
        return Err(conflict("fixture image changed before replacement"));
    }
    let root = RegKey::predef(enums::HKEY_CURRENT_USER);
    let keys = desired.keys.values();
    let [environment, installer, ownership, invocation] = keys;
    ensure_key(&root, installer_key()?, installer, &transaction)?;
    ensure_key(&root, store::ENVIRONMENT, environment, &transaction)?;
    ensure_key(&root, store::STATE_KEY, ownership, &transaction)?;
    ensure_key(&root, store::BINDING_KEY, invocation, &transaction)?;
    for (key, name, value) in [
        (store::ENVIRONMENT, store::PATH_VALUE, desired.path.as_ref()),
        (store::STATE_KEY, store::STATE_VALUE, desired.state.as_ref()),
        (
            store::STATE_KEY,
            store::SEQUENCE_VALUE,
            desired.sequence.as_ref(),
        ),
        (
            store::BINDING_KEY,
            store::BINDING_VALUE,
            desired.binding.as_ref(),
        ),
    ] {
        write_value(&root, key, name, value, &transaction)?;
    }
    for (key, presence) in [
        (store::BINDING_KEY, invocation),
        (store::STATE_KEY, ownership),
        (installer_key()?, installer),
        (store::ENVIRONMENT, environment),
    ] {
        if presence == KeyPresence::Absent {
            remove_empty(&root, key, &transaction)?;
        }
    }
    let after = read(&transaction)?;
    if after != *desired {
        return Err(conflict(
            "native Registry did not retain the requested raw image",
        ));
    }
    guard.active()?;
    transaction.commit()?;
    Ok(after)
}

fn read(transaction: &Transaction) -> io::Result<FixtureImage> {
    let root = RegKey::predef(enums::HKEY_CURRENT_USER);
    let environment = open(&root, store::ENVIRONMENT, transaction)?;
    let installer = open(&root, installer_key()?, transaction)?;
    let state = open(&root, store::STATE_KEY, transaction)?;
    let invocation = open(&root, store::BINDING_KEY, transaction)?;
    let keys = FixtureKeys::new(
        presence(environment.as_ref()),
        presence(installer.as_ref()),
        presence(state.as_ref()),
        presence(invocation.as_ref()),
    )?;
    FixtureImage::new(
        keys,
        read_value(
            environment.as_ref(),
            store::PATH_VALUE,
            store::MAX_PATH_BYTES,
        )?,
        read_value(state.as_ref(), store::STATE_VALUE, store::MAX_STATE_BYTES)?,
        read_value(
            state.as_ref(),
            store::SEQUENCE_VALUE,
            store::MAX_STATE_BYTES,
        )?,
        read_value(
            invocation.as_ref(),
            store::BINDING_VALUE,
            crate::installer::binding::MAX_REGISTRY_BYTES,
        )?,
    )
}

const fn presence(key: Option<&RegKey>) -> KeyPresence {
    if key.is_some() {
        KeyPresence::Present
    } else {
        KeyPresence::Absent
    }
}

fn open(root: &RegKey, key: &str, transaction: &Transaction) -> io::Result<Option<RegKey>> {
    store::open_optional(root, key, transaction, Stage::ReadValue).map_err(io::Error::other)
}

fn read_value(
    key: Option<&RegKey>,
    name: &str,
    maximum: usize,
) -> io::Result<Option<FixtureValue>> {
    let Some(key) = key else {
        return Ok(None);
    };
    Ok(store::read_optional(key, name, maximum)
        .map_err(io::Error::other)?
        .map(|value| FixtureValue {
            kind: value.kind,
            bytes: value.bytes,
        }))
}

fn ensure_key(
    root: &RegKey,
    key: &str,
    presence: KeyPresence,
    transaction: &Transaction,
) -> io::Result<()> {
    if presence == KeyPresence::Present {
        root.create_subkey_transacted_with_flags(key, transaction, store::RIGHTS)?;
    }
    Ok(())
}

fn write_value(
    root: &RegKey,
    key: &str,
    name: &str,
    value: Option<&FixtureValue>,
    transaction: &Transaction,
) -> io::Result<()> {
    let Some(key) = open(root, key, transaction)? else {
        return if value.is_none() {
            Ok(())
        } else {
            Err(conflict("fixture value has no existing key"))
        };
    };
    value.map_or_else(
        || match key.delete_value(name) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
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
    )
}

fn remove_empty(root: &RegKey, key: &str, transaction: &Transaction) -> io::Result<()> {
    let Some(opened) = open(root, key, transaction)? else {
        return Ok(());
    };
    let info = opened.query_info()?;
    if info.sub_keys != 0 || info.values != 0 {
        return Err(conflict(
            "fixture restoration retains an unrelated Registry key value or child",
        ));
    }
    drop(opened);
    root.delete_subkey_transacted_with_flags(key, transaction, enums::KEY_WOW64_64KEY)
}

fn installer_key() -> io::Result<&'static str> {
    store::STATE_KEY
        .rsplit_once('\\')
        .map(|(parent, _)| parent)
        .ok_or_else(|| conflict("invalid fixed installer key layout"))
}

fn conflict(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
