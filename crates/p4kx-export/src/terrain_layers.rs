// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DecimalU64, SourceValue, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_asset_raw_fields::{self, RawPlanetAssetMember};
use crate::CatalogError;
use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_archive_path::{self, LooseArchivePath};
use crate::catalog_read::loose_definition::{self, DefinitionIssue};
use crate::catalog_read::parsed_entry_json::{self, SourceNode, SourceNodeValue};
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::schema_fields::every_field_required;

pub(super) type Number = SourcedValue<CatalogJsonNumber>;
pub(super) type NumericSlots = SourcedValue<Vec<Number>>;
pub(super) type GroundLayers = SourcedValue<Vec<SourcedValue<GroundLayer>>>;
pub(super) type TextureLayers = SourcedValue<Vec<SourcedValue<TextureLayer>>>;

const TEXTURE: &str = "textureName";

/// Original terrain material selectors and scalar flags at one layer occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct GroundLayer {
    /// The authored texture selector.
    pub texture_name: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    /// The authored surface type.
    pub surface_type_name: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    /// The authored surface group.
    pub surface_group_name: SourcedValue<String>,
    /// The authored integer gloss value.
    pub gloss: Number,
    /// The authored bedrock flag.
    pub bedrock: SourcedValue<bool>,
    /// Additional unprojected original fields.
    pub additional_fields: Vec<RawPlanetAssetMember>,
}

/// An original terrain texture layer.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct TextureLayer {
    /// The authored texture selector.
    pub texture_name: SourcedValue<String>,
    /// Additional unprojected original fields.
    pub additional_fields: Vec<RawPlanetAssetMember>,
}

/// Directory lookups for the paths of the corresponding source layer slots.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LayerTextureLinks {
    /// Resolved directory links in authored array slot order.
    pub slots: Vec<LayerTextureLink>,
}

/// The directory lookup of one authored terrain texture slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LayerTextureLink {
    /// The physical position in the source layer array.
    pub ordinal: DecimalU64,
    /// The original texture path lookup outcome.
    pub target: LooseArchivePath,
}

pub(super) struct LayerLinks {
    pub(super) ground: Option<LayerTextureLinks>,
    pub(super) additional: Option<LayerTextureLinks>,
}

#[derive(Clone, Copy)]
pub(super) struct TextureLayerFieldNames {
    pub(super) array: &'static str,
    pub(super) texture: &'static str,
}

pub(super) struct LayerResolver<'resolver, 'archive, 'capture, 'input> {
    resolver: &'resolver mut loose_archive_path::Resolver<'archive, 'capture, 'input>,
    cancel: Option<&'resolver CancelToken>,
}

impl GroundLayer {
    pub(super) fn read(object: &EntryJsonObject<'_, '_>) -> Result<Self, CatalogError> {
        let object = &object.retaining_unprojected();
        Ok(Self {
            texture_name: object.field(TEXTURE, entry_json_object::text)?,
            surface_type_name: object.field("surfaceTypeName", entry_json_object::text)?,
            surface_group_name: object.field("surfaceGroupName", entry_json_object::text)?,
            gloss: object.field("gloss", integer)?,
            bedrock: object.field("bIsBedrock", entry_json_object::boolean)?,
            additional_fields: planet_asset_raw_fields::collect(object)?,
        })
    }

    pub(super) const fn texture(&self) -> &SourcedValue<String> {
        &self.texture_name
    }

    pub(super) fn field_issues(
        &self,
        labels: GroundLayerFieldNames,
        issues: &mut Vec<DefinitionIssue>,
    ) {
        note_invalid(
            issues,
            [
                (labels.surface_type, self.surface_type_name.state()),
                (labels.surface_group, self.surface_group_name.state()),
                (labels.gloss, self.gloss.state()),
                (labels.bedrock, self.bedrock.state()),
            ],
        );
    }
}

#[derive(Clone, Copy)]
pub(super) struct GroundLayerFieldNames {
    pub(super) texture: &'static str,
    pub(super) surface_type: &'static str,
    pub(super) surface_group: &'static str,
    pub(super) gloss: &'static str,
    pub(super) bedrock: &'static str,
}

impl TextureLayer {
    pub(super) fn read(object: &EntryJsonObject<'_, '_>) -> Result<Self, CatalogError> {
        let object = &object.retaining_unprojected();
        Ok(Self {
            texture_name: object.field(TEXTURE, entry_json_object::text)?,
            additional_fields: planet_asset_raw_fields::collect(object)?,
        })
    }

    pub(super) const fn texture(&self) -> &SourcedValue<String> {
        &self.texture_name
    }
}

