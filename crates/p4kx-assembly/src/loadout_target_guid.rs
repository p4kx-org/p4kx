// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::sync::Arc;

use p4kx_dataforge::Guid;

use crate::{
    LoadoutClassName, LoadoutClassSelection, LoadoutReference, LoadoutReferenceSelection,
    LoadoutXmlClassUse,
};

#[derive(Debug, Clone)]
enum TargetOrigin {
    Reference(LoadoutReferenceSelection),
    Class {
        selection: LoadoutClassSelection,
        xml_use: Option<Arc<LoadoutXmlClassUse>>,
    },
}

/// A target GUID paired with its actual authored Reference or selected class-name operation.
#[derive(Debug, Clone)]
pub struct LoadoutTargetGuid {
    guid: Guid,
    origin: Option<TargetOrigin>,
}

impl LoadoutTargetGuid {
    /// Creates a GUID without an original target-selection claim.
    #[must_use]
    pub const fn unbound(guid: Guid) -> Self {
        Self { guid, origin: None }
    }

    /// Returns the exact target GUID of this failure.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Borrows the authored Reference getter, when this GUID came from one.
    #[must_use]
    pub const fn selection(&self) -> Option<&LoadoutReferenceSelection> {
        match &self.origin {
            Some(TargetOrigin::Reference(selection)) => Some(selection),
            _ => None,
        }
    }

    /// Borrows the actual class getter whose selected Record supplied this GUID.
    #[must_use]
    pub const fn class_selection(&self) -> Option<&LoadoutClassSelection> {
        match &self.origin {
            Some(TargetOrigin::Class { selection, .. }) => Some(selection),
            _ => None,
        }
    }

    /// Borrows the XML occurrence that supplied this selected class, if any.
    #[must_use]
    pub fn class_xml_use(&self) -> Option<&LoadoutXmlClassUse> {
        match &self.origin {
            Some(TargetOrigin::Class { xml_use, .. }) => xml_use.as_deref(),
            _ => None,
        }
    }

    pub(crate) fn from_reference(reference: &LoadoutReference) -> Self {
        Self {
            guid: reference.guid(),
            origin: reference.selection().cloned().map(TargetOrigin::Reference),
        }
    }

    pub(crate) fn from_class(guid: Guid, class: &LoadoutClassName) -> Self {
        Self {
            guid,
            origin: class
                .selection()
                .cloned()
                .map(|selection| TargetOrigin::Class {
                    selection,
                    xml_use: None,
                }),
        }
    }

    pub(crate) fn for_failure(&self, usage: Option<&LoadoutXmlClassUse>) -> Self {
        let mut target = self.clone();
        if let (Some(TargetOrigin::Class { xml_use, .. }), Some(usage)) =
            (&mut target.origin, usage)
        {
            *xml_use = Some(Arc::new(usage.clone()));
        }
        target
    }
}

impl PartialEq for LoadoutTargetGuid {
    fn eq(&self, other: &Self) -> bool {
        self.guid == other.guid
    }
}
impl Eq for LoadoutTargetGuid {}
impl Display for LoadoutTargetGuid {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.guid, formatter)
    }
}
