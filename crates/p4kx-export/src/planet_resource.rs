// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, Filter, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_palette_reference;
pub use super::planet_palette_reference::PlanetPaletteReference;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_archive_path::{LooseArchivePathState, Resolver};
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition, Selection,
};
use crate::catalog_read::loose_record_link::LooseRecordLink;
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_table::CatalogTable;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

const FOLDER: &str = "data/libs/planets/resources/";
const EXTENSION: &str = "resource";
pub(crate) const KIND: Kind = Kind::PlanetResources;
pub(super) const FAMILY: DefinitionFamily =
    DefinitionFamily::new(KIND, FOLDER, EXTENSION, "PlanetResource", "guid");

pub(crate) type PlanetResourceTable = CatalogTable<PlanetResource>;

/// One physical loose planet resource and its original definition fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct PlanetResource {
    identity: PhysicalEntryIdentity,
    guid: SourcedValue<CatalogGuid>,
    guid_raw: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    name: SourcedValue<String>,
    minable: SourcedValue<bool>,
    tint_palette_path: SourcedValue<String>,
    tint_palette: PlanetPaletteReference,
    issues: Vec<DefinitionIssue>,
}

impl PlanetResource {
    /// Borrows the physical identity of this original Archive Entry.
    #[must_use]
    pub const fn identity(&self) -> &PhysicalEntryIdentity {
        &self.identity
    }

    /// Borrows the canonical GUID and its authored field evidence.
    #[must_use]
    pub const fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }

    /// Borrows the authored GUID spelling.
    #[must_use]
    pub const fn guid_raw(&self) -> &SourcedValue<String> {
        &self.guid_raw
    }

    /// Borrows the original resource name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<String> {
        &self.name
    }

    /// Borrows the original mining flag.
    #[must_use]
    pub const fn minable(&self) -> &SourcedValue<bool> {
        &self.minable
    }

    /// Borrows the original tint palette path.
    #[must_use]
    pub const fn tint_palette_path(&self) -> &SourcedValue<String> {
        &self.tint_palette_path
    }

    /// Borrows the tint palette lookup and its source candidates.
    #[must_use]
    pub const fn tint_palette(&self) -> &PlanetPaletteReference {
        &self.tint_palette
    }

    /// Lists the represented issues of this physical definition.
    #[must_use]
    pub fn issues(&self) -> &[DefinitionIssue] {
        &self.issues
    }
}

impl LooseDefinition for PlanetResource {
    fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }

    fn issues(&mut self) -> &mut Vec<DefinitionIssue> {
        &mut self.issues
    }
}

pub(crate) fn read(
    archive: &Archive,
    forge: &DataForge<'_>,
    original: &OriginalCatalogInput<'_, '_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
) -> Result<PlanetResourceTable, CatalogError> {
    read_selected(
        archive,
        capture,
        ledger,
        options,
        Selection::All,
        |path, authored| {
            resolve_tint_record(
                path,
                authored,
                forge,
                original,
                options.filter,
                options.cancel,
            )
        },
    )
}

pub(crate) fn read_selected(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    selection: Selection<'_>,
    mut palette_record: impl FnMut(&SourcedValue<String>, &str) -> Result<LooseRecordLink, CatalogError>,
) -> Result<PlanetResourceTable, CatalogError> {
    let mut paths = Resolver::new(archive, capture, options.cancel);
    loose_definition::read_selected(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        selection,
        |identity, object| {
            project(identity, object, |path| {
                PlanetPaletteReference::resolve(path, &mut paths, capture, &mut palette_record)
            })
        },
    )
}

pub(crate) fn resolve_tint_record(
    path: &SourcedValue<String>,
    authored: &str,
    forge: &DataForge<'_>,
    original: &OriginalCatalogInput<'_, '_>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<LooseRecordLink, CatalogError> {
    planet_palette_reference::resolve_record(path, authored, forge, original, filter, cancel)
}

fn project(
    identity: PhysicalEntryIdentity,
    object: &EntryJsonObject<'_, '_>,
    resolve_palette: impl FnOnce(&SourcedValue<String>) -> Result<PlanetPaletteReference, CatalogError>,
) -> Result<PlanetResource, CatalogError> {
    let guid = object.field("guid", entry_json_object::guid)?;
    let guid_raw = object.field("guid", entry_json_object::text)?;
    let name = object.field("name", entry_json_object::text)?;
    let minable = object.field("bMinable", entry_json_object::boolean)?;
    let tint_palette_path = object.field("tintPalettePath", entry_json_object::text)?;
    let tint_palette = resolve_palette(&tint_palette_path)?;
    let issues = field_issues(&guid, &name, &minable, tint_palette.state());
    Ok(PlanetResource {
        identity,
        guid,
        guid_raw,
        name,
        minable,
        tint_palette_path,
        tint_palette,
        issues,
    })
}

fn field_issues(
    guid: &SourcedValue<CatalogGuid>,
    name: &SourcedValue<String>,
    minable: &SourcedValue<bool>,
    palette: Option<LooseArchivePathState>,
) -> Vec<DefinitionIssue> {
    let mut issues: Vec<_> = loose_definition::identity_issue("guid", guid)
        .into_iter()
        .collect();
    for (field, state) in [("name", name.state()), ("bMinable", minable.state())] {
        if state == SourceValueState::Invalid {
            issues.push(DefinitionIssue {
                field,
                code: ExportDiagnosticCode::MalformedSource,
            });
        }
    }
    let code = palette.and_then(loose_definition::reference_issue);
    if let Some(code) = code {
        issues.push(DefinitionIssue {
            field: "tintPalettePath",
            code,
        });
    }
    issues
}

#[cfg(test)]
fn in_scope(path: &str) -> bool {
    FAMILY.includes(path)
}

#[cfg(test)]
#[path = "planet_resource_tests.rs"]
mod tests;
