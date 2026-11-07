// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

pub(super) type FixtureResult<Value> = Result<Value, Box<dyn Error>>;

#[derive(Debug, Clone, Copy)]
pub(super) struct Words {
    pub(super) date: u16,
    pub(super) time: u16,
}

pub(super) fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> FixtureResult<()> {
    bytes
        .get_mut(offset..offset + value.len())
        .ok_or("fixture field is outside its authored layout")?
        .copy_from_slice(value);
    Ok(())
}
