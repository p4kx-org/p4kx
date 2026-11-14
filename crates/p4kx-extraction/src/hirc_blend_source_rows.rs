// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_audio::{
    BankBlendAssociation, BankBlendAssociations, BankBlendExtent, BankBlendLayer, BankBlendLayers,
    BankContainerChild, BankGraphPoint, BankHircContainerNode, BankHircField, BankRtpcExtent,
};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::source_evidence_error::SourceEvidenceError;
use crate::source_value::SourcedValue;

#[path = "hirc_blend_rtpc_rows.rs"]
mod rtpc;

use super::super::origins::ObjectOrigins;
use super::dto::{BlendReferenceState, BlendSourceRow, BlendSourceTable};

pub(super) struct BlendRows<'view, 'input, R> {
    container: RefCell<BankHircContainerNode<'input, R>>,
    origins: &'view ObjectOrigins,
}

impl<'view, 'input, R> BlendRows<'view, 'input, R> {
    pub(super) const fn new(
        container: BankHircContainerNode<'input, R>,
        origins: &'view ObjectOrigins,
    ) -> Self {
        Self {
            container: RefCell::new(container),
            origins,
        }
    }
}

impl<R: Read + Seek> Serialize for BlendRows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut container = self.container.try_borrow_mut().map_err(S::Error::custom)?;
        let mut sequence = serializer.serialize_seq(None)?;
        if let Some(children) = container.children() {
            for original in children {
                let original = original.map_err(S::Error::custom)?;
                let row = child(self.origins, &original).map_err(S::Error::custom)?;
                sequence.serialize_element(&row)?;
            }
        }
        if let Some(layers) = container.blend_layers() {
            serialize_layers(&mut sequence, self.origins, layers)?;
        }
        sequence.end()
    }
}

fn serialize_layers<R: Read + Seek, S: SerializeSeq>(
    sequence: &mut S,
    origins: &ObjectOrigins,
    mut layers: BankBlendLayers<'_, R>,
) -> Result<(), S::Error> {
    while let Some(mut layer) = layers.next_layer().map_err(S::Error::custom)? {
        let row = layer_fields(origins, &layer).map_err(S::Error::custom)?;
        sequence.serialize_element(&row)?;
        let layer_id_range = layer.fields.layer_id.range.clone();
        let owner = Occurrence::new(layer.ordinal, &layer_id_range);
        if let Some(curves) = layer.rtpc() {
            rtpc::serialize(sequence, origins, owner, curves)?;
        }
        if let Some(associations) = layer.associations() {
            serialize_associations(sequence, origins, owner, associations)?;
        }
    }
    Ok(())
}

