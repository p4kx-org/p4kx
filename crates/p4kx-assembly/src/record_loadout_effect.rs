// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::spec::{EntrySpec, PortSpec, TargetSpec};
use crate::{LoadoutClassName, LoadoutPath, LoadoutReference};

/// A direct Record Loadout choice stemming from a matched authored port name.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecordLoadoutEffect<'site> {
    /// A direct inline override replaced an earlier Loadout occurrence.
    Override {
        /// Physical Record of the Entity receiving the override.
        owner_record_index: u32,
        /// Sibling position replaced by the resolver.
        ordinal: usize,
        /// Earlier authored occurrence.
        displaced: RecordPortSelection<'site>,
        /// Inline occurrence that replaced it.
        winner: RecordPortSelection<'site>,
    },
    /// A selected entry prevented a declared default item from being used.
    SuppressDefault {
        /// Physical Record that owns the declared default.
        owner_record_index: u32,
        /// Declared port whose default is suppressed.
        declared: RecordPortSelection<'site>,
        /// Selected entry whose name matched the declared port.
        winner: RecordPortSelection<'site>,
    },
}

/// One original Record port named in a matched Loadout choice.
#[derive(Debug)]
pub struct RecordPortSelection<'site> {
    name: &'site str,
    name_proof: Option<&'site LoadoutPath>,
    target: RecordPortTarget<'site>,
}

impl<'site> RecordPortSelection<'site> {
    pub(crate) fn entry(entry: &'site EntrySpec) -> Option<Self> {
        (!entry.origin.is_file()).then(|| Self {
            name: &entry.port,
            name_proof: entry.port_proof.as_ref(),
            target: RecordPortTarget::of(&entry.target),
        })
    }

    pub(crate) fn declared(port: &'site PortSpec) -> Self {
        Self {
            name: &port.name,
            name_proof: Some(&port.name_proof).filter(|proof| proof.selection().is_some()),
            target: RecordPortTarget::of(&port.default_item),
        }
    }

    /// Borrows the original spelling used by the resolver.
    #[must_use]
    pub const fn name(&self) -> &str {
        self.name
    }

    /// Borrows the original same-frame source selection when one is available.
    #[must_use]
    pub const fn name_proof(&self) -> Option<&LoadoutPath> {
        self.name_proof
    }

    /// Returns the kind of target selected from this occurrence before the merge.
    #[must_use]
    pub const fn target(&self) -> RecordPortTarget<'site> {
        self.target
    }
}

/// The actual authored target decision at one Record Loadout port.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum RecordPortTarget<'site> {
    /// The entry selected no Entity.
    Empty,
    /// The entry selected an original Reference.
    Reference(&'site LoadoutReference),
    /// The entry selected an original class name.
    Class(&'site LoadoutClassName),
    /// The source target could not be read.
    Invalid,
}

impl<'site> RecordPortTarget<'site> {
    const fn of(target: &'site TargetSpec) -> Self {
        match target {
            TargetSpec::None => Self::Empty,
            TargetSpec::Reference(reference) => Self::Reference(reference),
            TargetSpec::ClassName(class) => Self::Class(class),
            TargetSpec::Invalid(_) => Self::Invalid,
        }
    }
}
