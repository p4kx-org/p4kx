// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::NonZeroUsize;

use crate::DataForgeError;

/// A private original traversal prefix retained by one canonical read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub(crate) struct Position(Option<NonZeroUsize>);

impl Position {
    pub(super) const ROOT: Self = Self(None);

    pub(crate) const fn is_root(self) -> bool {
        self.0.is_none()
    }
}

#[derive(Debug)]
struct Edge {
    parent: Position,
    ordinal: u64,
}

/// Retains one edge per demanded structural prefix.
#[derive(Debug, Default)]
pub(crate) struct Positions {
    edges: Vec<Edge>,
}

impl Positions {
    #[cfg(test)]
    pub(super) const fn logical_lengths(&self) -> (usize, usize) {
        (self.edges.len(), self.edges.capacity())
    }
    pub(super) fn clear(&mut self) {
        self.edges.clear();
    }

    pub(super) fn append(
        &mut self,
        parent: Position,
        ordinal: u64,
    ) -> Result<Position, DataForgeError> {
        let index = self
            .edges
            .len()
            .checked_add(1)
            .and_then(NonZeroUsize::new)
            .ok_or(DataForgeError::TooLarge {
                part: "Record position",
            })?;
        self.edges.push(Edge { parent, ordinal });
        Ok(Position(Some(index)))
    }

    pub(crate) fn path(
        &self,
        position: Position,
        output: &mut Vec<u64>,
    ) -> Result<(), DataForgeError> {
        output.clear();
        let mut current = position;
        while let Some(index) = current.0 {
            let edge = self
                .edges
                .get(index.get() - 1)
                .ok_or(DataForgeError::CanonicalTreeInvariant)?;
            output.push(edge.ordinal);
            current = edge.parent;
        }
        output.reverse();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Position, Positions};
    use crate::DataForgeError;

    #[test]
    fn shared_prefixes_keep_original_field_positions_above_u32_without_large_storage()
    -> Result<(), DataForgeError> {
        const WIDE_FIELD: u64 = 4_294_967_296;
        let mut positions = Positions::default();
        let parent = positions.append(Position::ROOT, WIDE_FIELD)?;
        let child = positions.append(parent, 7)?;
        let mut path = Vec::new();
        positions.path(child, &mut path)?;
        assert_eq!(
            path,
            [WIDE_FIELD, 7],
            "the full original field ordinal survives the prefix chain"
        );
        assert_eq!(
            positions.logical_lengths().0,
            2,
            "only two structural edges are retained"
        );
        positions.path(parent, &mut path)?;
        assert_eq!(
            path,
            [WIDE_FIELD],
            "reusing scratch retains the exact original parent tuple"
        );
        Ok(())
    }
}
