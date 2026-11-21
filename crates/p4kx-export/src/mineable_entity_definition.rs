// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::prepared_definition::PreparedDefinition;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::harvest_link::MINEABLE_PARAMS;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::scan_signature::SignatureSlot;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

const SIGNATURE_PARAMS: &str = "SSCSignatureSystemParams";

/// One physical mineable entity and its authored mining and radar components.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct MineableEntity<'source> {
    /// The original physical Record and its identity.
    #[serde(flatten)]
    pub source: CanonicalRecordHeader<'source>,
    /// The original number of component slots.
    pub component_slots: SourcedValue<u32>,
    /// Mining components in their original relative order.
    pub mining: Vec<MineableParameters<'source>>,
    /// Signature components in their original relative order.
    pub radar: Vec<MineableSignature<'source>>,
}

impl MineableEntity<'_> {
    pub(crate) fn has_signatures(&self) -> bool {
        self.radar.iter().any(|component| {
            component
                .properties
                .value()
                .and_then(|properties| properties.base_signature_params.value())
                .and_then(|base| base.signatures.value())
                .is_some_and(|slots| !slots.is_empty())
        })
    }
}

/// One authored `MineableParams` component and its links.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct MineableParameters<'source> {
    /// The original concrete type.
    pub source_type: SourcedValue<&'source str>,
    /// The ordinal in the Entity's Components array.
    pub source_position: SourcedValue<u32>,
    /// The authored `MineableComposition` link.
    pub composition: SourcedValue<CatalogRecordReference>,
    /// The authored `MiningGlobalParams` link.
    pub global_params: SourcedValue<CatalogRecordReference>,
    /// Authored declarations outside the typed projection.
    pub authored_fields: SourceArenaOutput<'source>,
}

impl<'source> MineableParameters<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let composition = fields
            .record_link("composition", notes)?
            .map(|link| (*link).into_catalog());
        let global_params = fields
            .record_link("globalParams", notes)?
            .map(|link| (*link).into_catalog());
        let authored_fields = fields.authored_members_except([
            ("composition", composition.state()),
            ("globalParams", global_params.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            source_position: fields.array_ordinal()?,
            composition,
            global_params,
            authored_fields,
        })
    }
}

/// One authored signature component of a mineable entity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct MineableSignature<'source> {
    /// The original concrete type.
    pub source_type: SourcedValue<&'source str>,
    /// The ordinal in the Entity's Components array.
    pub source_position: SourcedValue<u32>,
    /// The authored radar properties.
    pub properties: SourcedValue<MineableRadarProperties<'source>>,
    /// Authored declarations outside the typed projection.
    pub authored_fields: SourceArenaOutput<'source>,
}

impl<'source> MineableSignature<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
        slot: &SignatureSlot,
    ) -> Result<Self, CatalogError> {
        let properties = fields.declared_owned("radarProperties", notes, |fields, notes| {
            MineableRadarProperties::read(fields, notes, slot)
        })?;
        let authored_fields =
            fields.authored_members_except([("radarProperties", properties.state())])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            source_position: fields.array_ordinal()?,
            properties,
            authored_fields,
        })
    }
}

/// Original radar contact and base signature declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct MineableRadarProperties<'source> {
    /// The original concrete type.
    pub source_type: SourcedValue<&'source str>,
    /// The authored radar contact type link.
    pub contact_type: SourcedValue<CatalogRecordReference>,
    /// The authored base signature parameters.
    pub base_signature_params: SourcedValue<MineableBaseSignature<'source>>,
    /// Remaining radar declarations and values.
    pub authored_fields: SourceArenaOutput<'source>,
}

impl<'source> MineableRadarProperties<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
        slot: &SignatureSlot,
    ) -> Result<Self, CatalogError> {
        let contact_type = fields
            .record_link("contactType", notes)?
            .map(|link| (*link).into_catalog());
        let base_signature_params =
            fields.declared_owned("baseSignatureParams", notes, |fields, notes| {
                MineableBaseSignature::read(fields, notes, slot)
            })?;
        let authored_fields = fields.authored_members_except([
            ("contactType", contact_type.state()),
            ("baseSignatureParams", base_signature_params.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            contact_type,
            base_signature_params,
            authored_fields,
        })
    }
}

/// Ordered original signature slots and their finite scan value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct MineableBaseSignature<'source> {
    /// The original concrete type.
    pub source_type: SourcedValue<&'source str>,
    /// Original values in slot order, with each Single's exact bits.
    pub signatures: SourcedValue<Vec<SourcedValue<ResourceFloat32>>>,
    /// Finite scan value selected by the original global radar declaration.
    pub scan_signature: SourcedValue<f32>,
    /// Authored declarations outside the typed projection.
    pub authored_fields: SourceArenaOutput<'source>,
}

impl<'source> MineableBaseSignature<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
        slot: &SignatureSlot,
    ) -> Result<Self, CatalogError> {
        let signatures = fields.single_array("signatures", notes)?;
        let scan_signature = slot.project(&signatures, fields.sources())?;
        let authored_fields =
            fields.authored_members_except([("signatures", signatures.state())])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            signatures,
            scan_signature,
            authored_fields,
        })
    }
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    slot: &SignatureSlot,
    cancel: Option<&CancelToken>,
) -> Result<(MineableEntity<'source>, Vec<FieldError>), CatalogError> {
    let (prepared, notes) = PreparedDefinition::read_original(
        tree,
        projection.original(),
        admission,
        cancel,
        |source, fields, notes| {
            let mut mining = Vec::new();
            let mut radar = Vec::new();
            let component_slots = fields.visit_components(notes, |kind, fields, notes| {
                match kind {
                    MINEABLE_PARAMS => mining.push(MineableParameters::read(fields, notes)?),
                    SIGNATURE_PARAMS => radar.push(MineableSignature::read(fields, notes, slot)?),
                    _ => {}
                }
                Ok(())
            })?;
            Ok(MineableEntity {
                source,
                component_slots,
                mining,
                radar,
            })
        },
    )?;
    Ok((prepared.into_definition(), notes))
}

pub(crate) fn needs_signature_slot(tree: &RecordTree<'_>) -> bool {
    crate::record_fields::elements(Some(tree.root()), "Components")
        .iter()
        .any(|value| {
            matches!(value, p4kx_extraction::Value::Struct(component)
                if component.type_name() == SIGNATURE_PARAMS)
        })
}
