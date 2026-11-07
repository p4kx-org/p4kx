// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;

pub(crate) const MAX_BYTES: usize = 42;

pub(crate) fn encode(sequence: u64) -> Vec<u8> {
    sequence
        .to_string()
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

pub(crate) fn decode(bytes: &[u8]) -> Result<u64, Fault> {
    if bytes.len() < 4 || bytes.len() > MAX_BYTES || !bytes.len().is_multiple_of(2) {
        return Err(Fault::CorruptState);
    }
    let boundary = bytes.len() - 2;
    if bytes.get(boundary..) != Some(&[0, 0]) {
        return Err(Fault::CorruptState);
    }
    let digits = bytes.get(..boundary).ok_or(Fault::CorruptState)?;
    if digits.len() > 2 && digits.starts_with(&[b'0', 0]) {
        return Err(Fault::CorruptState);
    }
    let (words, []) = digits.as_chunks::<2>() else {
        return Err(Fault::CorruptState);
    };
    let mut result = 0u64;
    for word in words {
        let [digit, 0] = word else {
            return Err(Fault::CorruptState);
        };
        if !digit.is_ascii_digit() {
            return Err(Fault::CorruptState);
        }
        result = result
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(digit - b'0')))
            .ok_or(Fault::SequenceExhausted)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use crate::installer::fault::Fault;
    use crate::installer::sequence;

    #[test]
    fn canonical_scalar_round_trips_without_an_msi_type_prefix() {
        for value in [0, 1, 42, u64::MAX] {
            assert_eq!(
                sequence::decode(&sequence::encode(value)),
                Ok(value),
                "canonical decimal REG_SZ is directly consumable by AppSearch"
            );
        }
    }

    #[test]
    fn prefixes_empty_odd_and_noncanonical_scalars_are_rejected() {
        for input in [
            vec![],
            vec![0, 0],
            vec![b'1'],
            vec![b'0', 0, b'1', 0, 0, 0],
            vec![b'#', 0, b'1', 0, 0, 0],
            vec![b'1', 0, 0, 0, 0, 0],
        ] {
            assert_eq!(
                sequence::decode(&input),
                Err(Fault::CorruptState),
                "raw RegLocator values cannot smuggle an alternate receipt spelling"
            );
        }
    }

    #[test]
    fn overflow_is_explicit_and_never_wraps_to_an_older_receipt() {
        let bytes: Vec<u8> = "18446744073709551616"
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(
            sequence::decode(&bytes),
            Err(Fault::SequenceExhausted),
            "receipt arithmetic cannot wrap"
        );
    }
}
