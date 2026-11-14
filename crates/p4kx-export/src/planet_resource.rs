// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, Filter, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_palette_reference::PlanetPaletteReference;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_archive_path::{LooseArchivePathState, Resolver};
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition,
};
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_table::CatalogTable;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

const FOLDER: &str = "data/libs/planets/resources/";
const EXTENSION: &str = "resource";
const KIND: Kind = Kind::PlanetResources;
pub(super) const FAMILY: DefinitionFamily =
    DefinitionFamily::new(KIND, FOLDER, EXTENSION, "PlanetResource", "guid");

pub(crate) type PlanetResourceTable = CatalogTable<PlanetResource>;

/// One physical loose planet resource and its original definition fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PlanetResource {
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

impl LooseDefinition for PlanetResource {
    fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }

    fn issues(&mut self) -> &mut Vec<DefinitionIssue> {
        &mut self.issues
    }
}

struct Definitions<'input, 'forge, 'source> {
    forge: &'input DataForge<'source>,
    original: &'input OriginalCatalogInput<'forge, 'source>,
    capture: &'input EntryCapture<'input>,
    filter: Option<&'input Filter>,
    cancel: Option<&'input CancelToken>,
    paths: Resolver<'input, 'input, 'input>,
}

pub(crate) fn read(
    archive: &Archive,
    forge: &DataForge<'_>,
    original: &OriginalCatalogInput<'_, '_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
) -> Result<PlanetResourceTable, CatalogError> {
    let mut reader = Definitions {
        forge,
        original,
        capture,
        filter: options.filter,
        cancel: options.cancel,
        paths: Resolver::new(archive, capture, options.cancel),
    };
    loose_definition::read(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        |identity, object| reader.project(identity, object),
    )
}

impl Definitions<'_, '_, '_> {
    fn project(
        &mut self,
        identity: PhysicalEntryIdentity,
        object: &EntryJsonObject<'_, '_>,
    ) -> Result<PlanetResource, CatalogError> {
        let guid = object.field("guid", entry_json_object::guid)?;
        let guid_raw = object.field("guid", entry_json_object::text)?;
        let name = object.field("name", entry_json_object::text)?;
        let minable = object.field("bMinable", entry_json_object::boolean)?;
        let tint_palette_path = object.field("tintPalettePath", entry_json_object::text)?;
        let tint_palette = self.resolve(&tint_palette_path)?;
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

    fn resolve(
        &mut self,
        path: &SourcedValue<String>,
    ) -> Result<PlanetPaletteReference, CatalogError> {
        if let Some(link) = PlanetPaletteReference::of_path(
            path,
            self.forge,
            self.original,
            self.capture,
            self.filter,
            self.cancel,
        )? {
            return Ok(link);
        }
        let resolution = self.paths.resolve(path, |_| true)?;
        Ok(PlanetPaletteReference::ArchiveEntry { resolution })
    }
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
