// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Mutex, PoisonError};

use p4kx_extraction::Guid;

/// A value made once per Record GUID and shared by the worker threads.
pub(crate) struct Memo<V> {
    held: Mutex<HashMap<Guid, V>>,
}

impl<V> Default for Memo<V> {
    fn default() -> Self {
        Self {
            held: Mutex::new(HashMap::new()),
        }
    }
}

impl<V: Clone> Memo<V> {
    /// Returns the value held for `guid`, else the value `make` returns, which is then held. Two
    /// threads that miss at once both make it; the values are equal.
    pub(crate) fn get_or_make(&self, guid: Guid, make: impl FnOnce() -> V) -> V {
        match self.get_or_try_make(guid, || Ok::<_, Infallible>(make())) {
            Ok(value) => value,
            Err(error) => match error {},
        }
    }

    /// Returns an observed value by GUID and retains successful observations.
    pub(crate) fn get_or_try_make<E>(
        &self,
        guid: Guid,
        make: impl FnOnce() -> Result<V, E>,
    ) -> Result<V, E> {
        let held = self
            .held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&guid)
            .cloned();
        if let Some(value) = held {
            return Ok(value);
        }
        let value = make()?;
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(guid, value.clone());
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_value_is_made_once_per_guid() {
        let memo = Memo::<u32>::default();
        let guid = Guid::from_stored([1; 16]);
        assert_eq!(memo.get_or_make(guid, || 7), 7);
        assert_eq!(memo.get_or_make(guid, || unreachable!("held")), 7);
        let other = Guid::from_stored([2; 16]);
        assert_eq!(memo.get_or_make(other, || 9), 9);
    }
}
