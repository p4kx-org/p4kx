// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DecimalU64, SourceUses, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_asset_raw_fields::{self, RawPlanetAssetMember};
use super::planet_geology_properties::{GeologyGeometry, GeologyPlacement, GeologySubstrate};
use super::planet_geology_references::{self, GeologyReferences};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_archive_path::Resolver;
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition, Selection,
};
use crate::catalog_read::loose_record_link::{self, GuidRecordLookup, LooseRecordLink};
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_table::CatalogTable;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

pub(super) const FAMILY: DefinitionFamily = DefinitionFamily::new(
    Kind::GeologyAssets,
    "data/libs/planets/geology_assets/",
    "geoasset",
    "PlanetGeologyAsset",
    "GUID",
);

/// A physical loose geology definition and its authored fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct PlanetGeologyAsset {
    /// The physical Entry that owns this definition.
    pub identity: PhysicalEntryIdentity,
    /// The parsed geology asset GUID.
    pub guid: SourcedValue<CatalogGuid>,
    /// The original geology asset GUID text.
    pub guid_raw: SourcedValue<String>,
    /// Authored geometry selectors and settings.
    pub geometry: GeologyGeometry,
    /// Authored placement settings.
    pub placement: GeologyPlacement,
    /// Authored substrate settings.
    pub substrate: GeologySubstrate,
    /// The linked Entity class Record header candidates and resolution state.
    pub entity: LooseRecordLink,
    /// The directory lookups for the authored geometry and material paths.
    pub references: GeologyReferences,
    /// The original mineable configuration geometry path.
    pub mineable_config_path: SourcedValue<String>,
    /// The original mineable material path.
    pub mineable_material_path: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    /// The authored harvestable model tag.
    pub harvestable_model_tag: SourcedValue<String>,
    /// The count of original resource declarations.
    pub resources_count: SourcedValue<DecimalU64>,
    #[schemars(extend("x-gameText" = true))]
    /// The original resource tokens in physical slot order.
    pub resources: SourcedValue<Vec<SourcedValue<String>>>,
    #[schemars(extend("x-gameText" = true))]
    /// The authored procedural audio selector.
    pub procedural_audio: SourcedValue<String>,
    /// The authored procedural audio offset.
    pub audio_offset: SourcedValue<Vec<SourcedValue<CatalogJsonNumber>>>,
    /// Additional unprojected original fields.
    pub additional_fields: Vec<RawPlanetAssetMember>,
    /// Source issues observed in this owner selection.
    pub issues: Vec<DefinitionIssue>,
}

impl LooseDefinition for PlanetGeologyAsset {
    fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }
    fn issues(&mut self) -> &mut Vec<DefinitionIssue> {
        &mut self.issues
    }
}

pub(crate) fn read(
    archive: &Archive,
    original: &OriginalCatalogInput<'_, '_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
) -> Result<CatalogTable<PlanetGeologyAsset>, CatalogError> {
    read_selected(
        archive,
        capture,
        ledger,
        options,
        Selection::All,
        &mut |guid, selector, expected| {
            loose_record_link::resolve_guid(
                guid,
                selector,
                original,
                expected,
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
    lookup: &mut GuidRecordLookup<'_>,
) -> Result<CatalogTable<PlanetGeologyAsset>, CatalogError> {
    let mut resolver = Resolver::new(archive, capture, options.cancel);
    loose_definition::read_selected(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        selection,
        |identity, object| {
            project(
                identity,
                &object.retaining_unprojected(),
                capture.source_uses(),
                lookup,
                &mut resolver,
            )
        },
    )
}

fn project(
    identity: PhysicalEntryIdentity,
    object: &EntryJsonObject<'_, '_>,
    sources: &SourceUses,
    lookup: &mut GuidRecordLookup<'_>,
    resolver: &mut Resolver<'_, '_, '_>,
) -> Result<PlanetGeologyAsset, CatalogError> {
    let guid = object.field("GUID", entry_json_object::guid)?;
    let geometry = GeologyGeometry::read(object)?;
    let entity = loose_record_link::by_guid_using(
        sources,
        &geometry.entity_class,
        &geometry.entity_class_raw,
        "EntityClassDefinition",
        lookup,
    )?;
    let mut issues = declaration_issues(&guid, &entity);
    let mineable_config_path = object.field("Mineable_cfg_path", entry_json_object::text)?;
    let mineable_material_path = object.field("Mineable_Material_path", entry_json_object::text)?;
    let references = references(
        resolver,
        &geometry,
        &mineable_config_path,
        &mineable_material_path,
        &mut issues,
    )?;
    let resources_count = object.field("resources", entry_json_object::array_len)?;
    if resources_count
        .value()
        .is_some_and(|count| count.get() != 0)
    {
        issues.push(DefinitionIssue {
            field: "resources",
            code: crate::export_diagnostic::ExportDiagnosticCode::UnsupportedValue,
        });
    }
    Ok(PlanetGeologyAsset {
        identity,
        guid,
        guid_raw: object.field("GUID", entry_json_object::text)?,
        geometry,
        placement: GeologyPlacement::read(object)?,
        substrate: GeologySubstrate::read(object)?,
        entity,
        references,
        mineable_config_path,
        mineable_material_path,
        harvestable_model_tag: object.field("harvestableModelTag", entry_json_object::text)?,
        resources_count,
        resources: object.array("resources", planet_asset_raw_fields::token)?,
        procedural_audio: object.field("sProceduralAudio", entry_json_object::text)?,
        audio_offset: object.array("vAudioOffset", entry_json_object::number)?,
        additional_fields: planet_asset_raw_fields::collect(object)?,
        issues,
    })
}

fn references(
    resolver: &mut Resolver<'_, '_, '_>,
    geometry: &GeologyGeometry,
    mineable_config_path: &SourcedValue<String>,
    mineable_material_path: &SourcedValue<String>,
    issues: &mut Vec<DefinitionIssue>,
) -> Result<GeologyReferences, CatalogError> {
    let references = GeologyReferences {
        geometry: resolver.resolve(&geometry.geometry_path, planet_geology_references::geometry)?,
        material: resolver.resolve(&geometry.material_path, planet_geology_references::material)?,
        mineable_config: resolver
            .resolve(mineable_config_path, planet_geology_references::geometry)?,
        mineable_material: resolver
            .resolve(mineable_material_path, planet_geology_references::material)?,
    };
    for (field, reference) in [
        ("geomPath", &references.geometry),
        ("matPath", &references.material),
        ("Mineable_cfg_path", &references.mineable_config),
        ("Mineable_Material_path", &references.mineable_material),
    ] {
        if let Some(code) = reference
            .state
            .value()
            .copied()
            .and_then(loose_definition::reference_issue)
        {
            issues.push(DefinitionIssue { field, code });
        }
    }
    Ok(references)
}

fn declaration_issues(
    guid: &SourcedValue<CatalogGuid>,
    entity: &LooseRecordLink,
) -> Vec<DefinitionIssue> {
    let mut issues: Vec<_> = loose_definition::identity_issue("GUID", guid)
        .into_iter()
        .collect();
    if let Some(code) = entity
        .state
        .value()
        .copied()
        .and_then(loose_definition::reference_issue)
    {
        issues.push(DefinitionIssue {
            field: "entityClass",
            code,
        });
    }
    issues
}