impl<'resolver, 'archive, 'capture, 'input> LayerResolver<'resolver, 'archive, 'capture, 'input> {
    pub(super) const fn new(
        resolver: &'resolver mut loose_archive_path::Resolver<'archive, 'capture, 'input>,
        cancel: Option<&'resolver CancelToken>,
    ) -> Self {
        Self { resolver, cancel }
    }

    pub(super) fn resolve(
        &mut self,
        path: &SourcedValue<String>,
        field: &'static str,
        issues: &mut Vec<DefinitionIssue>,
    ) -> Result<LooseArchivePath, CatalogError> {
        let target = self.resolver.resolve(path, |_| true)?;
        if let Some(code) = target
            .state
            .value()
            .copied()
            .and_then(loose_definition::reference_issue)
        {
            let issue = DefinitionIssue { field, code };
            parsed_entry_json::push(issues, issue)?;
        }
        Ok(target)
    }

    pub(super) fn layers(
        &mut self,
        ground: &GroundLayers,
        additional: &TextureLayers,
        ground_names: GroundLayerFieldNames,
        additional_names: TextureLayerFieldNames,
        issues: &mut Vec<DefinitionIssue>,
    ) -> Result<LayerLinks, CatalogError> {
        note_invalid_array(issues, "groundLayers", ground);
        note_invalid_array(issues, additional_names.array, additional);
        note_ground_issues(ground, ground_names, issues);
        Ok(LayerLinks {
            ground: self.array(ground, GroundLayer::texture, ground_names.texture, issues)?,
            additional: self.array(
                additional,
                TextureLayer::texture,
                additional_names.texture,
                issues,
            )?,
        })
    }

    pub(super) fn array<T>(
        &mut self,
        items: &SourcedValue<Vec<SourcedValue<T>>>,
        path: impl Fn(&T) -> &SourcedValue<String>,
        field: &'static str,
        issues: &mut Vec<DefinitionIssue>,
    ) -> Result<Option<LayerTextureLinks>, CatalogError> {
        let Some(items) = items.value() else {
            return Ok(None);
        };
        let mut slots = Vec::new();
        for (index, item) in items.iter().enumerate() {
            parsed_entry_json::check_cancel(self.cancel)?;
            let Some(item) = item.value() else {
                continue;
            };
            let ordinal =
                u64::try_from(index).map_err(|_| crate::ExportAccountingError::CounterOverflow)?;
            let target = self.resolve(path(item), field, issues)?;
            parsed_entry_json::push(
                &mut slots,
                LayerTextureLink {
                    ordinal: DecimalU64::new(ordinal),
                    target,
                },
            )?;
        }
        Ok(Some(LayerTextureLinks { slots }))
    }
}

pub(super) fn integer(node: &SourceNode<'_>) -> SourceValue<CatalogJsonNumber> {
    match node.value {
        SourceNodeValue::Number(token) if !token.contains(['.', 'e', 'E']) => {
            entry_json_object::number(node)
        }
        _ => SourceValue::Invalid,
    }
}

pub(super) fn note_ground_issues(
    ground: &GroundLayers,
    labels: GroundLayerFieldNames,
    issues: &mut Vec<DefinitionIssue>,
) {
    if let Some(layers) = ground.value() {
        for layer in layers.iter().filter_map(SourcedValue::value) {
            layer.field_issues(labels, issues);
        }
    }
}

pub(super) fn note_invalid_array<T>(
    issues: &mut Vec<DefinitionIssue>,
    field: &'static str,
    value: &SourcedValue<Vec<SourcedValue<T>>>,
) {
    note_invalid(issues, [(field, value.state())]);
    if let Some(items) = value.value() {
        note_invalid(issues, items.iter().map(|slot| (field, slot.state())));
    }
}

pub(super) fn read_field<'read, T>(
    object: &EntryJsonObject<'_, 'read>,
    name: &'static str,
    project: impl FnOnce(&SourceNode<'read>) -> SourceValue<T>,
    issues: &mut Vec<DefinitionIssue>,
) -> Result<SourcedValue<T>, CatalogError> {
    let value = object.field(name, project)?;
    note_invalid(issues, [(name, value.state())]);
    Ok(value)
}

pub(super) fn note_invalid(
    issues: &mut Vec<DefinitionIssue>,
    fields: impl IntoIterator<Item = (&'static str, SourceValueState)>,
) {
    issues.extend(fields.into_iter().filter_map(|(field, state)| {
        (state == SourceValueState::Invalid).then_some(DefinitionIssue {
            field,
            code: ExportDiagnosticCode::MalformedSource,
        })
    }));
}
