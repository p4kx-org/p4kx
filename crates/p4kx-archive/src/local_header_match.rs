// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::format_error::FormatError;
use crate::source::Source;

pub(crate) const LOCAL_HEADER_LEN: usize = 30;
pub(crate) const LOCAL_NAME_AT: usize = 26;
pub(crate) const LOCAL_EXTRA_AT: usize = 28;
const LOCAL_SIGNATURE: [u8; 4] = 0x0403_4B50_u32.to_le_bytes();
const LOCAL_CIG_SIGNATURE: [u8; 4] = 0x1403_4B50_u32.to_le_bytes();

pub(crate) struct LocalStart {
    pub(crate) data_start: u64,
    pub(crate) cig: bool,
    pub(crate) name_len: u16,
    pub(crate) extra_len: u16,
}

/// Matches an original local header and its complete original name without reading payload bytes.
pub(crate) fn read(
    source: &Source,
    offset: u64,
    name: &[u8],
) -> Result<Option<LocalStart>, FormatError> {
    if offset
        .checked_add((LOCAL_HEADER_LEN + name.len()) as u64)
        .is_none_or(|end| end > source.len())
    {
        return Ok(None);
    }
    let mut fixed = [0; LOCAL_HEADER_LEN];
    source.read_into(offset, &mut fixed)?;
    let signature = fixed.first_chunk::<4>();
    let cig = signature == Some(&LOCAL_CIG_SIGNATURE);
    if !cig && signature != Some(&LOCAL_SIGNATURE) {
        return Ok(None);
    }
    let Some((name_len, extra_len)) = local_lengths(&fixed) else {
        return Ok(None);
    };
    if usize::from(name_len) != name.len() {
        return Ok(None);
    }
    let mut stored = vec![0; name.len()];
    source.read_into(offset + LOCAL_HEADER_LEN as u64, &mut stored)?;
    if stored != name {
        return Ok(None);
    }
    let data_start = offset
        .checked_add(LOCAL_HEADER_LEN as u64)
        .and_then(|start| start.checked_add(u64::from(name_len)))
        .and_then(|start| start.checked_add(u64::from(extra_len)));
    Ok(data_start.map(|data_start| LocalStart {
        data_start,
        cig,
        name_len,
        extra_len,
    }))
}

fn local_lengths(fixed: &[u8; LOCAL_HEADER_LEN]) -> Option<(u16, u16)> {
    let lengths = fixed.get(LOCAL_NAME_AT..)?.first_chunk::<4>()?;
    let [name_low, name_high, extra_low, extra_high] = *lengths;
    Some((
        u16::from_le_bytes([name_low, name_high]),
        u16::from_le_bytes([extra_low, extra_high]),
    ))
}

#[cfg(test)]
#[path = "local_header_match_tests.rs"]
mod tests;
