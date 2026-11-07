// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;

pub(crate) fn code(input: &[u16]) -> Result<u128, Fault> {
    let words = match input {
        [left, middle @ .., right] if *left == u16::from(b'{') && *right == u16::from(b'}') => {
            middle
        }
        middle => middle,
    };
    if words.len() != 36 {
        return Err(Fault::InvalidIdentity);
    }
    let mut value = 0u128;
    let mut digits = 0;
    for (position, word) in words.iter().copied().enumerate() {
        if [8, 13, 18, 23].contains(&position) {
            if word != u16::from(b'-') {
                return Err(Fault::InvalidIdentity);
            }
            continue;
        }
        let digit = char::from_u32(u32::from(word))
            .and_then(|character| character.to_digit(16))
            .ok_or(Fault::InvalidIdentity)?;
        value = value
            .checked_mul(16)
            .and_then(|value| value.checked_add(u128::from(digit)))
            .ok_or(Fault::InvalidIdentity)?;
        digits += 1;
    }
    if digits != 32 || value == 0 {
        return Err(Fault::InvalidIdentity);
    }
    Ok(value)
}

pub(crate) fn sequence(words: &[u16]) -> Result<u64, Fault> {
    if words.is_empty()
        || words.len() > 20
        || (words.len() > 1 && words.starts_with(&[u16::from(b'0')]))
    {
        return Err(Fault::InvalidArguments);
    }
    let mut value = 0u64;
    for word in words {
        let digit = word
            .checked_sub(u16::from(b'0'))
            .filter(|digit| *digit < 10)
            .ok_or(Fault::InvalidArguments)?;
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(digit)))
            .ok_or(Fault::SequenceExhausted)?;
    }
    Ok(value)
}
