// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DecimalU64, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_asset_raw_fields::{self, RawPlanetAssetMember};
use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_archive_path::{self, LooseArchivePath};
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition, Selection,
};
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_read::parsed_entry_json;
use crate::catalog_table::CatalogTable;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

const KIND: Kind = Kind::Ecosystems;
pub(super) const FAMILY: DefinitionFamily =
    DefinitionFamily::new(KIND, "data/libs/ecosystems/", "eco", "Ecosystem", GUID);
const GUID: &str = "GUID";
const SOUND_EFFECT: &str = "SoundEffect";
const TEXTURES: &str = "Textures";
const AUDIO_RTPC_ARRAY: &str = "audioRTPCArray";
const AUDIO_SWITCH_ARRAY: &str = "audioSwitchArray";
const AUDIO_TRIGGER: &str = "audioTrigger";
const AUDIO_ECCENTRICITY: &str = "fAudioTerrainEccentricity";

pub(crate) type EcosystemTable = CatalogTable<Ecosystem>;

/// One physical ecosystem definition and its authored audio and texture fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct Ecosystem {
    /// The physical original Entry.
    pub identity: PhysicalEntryIdentity,
    /// The original parsed GUID.
    pub guid: SourcedValue<CatalogGuid>,
    /// The authored GUID spelling.
    pub guid_raw: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    /// The original protected ecosystem name.
    pub name: SourcedValue<String>,
    /// Original sound settings and observed array counts.
    pub sound_effect: SourcedValue<EcosystemSoundEffect>,
    /// Original texture and mask selector fields.
    pub textures: SourcedValue<EcosystemTextures>,
    /// Directory lookups for the original texture selectors.
    pub texture_links: Option<EcosystemTextureLinks>,
    /// Additional original JSON members and their exact tokens.
    pub additional_fields: Vec<RawPlanetAssetMember>,
    /// Source issues observed in the selected owner scope.
    pub issues: Vec<DefinitionIssue>,
}

/// Original settings of an ecosystem sound declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct EcosystemSoundEffect {
    /// The original number of audio RTPC entries.
    pub audio_rtpc_array_count: SourcedValue<DecimalU64>,
    /// The original number of audio switch entries.
    pub audio_switch_array_count: SourcedValue<DecimalU64>,
    /// The original audio trigger text.
    pub audio_trigger: SourcedValue<String>,
    /// The exact authored terrain eccentricity.
    pub audio_terrain_eccentricity: SourcedValue<CatalogJsonNumber>,
    /// Additional original JSON members and their exact tokens.
    pub additional_fields: Vec<RawPlanetAssetMember>,
}

/// Original texture and mask selectors of an ecosystem.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct EcosystemTextures {
    /// The original climate texture path.
    pub climate: SourcedValue<String>,
    /// The original data texture path.
    pub data: SourcedValue<String>,
    /// The original normal texture path.
    pub normal: SourcedValue<String>,
    /// The original elevation texture path.
    pub elevation: SourcedValue<String>,
    /// The original object preset spawn mask path.
    pub object_preset_spawn_mask: SourcedValue<String>,
    /// Additional original JSON members and their exact tokens.
    pub additional_fields: Vec<RawPlanetAssetMember>,
}

/// Directory evidence for the original ecosystem selectors.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct EcosystemTextureLinks {
    /// The original climate texture lookup.
    pub climate: LooseArchivePath,
    /// The original data texture lookup.
    pub data: LooseArchivePath,
    /// The original normal texture lookup.
    pub normal: LooseArchivePath,
    /// The original elevation texture lookup.
    pub elevation: LooseArchivePath,
    /// The original object preset spawn mask lookup.
    pub object_preset_spawn_mask: LooseArchivePath,
}

impl LooseDefinition for Ecosystem {
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
    paths: &mut loose_archive_path::Resolver<'_, '_, '_>,
) -> Result<EcosystemTable, CatalogError> {
    read_selected(archive, capture, ledger, options, paths, Selection::All)
}

pub(crate) fn read_selected(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    paths: &mut loose_archive_path::Resolver<'_, '_, '_>,
    selection: Selection<'_>,
) -> Result<EcosystemTable, CatalogError> {
    let mut table = loose_definition::read_selected(
        archive, capture, ledger, options, FAMILY, selection, project,
    )?;
    let mut resolver = TextureResolver { paths, ledger };
    for row in &mut table.rows {
        parsed_entry_json::check_cancel(options.cancel)?;
        if let Some(textures) = row.textures.value() {
            row.texture_links = Some(resolver.resolve(textures, &mut row.issues)?);
        }
    }
    Ok(table)
}

