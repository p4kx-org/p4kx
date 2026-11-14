// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Where the value of a part's attribute comes from once the Modification is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ValueOrigin {
    /// The implementation file named by the Entity.
    #[default]
    Base,
    /// The patch file of the Modification, which replaced or merged the part.
    Patch,
    /// An `Elem` of the Modification, which set the attribute.
    Elem,
}

impl ValueOrigin {
    /// Returns the stable code of the origin, e.g. `elem`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Patch => "patch",
            Self::Elem => "elem",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct DeclarationOrigins {
    pub(super) base: Option<p4kx_extraction::EntryOrigin>,
    pub(super) patch: Option<p4kx_extraction::EntryOrigin>,
}

impl PartialEq for DeclarationOrigins {
    fn eq(&self, other: &Self) -> bool {
        let binding = |origin: &Option<p4kx_extraction::EntryOrigin>| {
            origin.as_ref().map(p4kx_extraction::EntryOrigin::binding)
        };
        binding(&self.base) == binding(&other.base) && binding(&self.patch) == binding(&other.patch)
    }
}
