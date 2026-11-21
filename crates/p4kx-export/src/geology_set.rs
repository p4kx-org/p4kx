// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DecimalU64, Guid, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_asset_raw_fields::{self, RawPlanetAssetMember};
use super::terrain_guid_link::{self, TerrainGuidIndex, TerrainGuidLink};
use super::terrain_layers;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition, Selection,
};
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_read::parsed_entry_json;
use crate::catalog_table::CatalogTable;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

const KIND: Kind = Kind::GeologySets;
pub(super) const FAMILY: DefinitionFamily = DefinitionFamily::new(
    KIND,
    "data/libs/planets/geology_sets/",
    "geology",
    "GeologySet",
    "GUID",
);

pub(crate) type GeologySetTable = CatalogTable<GeologySet>;
type GuidSlots = SourcedValue<Vec<SourcedValue<CatalogGuid>>>;

/// Original geology asset selections, probabilities and texture-set declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct GeologySet {
    /// The physical Entry that owns this definition.
    pub identity: PhysicalEntryIdentity,
    /// The parsed owner GUID.
    pub guid: SourcedValue<CatalogGuid>,
    /// The original owner GUID text.
    pub guid_raw: SourcedValue<String>,
    /// The ordered authored scattered asset GUID slots.
    pub scattered_assets: GuidSlots,
    /// The original scattered asset GUID text at each slot.
    pub scattered_assets_raw: SourcedValue<Vec<SourcedValue<String>>>,
    /// The ordered authored cliff asset GUID slots.
    pub cliff_assets: GuidSlots,
    /// The original cliff asset GUID text at each slot.
    pub cliff_assets_raw: SourcedValue<Vec<SourcedValue<String>>>,
    /// The authored probabilities by size slot.
    pub size_probabilities: terrain_layers::NumericSlots,
    /// The authored asset counts by size slot.
    pub size_number_of_assets: terrain_layers::NumericSlots,
    /// The authored cliff probability.
    pub cliff_probability: terrain_layers::Number,
    /// The authored geology texture-set GUID.
    pub geology_texture_set_guid: SourcedValue<CatalogGuid>,
    /// The original geology texture-set GUID text.
    pub geology_texture_set_guid_raw: SourcedValue<String>,
    /// The selected terrain GUID links in the original slot order.
    pub links: Option<GeologySetLinks>,
    /// Additional unprojected original fields.
    pub additional_fields: Vec<RawPlanetAssetMember>,
    /// Source issues observed in this owner selection.
    pub issues: Vec<DefinitionIssue>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original asset and texture-set GUID relations declared by a geology set.
#[non_exhaustive]
pub struct GeologySetLinks {
    /// Candidate evidence for the authored scattered asset slots.
    pub scattered_assets: Option<Vec<GeologyAssetSlot>>,
    /// Candidate evidence for the authored cliff asset slots.
    pub cliff_assets: Option<Vec<GeologyAssetSlot>>,
    /// Candidate evidence for the authored geology texture set.
    pub geology_texture_set: TerrainGuidLink,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
/// A positioned asset GUID relation in a geology set.
#[non_exhaustive]
pub struct GeologyAssetSlot {
    /// The physical position in the original array.
    pub ordinal: DecimalU64,
    /// The GUID resolution outcome for this slot.
    pub link: TerrainGuidLink,
}

impl LooseDefinition for GeologySet {
    fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }

    fn issues(&mut self) -> &mut Vec<DefinitionIssue> {
        &mut self.issues
    }
}

pub(crate) fn read(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
) -> Result<GeologySetTable, CatalogError> {
    read_selected(archive, capture, ledger, options, Selection::All)
}

pub(crate) fn read_selected(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    selection: Selection<'_>,
) -> Result<GeologySetTable, CatalogError> {
    loose_definition::read_selected(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        selection,
        GeologySet::project,
    )
}