fn project(
    identity: PhysicalEntryIdentity,
    object: &EntryJsonObject<'_, '_>,
) -> Result<Ecosystem, CatalogError> {
    let object = &object.retaining_unprojected();
    let guid = object.field(GUID, entry_json_object::guid)?;
    let guid_raw = object.field(GUID, entry_json_object::text)?;
    let name = object.field("name", entry_json_object::text)?;
    let sound_effect = object.object(SOUND_EFFECT, EcosystemSoundEffect::read)?;
    let textures = object.object(TEXTURES, EcosystemTextures::read)?;
    let mut issues: Vec<_> = loose_definition::identity_issue(GUID, &guid)
        .into_iter()
        .collect();
    malformed_fields(
        &mut issues,
        [
            ("name", name.state()),
            (SOUND_EFFECT, sound_effect.state()),
            (TEXTURES, textures.state()),
        ],
    );
    if let Some(sound) = sound_effect.value() {
        sound.diagnostics(&mut issues);
    }
    Ok(Ecosystem {
        identity,
        guid,
        guid_raw,
        name,
        sound_effect,
        textures,
        texture_links: None,
        additional_fields: planet_asset_raw_fields::collect(object)?,
        issues,
    })
}

impl EcosystemSoundEffect {
    fn read(object: &EntryJsonObject<'_, '_>) -> Result<Self, CatalogError> {
        let object = &object.retaining_unprojected();
        Ok(Self {
            audio_rtpc_array_count: object.field(AUDIO_RTPC_ARRAY, entry_json_object::array_len)?,
            audio_switch_array_count: object
                .field(AUDIO_SWITCH_ARRAY, entry_json_object::array_len)?,
            audio_trigger: object.field(AUDIO_TRIGGER, entry_json_object::text)?,
            audio_terrain_eccentricity: object
                .field(AUDIO_ECCENTRICITY, entry_json_object::number)?,
            additional_fields: planet_asset_raw_fields::collect(object)?,
        })
    }

    fn diagnostics(&self, issues: &mut Vec<DefinitionIssue>) {
        malformed_fields(
            issues,
            [
                ("SoundEffect.audioTrigger", self.audio_trigger.state()),
                (
                    "SoundEffect.fAudioTerrainEccentricity",
                    self.audio_terrain_eccentricity.state(),
                ),
            ],
        );
        for (field, count) in [
            ("SoundEffect.audioRTPCArray", &self.audio_rtpc_array_count),
            (
                "SoundEffect.audioSwitchArray",
                &self.audio_switch_array_count,
            ),
        ] {
            let code = if count.state() == SourceValueState::Invalid {
                ExportDiagnosticCode::MalformedSource
            } else if count.value().is_some_and(|count| count.get() != 0) {
                ExportDiagnosticCode::UnsupportedValue
            } else {
                continue;
            };
            issues.push(DefinitionIssue { field, code });
        }
    }
}

impl EcosystemTextures {
    fn read(object: &EntryJsonObject<'_, '_>) -> Result<Self, CatalogError> {
        let object = &object.retaining_unprojected();
        Ok(Self {
            climate: object.field("ecoSystemClimateTexture", entry_json_object::text)?,
            data: object.field("ecoSystemDataTexture", entry_json_object::text)?,
            normal: object.field("ecoSystemNormalTexture", entry_json_object::text)?,
            elevation: object.field("elevationTexture", entry_json_object::text)?,
            object_preset_spawn_mask: object
                .field("ObjectPresetSpawnMask", entry_json_object::text)?,
            additional_fields: planet_asset_raw_fields::collect(object)?,
        })
    }
}

struct TextureResolver<'resolver, 'archive, 'capture, 'input> {
    paths: &'resolver mut loose_archive_path::Resolver<'archive, 'capture, 'input>,
    ledger: &'resolver mut ExportLedger,
}

impl TextureResolver<'_, '_, '_, '_> {
    fn resolve(
        &mut self,
        textures: &EcosystemTextures,
        issues: &mut Vec<DefinitionIssue>,
    ) -> Result<EcosystemTextureLinks, CatalogError> {
        Ok(EcosystemTextureLinks {
            climate: self.resolve_path(
                "Textures.ecoSystemClimateTexture",
                &textures.climate,
                issues,
            )?,
            data: self.resolve_path("Textures.ecoSystemDataTexture", &textures.data, issues)?,
            normal: self.resolve_path(
                "Textures.ecoSystemNormalTexture",
                &textures.normal,
                issues,
            )?,
            elevation: self.resolve_path(
                "Textures.elevationTexture",
                &textures.elevation,
                issues,
            )?,
            object_preset_spawn_mask: self.resolve_path(
                "Textures.ObjectPresetSpawnMask",
                &textures.object_preset_spawn_mask,
                issues,
            )?,
        })
    }

    fn resolve_path(
        &mut self,
        field: &'static str,
        path: &SourcedValue<String>,
        issues: &mut Vec<DefinitionIssue>,
    ) -> Result<LooseArchivePath, CatalogError> {
        let resolved = self.paths.resolve(path, |_| true)?;
        if let Some(code) = resolved
            .state
            .value()
            .copied()
            .and_then(loose_definition::reference_issue)
        {
            let issue = DefinitionIssue { field, code };
            issue.record(self.ledger, KIND);
            issues.push(issue);
        }
        Ok(resolved)
    }
}

fn malformed_fields(
    issues: &mut Vec<DefinitionIssue>,
    fields: impl IntoIterator<Item = (&'static str, SourceValueState)>,
) {
    for (field, state) in fields {
        if state == SourceValueState::Invalid {
            issues.push(DefinitionIssue {
                field,
                code: ExportDiagnosticCode::MalformedSource,
            });
        }
    }
}

#[cfg(test)]
#[path = "ecosystem_tests.rs"]
mod tests;
