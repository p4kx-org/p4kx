// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::{SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole};

/// A coordinate in one WEM parse, ready to bind to the actual input.
#[derive(Debug, Clone, Copy)]
pub struct WemOriginSite {
    namespace: &'static str,
    indices: Indices,
    name: &'static str,
    role: SourceUseRole,
}

#[derive(Debug, Clone, Copy)]
enum Indices {
    None,
    One([u64; 1]),
    Two([u64; 2]),
}

impl WemOriginSite {
    pub(super) const fn global(
        namespace: &'static str,
        name: &'static str,
        role: SourceUseRole,
    ) -> Self {
        Self {
            namespace,
            indices: Indices::None,
            name,
            role,
        }
    }

    pub(super) const fn single(
        namespace: &'static str,
        index: u64,
        name: &'static str,
        role: SourceUseRole,
    ) -> Self {
        Self {
            namespace,
            indices: Indices::One([index]),
            name,
            role,
        }
    }

    pub(super) const fn pair(
        namespace: &'static str,
        indices: [u64; 2],
        name: &'static str,
        role: SourceUseRole,
    ) -> Self {
        Self {
            namespace,
            indices: Indices::Two(indices),
            name,
            role,
        }
    }

    /// Returns the original coordinate namespace.
    #[must_use]
    pub const fn namespace(&self) -> &'static str {
        self.namespace
    }

    /// Returns the zero-based coordinates in the decoded WEM.
    #[must_use]
    pub const fn indices(&self) -> &[u64] {
        match &self.indices {
            Indices::None => &[],
            Indices::One(indices) => indices,
            Indices::Two(indices) => indices,
        }
    }

    /// Returns the original field or decision label.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the contribution's role in the result.
    #[must_use]
    pub const fn role(&self) -> SourceUseRole {
        self.role
    }

    /// Binds the planned contribution to the payload actually parsed.
    ///
    /// # Errors
    /// Returns an error when the source evidence registry cannot record the use.
    pub fn bind(&self, payload: &SourceScope) -> Result<SourceUse, SourceEvidenceError> {
        payload.at(
            wem_location(self.namespace(), self.indices(), self.name(), None),
            self.role(),
        )
    }

    /// Binds this WEM-local coordinate to its observed half-open range in a decoded soundbank.
    ///
    /// # Errors
    /// Returns an error when the source evidence registry cannot record the use.
    pub fn bind_embedded(
        &self,
        bank: &SourceScope,
        bytes: Range<u64>,
    ) -> Result<SourceUse, SourceEvidenceError> {
        bank.at(
            wem_location(self.namespace(), self.indices(), self.name(), Some(bytes)),
            self.role(),
        )
    }
}

pub(crate) fn wem_location(
    namespace: &str,
    coordinates: &[u64],
    field: &str,
    member: Option<Range<u64>>,
) -> SourceLocation {
    let (namespace, indices) = member.map_or_else(
        || (namespace.to_owned(), coordinates.to_vec()),
        |member| {
            let mut indices = vec![member.start, member.end];
            indices.extend_from_slice(coordinates);
            (format!("bnkWem.{namespace}"), indices)
        },
    );
    SourceLocation::Subresource {
        namespace,
        indices,
        field: field.to_owned(),
    }
}