impl GeologySet {
    fn project(
        identity: PhysicalEntryIdentity,
        object: &EntryJsonObject<'_, '_>,
    ) -> Result<Self, CatalogError> {
        let object = &object.retaining_unprojected();
        let guid = object.field("GUID", entry_json_object::guid)?;
        let guid_raw = object.field("GUID", entry_json_object::text)?;
        let scattered_assets = object.array("scatteredAssets", entry_json_object::guid)?;
        let scattered_assets_raw = object.array("scatteredAssets", entry_json_object::text)?;
        let cliff_assets = object.array("cliffAssets", entry_json_object::guid)?;
        let cliff_assets_raw = object.array("cliffAssets", entry_json_object::text)?;
        let size_probabilities = object.array("sizeProbabilities", entry_json_object::number)?;
        let size_number_of_assets = object.array("sizeNumberOfAssets", terrain_layers::integer)?;
        let cliff_probability = object.field("cliffProbability", entry_json_object::number)?;
        let geology_texture_set_guid =
            object.field("geologyTextureSetGUID", entry_json_object::guid)?;
        let geology_texture_set_guid_raw =
            object.field("geologyTextureSetGUID", entry_json_object::text)?;
        let mut issues: Vec<_> = loose_definition::identity_issue("GUID", &guid)
            .into_iter()
            .collect();
        terrain_layers::note_invalid(
            &mut issues,
            [
                ("cliffProbability", cliff_probability.state()),
                ("geologyTextureSetGUID", geology_texture_set_guid.state()),
            ],
        );
        terrain_layers::note_invalid_array(&mut issues, "scatteredAssets", &scattered_assets);
        terrain_layers::note_invalid_array(&mut issues, "cliffAssets", &cliff_assets);
        terrain_layers::note_invalid_array(&mut issues, "sizeProbabilities", &size_probabilities);
        terrain_layers::note_invalid_array(
            &mut issues,
            "sizeNumberOfAssets",
            &size_number_of_assets,
        );
        Ok(Self {
            identity,
            guid,
            guid_raw,
            scattered_assets,
            scattered_assets_raw,
            cliff_assets,
            cliff_assets_raw,
            size_probabilities,
            size_number_of_assets,
            cliff_probability,
            geology_texture_set_guid,
            geology_texture_set_guid_raw,
            links: None,
            additional_fields: planet_asset_raw_fields::collect(object)?,
            issues,
        })
    }

    pub(crate) fn required_guids(&self) -> impl Iterator<Item = CatalogGuid> + '_ {
        self.scattered_assets
            .value()
            .into_iter()
            .flatten()
            .chain(self.cliff_assets.value().into_iter().flatten())
            .chain(std::iter::once(&self.geology_texture_set_guid))
            .filter_map(SourcedValue::value)
            .copied()
            .filter(|guid| guid.0 != Guid::default())
    }

    pub(crate) fn link(
        &mut self,
        index: &TerrainGuidIndex,
        capture: &EntryCapture<'_>,
        ledger: &mut ExportLedger,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let scattered_assets =
            Self::link_asset_slots(index, capture, &self.scattered_assets, cancel)?;
        let cliff_assets = Self::link_asset_slots(index, capture, &self.cliff_assets, cancel)?;
        let geology_texture_set = index.resolve(
            Kind::GeologyTextureSets,
            &self.geology_texture_set_guid,
            capture,
        )?;
        for (field, slots) in [
            ("scatteredAssets[]", &scattered_assets),
            ("cliffAssets[]", &cliff_assets),
        ] {
            for slot in slots.iter().flatten() {
                self.note_link(field, &slot.link, ledger)?;
            }
        }
        self.note_link("geologyTextureSetGUID", &geology_texture_set, ledger)?;
        self.links = Some(GeologySetLinks {
            scattered_assets,
            cliff_assets,
            geology_texture_set,
        });
        Ok(())
    }

    fn note_link(
        &mut self,
        field: &'static str,
        link: &TerrainGuidLink,
        ledger: &mut ExportLedger,
    ) -> Result<(), CatalogError> {
        if let Some(code) = link
            .state
            .value()
            .copied()
            .and_then(terrain_guid_link::link_issue)
        {
            let issue = DefinitionIssue { field, code };
            issue.record(ledger, KIND);
            parsed_entry_json::push(&mut self.issues, issue)?;
        }
        Ok(())
    }

    fn link_asset_slots(
        index: &TerrainGuidIndex,
        capture: &EntryCapture<'_>,
        slots: &GuidSlots,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Vec<GeologyAssetSlot>>, CatalogError> {
        let Some(slots) = slots.value() else {
            return Ok(None);
        };
        let mut output = Vec::new();
        for (ordinal, slot) in slots.iter().enumerate() {
            parsed_entry_json::check_cancel(cancel)?;
            let ordinal = u64::try_from(ordinal)
                .map_err(|_| crate::ExportAccountingError::CounterOverflow)?;
            let link = index.resolve(Kind::GeologyAssets, slot, capture)?;
            parsed_entry_json::push(
                &mut output,
                GeologyAssetSlot {
                    ordinal: DecimalU64::new(ordinal),
                    link,
                },
            )?;
        }
        Ok(Some(output))
    }
}
