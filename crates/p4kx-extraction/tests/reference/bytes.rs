// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

const BLOCK: usize = 64 << 10;

/// How the bytes P4KX decodes relate to the bytes a reference tool wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    Equal,
    /// The first byte that differs.
    Differs {
        offset: u64,
    },
    /// The reference ends first; `zeros_only` tells whether the extra bytes of P4KX are all zero.
    ReferenceShorter {
        zeros_only: bool,
        extra: u64,
        reference_len: u64,
        last_reference_byte: Option<u8>,
    },
    /// The reference has bytes after the end of P4KX's.
    ReferenceLonger {
        extra: u64,
    },
}

fn fill(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while let Some(rest) = buffer.get_mut(filled..).filter(|rest| !rest.is_empty()) {
        match reader.read(rest) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

fn drain(
    reader: &mut impl Read,
    buffer: &mut [u8],
    mut zeros_only: bool,
) -> io::Result<(bool, u64)> {
    let mut total = 0;
    loop {
        let read = fill(reader, buffer)?;
        if read == 0 {
            return Ok((zeros_only, total));
        }
        total += read as u64;
        zeros_only &= buffer.iter().take(read).all(|byte| *byte == 0);
    }
}

/// Compares `ours` with `reference` block by block, holding two blocks in memory.
///
/// # Errors
///
/// Returns the first read error of either side.
pub(crate) fn compare_streams(
    mut ours: impl Read,
    mut reference: impl Read,
) -> io::Result<Verdict> {
    let mut left = vec![0_u8; BLOCK];
    let mut right = vec![0_u8; BLOCK];
    let mut offset = 0_u64;
    let mut last_reference_byte = None;
    loop {
        let a = fill(&mut ours, &mut left)?;
        let b = fill(&mut reference, &mut right)?;
        if let Some(index) = b.checked_sub(1) {
            last_reference_byte = right.get(index).copied();
        }
        let common = a.min(b);
        if let Some(position) = left
            .iter()
            .zip(&right)
            .take(common)
            .position(|(x, y)| x != y)
        {
            return Ok(Verdict::Differs {
                offset: offset + position as u64,
            });
        }
        if a > common {
            let zeros = left.iter().take(a).skip(common).all(|byte| *byte == 0);
            let (zeros_only, rest) = drain(&mut ours, &mut left, zeros)?;
            return Ok(Verdict::ReferenceShorter {
                zeros_only,
                extra: (a - common) as u64 + rest,
                reference_len: offset + b as u64,
                last_reference_byte,
            });
        }
        if b > common {
            let (_, rest) = drain(&mut reference, &mut right, true)?;
            return Ok(Verdict::ReferenceLonger {
                extra: (b - common) as u64 + rest,
            });
        }
        if common < BLOCK {
            return Ok(Verdict::Equal);
        }
        offset += common as u64;
    }
}
