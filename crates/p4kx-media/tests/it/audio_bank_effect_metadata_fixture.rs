// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestHircObject;

use super::{TestResult, bank};

pub(super) const PLUGIN_ID: u32 = 0x00AF_00F3;
pub(super) const OPAQUE: [u8; 5] = [0x80, 0xAA, 0, 0xFF, 0x59];
pub(super) const NONFINITE: u32 = 0x7FC1_9023;
pub(super) const BANK_DATA_ID: u32 = 0x4321_EE99;

pub(super) fn fixture(version: u32, kind: u8, body: &[u8]) -> Vec<u8> {
    bank(version, TestHircObject::raw(kind, 5, body))
}

pub(super) fn effect() -> TestResult<Vec<u8>> {
    let mut body = parameters(&OPAQUE)?;
    body.push(2);
    for _ in 0..2 {
        body.push(0);
        body.extend(BANK_DATA_ID.to_le_bytes());
    }
    body.extend(rtpc());
    body.extend(states());
    body.extend(2_u16.to_le_bytes());
    for (accumulation, bits) in [(0xF9, NONFINITE), (0xFF, 0)] {
        body.extend([0x81, 0x04, accumulation]);
        body.extend(bits.to_le_bytes());
    }
    Ok(body)
}

pub(super) fn device() -> TestResult<Vec<u8>> {
    let mut body = effect()?;
    body.extend([2, 0xC1, 4]);
    body.extend(0_u32.to_le_bytes());
    body.extend([0xC3, 4]);
    body.extend(BANK_DATA_ID.to_le_bytes());
    body.push(0x80);
    Ok(body)
}

pub(super) fn empty_effect() -> TestResult<Vec<u8>> {
    let mut body = parameters(&[])?;
    body.extend([0; 7]);
    Ok(body)
}

pub(super) fn modulator() -> Vec<u8> {
    let mut body = vec![2, 0xFF, 4];
    body.extend(3_u32.to_le_bytes());
    body.extend(NONFINITE.to_le_bytes());
    body.extend([1, 0xA9]);
    body.extend(0x8000_0000_u32.to_le_bytes());
    body.extend(NONFINITE.to_le_bytes());
    body.extend(rtpc());
    body
}

pub(super) fn truncated_blob() -> Vec<u8> {
    let mut body = PLUGIN_ID.to_le_bytes().to_vec();
    body.extend(u32::MAX.to_le_bytes());
    body.extend(OPAQUE);
    body
}

pub(super) fn truncated_bank_data() -> TestResult<Vec<u8>> {
    let mut body = parameters(&[])?;
    body.extend([u8::MAX, 0]);
    body.extend(BANK_DATA_ID.to_le_bytes());
    body.push(1);
    Ok(body)
}

pub(super) fn malformed_override() -> TestResult<Vec<u8>> {
    let mut body = empty_effect()?;
    body.truncate(body.len() - size_of::<u16>());
    body.extend(u16::MAX.to_le_bytes());
    body.extend([0xFF; 5]);
    Ok(body)
}

fn parameters(bytes: &[u8]) -> TestResult<Vec<u8>> {
    let mut body = PLUGIN_ID.to_le_bytes().to_vec();
    body.extend(u32::try_from(bytes.len())?.to_le_bytes());
    body.extend(bytes);
    Ok(body)
}

fn rtpc() -> Vec<u8> {
    let mut body = 1_u16.to_le_bytes().to_vec();
    body.extend(11_u32.to_le_bytes());
    body.extend([0x87, 0xF2, 0x81, 0x04]);
    body.extend(44_u32.to_le_bytes());
    body.push(0xAB);
    body.extend(1_u16.to_le_bytes());
    body.extend(NONFINITE.to_le_bytes());
    body.extend(0x8000_0000_u32.to_le_bytes());
    body.extend(0xABC9_899A_u32.to_le_bytes());
    body
}

fn states() -> Vec<u8> {
    let mut body = vec![1, 0x81, 0x04, 0xFC, 0xEE, 1];
    body.extend(11_u32.to_le_bytes());
    body.extend([0xF4, 1]);
    body.extend(13_u32.to_le_bytes());
    body.extend(1_u16.to_le_bytes());
    body.extend(0xDEAD_u16.to_le_bytes());
    body.extend(NONFINITE.to_le_bytes());
    body
}
