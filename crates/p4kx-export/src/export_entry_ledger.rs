// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::Kind;
pub use crate::catalog_read::original::FailedExportEntry;
use crate::catalog_read::original::{EntryDiagnosis, EntryReceipt};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::SourceDisposition;
use crate::{ExportAccountingError, HealthText};

/// The required operation performed on a physical Archive Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub enum EntryOperation {
    /// Enumerating the selected Entry within a bounded source walk.
    Enumeration,
    /// Opening a container and reading its directory.
    ContainerDirectory,
    /// Reading and projecting container placement XML.
    ContainerXml,
    /// Reading entity links that place nested containers.
    EntityLinks,
    /// Scanning a SOC for its authored harvest preset.
    SocPreset,
    /// Counting entity tags in SOC content.
    SocTags,
    /// Reading the shop registry.
    ShopRegistry,
    /// Reading an authored shop inventory.
    ShopInventory,
    /// Reading a shop prefab.
    ShopPrefab,
    /// Reading shop metadata XML.
    ShopMetadata,
    /// Reading shop editor XML.
    ShopEditor,
    /// Projecting shop components from SOC content.
    ShopSoc,
    /// Reading another required source XML document.
    SourceXml,
    /// Completing the byte read required by an XML Loadout.
    XmlLoadoutBytes,
    /// Completing the byte read required by a vehicle implementation.
    VehicleImplementationBytes,
    /// Completing the byte read required by a vehicle implementation patch.
    VehiclePatchBytes,
    /// Reading and projecting an XML Loadout document.
    XmlLoadout,
    /// Reading and projecting a vehicle implementation document.
    VehicleImplementation,
    /// Reading and projecting a vehicle implementation patch.
    VehiclePatch,
}

/// The exact identity and outcome of one actually consumed Archive Entry.
#[derive(Debug, Clone)]
pub(crate) struct ArchiveEntryObservation {
    pub(crate) operation: EntryOperation,
    pub(crate) kind: Kind,
    pub(crate) entry_index: Option<usize>,
    pub(crate) container_entry_index: Option<usize>,
    pub(crate) path: HealthText,
    pub(crate) role: ExportSourceRole,
    pub(crate) disposition: SourceDisposition,
    pub(crate) diagnosis: EntryDiagnosis,
    pub(crate) receipt: Option<EntryReceipt>,
}

impl ArchiveEntryObservation {
    pub(crate) fn original(&self) -> Result<&EntryReceipt, ExportAccountingError> {
        self.receipt
            .as_ref()
            .ok_or(ExportAccountingError::UnboundHealthSubject)
    }

    pub(crate) fn merge(&mut self, other: &Self) -> Result<(), ExportAccountingError> {
        if self != other {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        match (&mut self.receipt, &other.receipt) {
            (Some(left), Some(right)) => left.merge(right),
            (None, None) => Ok(()),
            _ => Err(ExportAccountingError::UnboundHealthSubject),
        }
    }
}

impl PartialEq for ArchiveEntryObservation {
    fn eq(&self, other: &Self) -> bool {
        self.operation == other.operation
            && self.kind == other.kind
            && self.entry_index == other.entry_index
            && self.container_entry_index == other.container_entry_index
            && self.path == other.path
            && self.role == other.role
            && self.disposition == other.disposition
            && self.diagnosis == other.diagnosis
    }
}
impl Eq for ArchiveEntryObservation {}
