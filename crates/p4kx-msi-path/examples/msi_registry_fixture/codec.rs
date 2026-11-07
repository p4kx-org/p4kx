// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_msi_path::native_fixture::{
    CoreInspection, CorePhase, FixtureGuard, FixtureImage, FixtureKeys, FixtureValue, KeyPresence,
    OwnerClaim,
};
use serde::{Deserialize, Serialize};

const SCHEMA: u32 = 1;
const ROLE: &str = "disposable-ci-registry-fixture";
const MAX_RAW_BYTES: usize = 16 << 20;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    schema: u32,
    role: String,
    run: String,
    attempt: String,
    revision: String,
    process_arch: String,
    root: String,
    image: Image,
    core: Core,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Image {
    keys: Keys,
    path: Option<Value>,
    state: Option<Value>,
    sequence: Option<Value>,
    binding: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent key-existence observations in the fixed wire schema."
)]
struct Keys {
    environment: bool,
    installer: bool,
    ownership: bool,
    invocation: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Value {
    registry_type: u32,
    bytes_hex: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Seed {
    schema: u32,
    path: Option<Value>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
enum Core {
    Empty,
    Rejected {
        reason: String,
    },
    Valid {
        sequence: String,
        phase: Option<Phase>,
        owner: Option<Owner>,
        reserved_binding: Option<String>,
        cancelled_binding: Option<String>,
        journal_binding: Option<String>,
    },
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Phase {
    Prepared,
    Applied,
    Committed,
    RolledBack,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Claim {
    Borrowed,
    Inserted,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    product: String,
    package: String,
    claim: Claim,
}

impl Envelope {
    pub(super) fn from_image(guard: &FixtureGuard, image: &FixtureImage) -> io::Result<Self> {
        let [path, state, sequence, binding] = image.values();
        let [environment, installer, ownership, invocation] = image
            .keys()
            .values()
            .map(|presence| presence == KeyPresence::Present);
        Ok(Self {
            schema: SCHEMA,
            role: ROLE.into(),
            run: guard.run().into(),
            attempt: guard.attempt().into(),
            revision: guard.revision().into(),
            process_arch: std::env::consts::ARCH.into(),
            root: guard.root().to_str().ok_or_else(invalid)?.into(),
            image: Image {
                keys: Keys {
                    environment,
                    installer,
                    ownership,
                    invocation,
                },
                path: path.map(Value::from_raw),
                state: state.map(Value::from_raw),
                sequence: sequence.map(Value::from_raw),
                binding: binding.map(Value::from_raw),
            },
            core: Core::from_inspection(image.inspect_core())?,
        })
    }

    pub(super) fn into_image(self, guard: &FixtureGuard) -> io::Result<FixtureImage> {
        if self.schema != SCHEMA
            || self.role != ROLE
            || self.run != guard.run()
            || self.attempt != guard.attempt()
            || self.revision != guard.revision()
            || self.process_arch != std::env::consts::ARCH
            || Some(self.root.as_str()) != guard.root().to_str()
        {
            return Err(invalid());
        }
        let image = self.image.into_raw()?;
        if self.core != Core::from_inspection(image.inspect_core())? {
            return Err(invalid());
        }
        Ok(image)
    }
}

impl Image {
    fn into_raw(self) -> io::Result<FixtureImage> {
        let [environment, installer, ownership, invocation] = [
            self.keys.environment,
            self.keys.installer,
            self.keys.ownership,
            self.keys.invocation,
        ]
        .map(|present| {
            if present {
                KeyPresence::Present
            } else {
                KeyPresence::Absent
            }
        });
        FixtureImage::new(
            FixtureKeys::new(environment, installer, ownership, invocation)?,
            self.path.map(Value::into_raw).transpose()?,
            self.state.map(Value::into_raw).transpose()?,
            self.sequence.map(Value::into_raw).transpose()?,
            self.binding.map(Value::into_raw).transpose()?,
        )
    }
}

impl Seed {
    pub(super) fn into_path(self) -> io::Result<Option<FixtureValue>> {
        if self.schema != SCHEMA {
            return Err(invalid());
        }
        self.path.map(Value::into_raw).transpose()
    }
}

impl Value {
    fn from_raw(value: &FixtureValue) -> Self {
        let mut bytes_hex = String::with_capacity(value.bytes().len().saturating_mul(2));
        for byte in value.bytes() {
            bytes_hex.push(hex_digit(byte >> 4));
            bytes_hex.push(hex_digit(byte & 15));
        }
        Self {
            registry_type: value.kind(),
            bytes_hex,
        }
    }

    fn into_raw(self) -> io::Result<FixtureValue> {
        if self.bytes_hex.len() > MAX_RAW_BYTES * 2 || !self.bytes_hex.len().is_multiple_of(2) {
            return Err(invalid());
        }
        let mut bytes = Vec::with_capacity(self.bytes_hex.len() / 2);
        for [high, low] in self.bytes_hex.as_bytes().as_chunks::<2>().0 {
            bytes.push(digit(*high)?.checked_mul(16).ok_or_else(invalid)? | digit(*low)?);
        }
        FixtureValue::new(self.registry_type, bytes)
    }
}

impl Core {
    fn from_inspection(inspection: CoreInspection) -> io::Result<Self> {
        Ok(match inspection {
            CoreInspection::Empty => Self::Empty,
            CoreInspection::Rejected { reason } => Self::Rejected { reason },
            CoreInspection::Valid {
                sequence,
                phase,
                owner,
                reserved_binding,
                cancelled_binding,
                journal_binding,
            } => Self::Valid {
                sequence: sequence.to_string(),
                phase: phase.map(Phase::from_core).transpose()?,
                owner: owner
                    .map(|owner| -> io::Result<Owner> {
                        Ok(Owner {
                            product: format!("{:032x}", owner.product),
                            package: format!("{:032x}", owner.package),
                            claim: match owner.claim {
                                OwnerClaim::Borrowed => Claim::Borrowed,
                                OwnerClaim::Inserted => Claim::Inserted,
                                _ => return Err(invalid()),
                            },
                        })
                    })
                    .transpose()?,
                reserved_binding,
                cancelled_binding,
                journal_binding,
            },
            _ => return Err(invalid()),
        })
    }
}

impl Phase {
    fn from_core(phase: CorePhase) -> io::Result<Self> {
        Ok(match phase {
            CorePhase::Prepared => Self::Prepared,
            CorePhase::Applied => Self::Applied,
            CorePhase::Committed => Self::Committed,
            CorePhase::RolledBack => Self::RolledBack,
            _ => return Err(invalid()),
        })
    }
}

fn digit(byte: u8) -> io::Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(invalid()),
    }
}

fn hex_digit(nibble: u8) -> char {
    char::from(if nibble < 10 {
        b'0' + nibble
    } else {
        b'a' + nibble - 10
    })
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid or mismatched raw Registry evidence",
    )
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
