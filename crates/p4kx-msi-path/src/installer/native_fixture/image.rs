// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use winreg::enums::RegType;

/// Whether a fixed Registry key exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyPresence {
    /// The key does not exist.
    Absent,
    /// The key exists, including when it has no values.
    Present,
}

/// Exact existence of the four fixed Registry keys observed by the fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixtureKeys {
    environment: KeyPresence,
    installer: KeyPresence,
    ownership: KeyPresence,
    invocation: KeyPresence,
}

impl FixtureKeys {
    /// Constructs a key-existence image and rejects impossible parent relationships.
    ///
    /// # Errors
    /// Returns invalid input when a child exists without its parent.
    pub fn new(
        environment: KeyPresence,
        installer: KeyPresence,
        ownership: KeyPresence,
        invocation: KeyPresence,
    ) -> io::Result<Self> {
        if (invocation == KeyPresence::Present && ownership != KeyPresence::Present)
            || (ownership == KeyPresence::Present && installer != KeyPresence::Present)
        {
            return Err(invalid());
        }
        Ok(Self {
            environment,
            installer,
            ownership,
            invocation,
        })
    }

    /// Returns Environment, Installer, Ownership and Invocation key presence in that order.
    #[must_use]
    pub const fn values(self) -> [KeyPresence; 4] {
        [
            self.environment,
            self.installer,
            self.ownership,
            self.invocation,
        ]
    }
}

/// Registry type and unmodified data bytes for one fixed value.
#[derive(Debug, Clone, PartialEq)]
pub struct FixtureValue {
    pub(super) kind: RegType,
    pub(super) bytes: Vec<u8>,
}

impl FixtureValue {
    /// Accepts a known Windows Registry type and bounded raw data without string decoding.
    ///
    /// # Errors
    /// Returns invalid input for an unknown type or an oversized raw value.
    pub fn new(kind: u32, bytes: Vec<u8>) -> io::Result<Self> {
        let kind = match kind {
            0 => RegType::REG_NONE,
            1 => RegType::REG_SZ,
            2 => RegType::REG_EXPAND_SZ,
            3 => RegType::REG_BINARY,
            4 => RegType::REG_DWORD,
            5 => RegType::REG_DWORD_BIG_ENDIAN,
            6 => RegType::REG_LINK,
            7 => RegType::REG_MULTI_SZ,
            8 => RegType::REG_RESOURCE_LIST,
            9 => RegType::REG_FULL_RESOURCE_DESCRIPTOR,
            10 => RegType::REG_RESOURCE_REQUIREMENTS_LIST,
            11 => RegType::REG_QWORD,
            _ => return Err(invalid()),
        };
        if bytes.len() > super::super::MAX_STATE_BYTES {
            return Err(invalid());
        }
        Ok(Self { kind, bytes })
    }

    /// Returns the original Registry type code.
    #[must_use]
    pub const fn kind(&self) -> u32 {
        match &self.kind {
            RegType::REG_NONE => 0,
            RegType::REG_SZ => 1,
            RegType::REG_EXPAND_SZ => 2,
            RegType::REG_BINARY => 3,
            RegType::REG_DWORD => 4,
            RegType::REG_DWORD_BIG_ENDIAN => 5,
            RegType::REG_LINK => 6,
            RegType::REG_MULTI_SZ => 7,
            RegType::REG_RESOURCE_LIST => 8,
            RegType::REG_FULL_RESOURCE_DESCRIPTOR => 9,
            RegType::REG_RESOURCE_REQUIREMENTS_LIST => 10,
            RegType::REG_QWORD => 11,
        }
    }

    /// Returns zero-length data and original terminator tails without decoding.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact fixed-value data and key existence; unrelated values and key ACLs are outside its scope.
#[derive(Debug, Clone, PartialEq)]
pub struct FixtureImage {
    pub(super) keys: FixtureKeys,
    pub(super) path: Option<FixtureValue>,
    pub(super) state: Option<FixtureValue>,
    pub(super) sequence: Option<FixtureValue>,
    pub(super) binding: Option<FixtureValue>,
}

impl FixtureImage {
    /// Constructs a bounded image without normalising any Registry bytes.
    ///
    /// # Errors
    /// Returns invalid input for inconsistent key existence or excessive slot sizes.
    pub fn new(
        keys: FixtureKeys,
        path: Option<FixtureValue>,
        state: Option<FixtureValue>,
        sequence: Option<FixtureValue>,
        binding: Option<FixtureValue>,
    ) -> io::Result<Self> {
        let image = Self {
            keys,
            path,
            state,
            sequence,
            binding,
        };
        image.validate()?;
        Ok(image)
    }

    /// Returns the exact key-existence image.
    #[must_use]
    pub const fn keys(&self) -> FixtureKeys {
        self.keys
    }

    /// Returns PATH, `StateV1`, Sequence and Binding values in that fixed order.
    #[must_use]
    pub const fn values(&self) -> [Option<&FixtureValue>; 4] {
        [
            self.path.as_ref(),
            self.state.as_ref(),
            self.sequence.as_ref(),
            self.binding.as_ref(),
        ]
    }

    pub(super) fn validate(&self) -> io::Result<()> {
        let [environment, _, ownership, invocation] = self.keys.values();
        for (value, maximum, present) in [
            (&self.path, super::super::MAX_PATH_BYTES, environment),
            (&self.state, super::super::MAX_STATE_BYTES, ownership),
            (
                &self.sequence,
                crate::installer::sequence::MAX_BYTES,
                ownership,
            ),
            (
                &self.binding,
                crate::installer::binding::MAX_REGISTRY_BYTES,
                invocation,
            ),
        ] {
            if value
                .as_ref()
                .is_some_and(|value| value.bytes.len() > maximum || present != KeyPresence::Present)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "invalid raw fixture image")
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
