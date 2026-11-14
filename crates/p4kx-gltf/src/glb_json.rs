// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::num::NonZeroU64;

use crate::GltfError;

pub(crate) fn encode(
    root: &gltf_json::Root,
    budget: Option<NonZeroU64>,
) -> Result<Vec<u8>, GltfError> {
    let mut bytes = match budget {
        Some(budget) => BudgetedJson::encode(root, budget)?,
        None => root.to_vec()?,
    };
    let padded = bytes
        .len()
        .checked_next_multiple_of(4)
        .ok_or_else(overflow)?;
    bytes.resize(padded, b' ');
    Ok(bytes)
}

struct BudgetedJson {
    retained: Vec<u8>,
    len: u64,
    budget: NonZeroU64,
}

impl BudgetedJson {
    fn encode(root: &gltf_json::Root, budget: NonZeroU64) -> Result<Vec<u8>, GltfError> {
        let mut json = Self {
            retained: Vec::new(),
            len: 0,
            budget,
        };
        root.to_writer(&mut json)?;
        let actual = json.len.checked_next_multiple_of(4).ok_or_else(overflow)?;
        if actual > budget.get() {
            return Err(GltfError::JsonBudgetExceeded {
                actual,
                budget: budget.get(),
            });
        }
        Ok(json.retained)
    }
}

impl Write for BudgetedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.len = self
            .len
            .checked_add(bytes.len() as u64)
            .ok_or_else(overflow)?;
        // Continue counting for the final padded size after the buffer stops growing.
        if self.len <= self.budget.get() {
            self.retained
                .try_reserve(bytes.len())
                .map_err(|error| io::Error::new(io::ErrorKind::OutOfMemory, error))?;
            self.retained.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn overflow() -> io::Error {
    io::Error::new(io::ErrorKind::FileTooLarge, "GLB JSON byte count overflow")
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use gltf_json::Root;

    use super::encode;
    use crate::GltfError;

    #[test]
    fn padded_budget_preserves_the_raw_output_or_reports_exact_required_bytes() {
        let root = Root::default();
        let raw_size = root.to_vec().expect("JSON root").len() as u64;
        let size = raw_size.next_multiple_of(4);
        let budget = |bytes| Some(NonZeroU64::new(bytes).expect("nonzero budget"));
        let raw = encode(&root, None).expect("format only");
        assert_eq!(
            encode(&root, budget(size)).expect("exact budget"),
            raw,
            "an admitted document keeps the same encoded bytes"
        );
        assert!(
            matches!(
                encode(&root, budget(size - 1)),
                Err(GltfError::JsonBudgetExceeded { actual, budget })
                    if actual == size && budget == size - 1
            ),
            "the budget includes required padding"
        );
        assert!(
            matches!(
                encode(&root, budget(4)),
                Err(GltfError::JsonBudgetExceeded { actual, budget: 4 }) if actual == size
            ),
            "counting after the budget still reports the full size"
        );
    }
}