fn serialize_associations<R: Read + Seek, S: SerializeSeq>(
    sequence: &mut S,
    origins: &ObjectOrigins,
    layer: Occurrence<'_>,
    mut associations: BankBlendAssociations<'_, R>,
) -> Result<(), S::Error> {
    while let Some(mut association) = associations.next_association().map_err(S::Error::custom)? {
        let row = association_fields(origins, layer, &association).map_err(S::Error::custom)?;
        sequence.serialize_element(&row)?;
        let child_id_range = association.fields.child_id.range.clone();
        let owner = Occurrence::new(association.ordinal, &child_id_range);
        if let Some(points) = association.points() {
            for point in points {
                let point = point.map_err(S::Error::custom)?;
                let row =
                    crossfade_point(origins, layer, owner, &point).map_err(S::Error::custom)?;
                sequence.serialize_element(&row)?;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Occurrence<'source> {
    ordinal: u32,
    id_range: &'source Range<u64>,
}

impl<'source> Occurrence<'source> {
    const fn new(ordinal: u32, id_range: &'source Range<u64>) -> Self {
        Self { ordinal, id_range }
    }

    fn source(
        self,
        origins: &ObjectOrigins,
        label: &'static str,
    ) -> Result<SourcedValue<u32>, SourceEvidenceError> {
        origins.derived(self.ordinal, self.id_range, label)
    }
}

pub(super) fn table(
    origins: &ObjectOrigins,
    extent: Option<&BankBlendExtent>,
    count_label: &'static str,
    range_label: &'static str,
) -> Result<Option<BlendSourceTable<u32>>, SourceEvidenceError> {
    extent
        .map(|extent| {
            table_fields(
                origins,
                &extent.declared_count,
                &extent.checked_range,
                extent.is_complete(),
                count_label,
                range_label,
            )
        })
        .transpose()
}

fn table_fields<T: Copy>(
    origins: &ObjectOrigins,
    count: &BankHircField<T>,
    range: &Range<u64>,
    complete: bool,
    count_label: &'static str,
    range_label: &'static str,
) -> Result<BlendSourceTable<T>, SourceEvidenceError> {
    let inspected = count.range.start..range.end;
    Ok(BlendSourceTable {
        declared_count: origins.field(count, count_label)?,
        checked_range: origins.range(range, range_label)?,
        complete: origins.derived(complete, &inspected, "blendTableCoverage")?,
    })
}

fn rtpc_table(
    origins: &ObjectOrigins,
    extent: &BankRtpcExtent,
) -> Result<BlendSourceTable<u16>, SourceEvidenceError> {
    table_fields(
        origins,
        &extent.curve_count,
        &extent.checked_range,
        extent.curve_count.value == extent.checked_curves,
        "blendRtpcCurveCount",
        "blendRtpcCurvesExtent",
    )
}

fn child(
    origins: &ObjectOrigins,
    original: &BankContainerChild,
) -> Result<BlendSourceRow, SourceEvidenceError> {
    let id = &original.child_id;
    Ok(BlendSourceRow::Child {
        ordinal: origins.derived(original.ordinal, &id.range, "containerChildPosition")?,
        child_id: origins.field(id, "containerChildId")?,
    })
}

fn layer_fields<R>(
    origins: &ObjectOrigins,
    original: &BankBlendLayer<'_, R>,
) -> Result<BlendSourceRow, SourceEvidenceError> {
    let fields = &original.fields;
    let layer = Occurrence::new(original.ordinal, &fields.layer_id.range);
    Ok(BlendSourceRow::Layer {
        layer_ordinal: layer.source(origins, "blendLayerPosition")?,
        range: origins.range(&original.checked_range, "blendLayerExtent")?,
        layer_id: origins.field(&fields.layer_id, "blendLayerId")?,
        initial_rtpc: fields
            .rtpc
            .as_ref()
            .map(|extent| rtpc_table(origins, extent))
            .transpose()?,
        crossfade_rtpc_id: origins
            .optional(fields.crossfade_rtpc_id.as_ref(), "blendCrossfadeRtpcId")?,
        crossfade_rtpc_type: origins.optional(
            fields.crossfade_rtpc_type.as_ref(),
            "blendCrossfadeRtpcType",
        )?,
        associations: table(
            origins,
            fields.associations.as_ref(),
            "blendAssociationCount",
            "blendAssociationsExtent",
        )?,
        complete: origins.derived(
            original.issue.is_none(),
            &original.checked_range,
            "blendLayerCoverage",
        )?,
    })
}

fn association_fields<R>(
    origins: &ObjectOrigins,
    layer: Occurrence<'_>,
    original: &BankBlendAssociation<'_, R>,
) -> Result<BlendSourceRow, SourceEvidenceError> {
    let fields = &original.fields;
    let association = Occurrence::new(original.ordinal, &fields.child_id.range);
    Ok(BlendSourceRow::Association {
        layer_ordinal: layer.source(origins, "blendLayerPosition")?,
        association_ordinal: association.source(origins, "blendAssociationPosition")?,
        range: origins.range(&original.checked_range, "blendAssociationExtent")?,
        child_id: origins.field(&fields.child_id, "blendAssociatedChildId")?,
        points: table(
            origins,
            fields.points.as_ref(),
            "blendPointCount",
            "blendPointsExtent",
        )?,
        complete: origins.derived(
            original.issue.is_none(),
            &original.checked_range,
            "blendAssociationCoverage",
        )?,
        reference_resolution: origins.derived(
            BlendReferenceState::Unchecked,
            &fields.child_id.range,
            "blendAssociatedChildId",
        )?,
    })
}

fn crossfade_point(
    origins: &ObjectOrigins,
    layer: Occurrence<'_>,
    association: Occurrence<'_>,
    original: &BankGraphPoint,
) -> Result<BlendSourceRow, SourceEvidenceError> {
    Ok(BlendSourceRow::CrossfadePoint {
        layer_ordinal: layer.source(origins, "blendLayerPosition")?,
        association_ordinal: association.source(origins, "blendAssociationPosition")?,
        point_ordinal: origins.derived(
            original.ordinal,
            &original.from_bits.range,
            "blendCrossfadePointPosition",
        )?,
        range: origins.range(&original.checked_range, "blendCrossfadePointExtent")?,
        from_bits: origins.field(&original.from_bits, "blendPointFromBits")?,
        to_bits: origins.optional(original.to_bits.as_ref(), "blendPointToBits")?,
        interpolation: origins
            .optional(original.interpolation.as_ref(), "blendPointInterpolation")?,
    })
}
