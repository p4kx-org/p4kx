// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::Kind;
use crate::contract::every_field_required;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticStage, ExportSourceRole};
use crate::export_ledger::{SourceDiagnosis, SourceDisposition};

/// The required operation performed on a physical Archive Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArchiveEntryObservation {
    pub(crate) operation: EntryOperation,
    pub(crate) kind: Kind,
    pub(crate) entry_index: Option<usize>,
    pub(crate) container_entry_index: Option<usize>,
    pub(crate) path: String,
    pub(crate) role: ExportSourceRole,
    pub(crate) disposition: SourceDisposition,
    pub(crate) diagnosis: SourceDiagnosis,
}

/// One exact failed Archive Entry identity, retained independently of bounded examples.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct FailedExportEntry {
    /// The failed required operation on the physical Entry.
    pub operation: EntryOperation,
    /// The affected catalog kind.
    pub kind: Kind,
    /// The outer Archive Entry index, null when unavailable or the configured Entry is missing.
    pub entry_index: Option<usize>,
    /// The nested container Entry index when available.
    pub container_entry_index: Option<usize>,
    /// The canonical Entry path, using `outer!inner` for a nested file.
    pub path: String,
    /// The relationship of the source to the affected kind.
    pub role: ExportSourceRole,
    /// The failed source-read or projection stage.
    pub stage: ExportDiagnosticStage,
    /// The stable failure category.
    pub code: ExportDiagnosticCode,
    /// The technical source field when the failing operation identifies one.
    pub field: Option<String>,
    /// The technical source struct or document type when available.
    pub source_type: Option<String>,
    /// The configured target GUID when the failing operation identifies one.
    #[schemars(with = "Option<crate::guid_text::CatalogGuid>")]
    pub target_guid: Option<String>,
}
