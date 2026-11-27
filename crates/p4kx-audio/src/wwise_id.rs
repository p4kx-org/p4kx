// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

const FNV_OFFSET_BASIS: u32 = 0x811C_9DC5;
const FNV_PRIME: u32 = 0x0100_0193;

/// Returns the Wwise short ID of a name, e.g. of an event or soundbank: the 32-bit FNV-1 hash
/// of the name in ASCII lower case.
#[must_use]
pub fn wwise_id(name: &str) -> u32 {
    name.bytes().fold(FNV_OFFSET_BASIS, |hash, byte| {
        hash.wrapping_mul(FNV_PRIME) ^ u32::from(byte.to_ascii_lowercase())
    })
}
