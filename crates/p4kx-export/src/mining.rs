// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourcedValue, Value};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::authored_fields;

use crate::contract::{
    CompositionPart, MineableComposition, MineableElement, MineableEntity, RefiningProcess,
};
use crate::guid_text::CatalogGuid;
use crate::harvest_link::MINEABLE_PARAMS;
use crate::picked::Picked;
use crate::record_fields::{
    class_name, component, elements, integer, number, reference, single, structs, structure, text,
};
use crate::record_observation::RecordObservation;
use crate::scan_signature::SignatureSlot;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

const SIGNATURE_SYSTEM_PARAMS: &str = "SSCSignatureSystemParams";

type Projection<'source> = fn(&RecordTree<'source>, SourcedValue<GameTextRef>) -> Picked<'source>;

pub(crate) fn named_definition<'source>(
    kind: Kind,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(Picked<'source>, Vec<FieldError>), CatalogError> {
    let (field, project): (&str, Projection<'source>) = match kind {
        Kind::RefiningProcesses => ("processName", |tree, name| {
            Picked::Refining(refining_process(tree, name))
        }),
        Kind::MineableCompositions => ("depositName", |tree, name| {
            Picked::Composition(mineable_composition(tree, name))
        }),
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    let (name, notes) =
        authored_fields::read_root_locale(tree, projection, admission, cancel, field)?;
    Ok((project(tree, name), notes))
}

/// Returns the mineable entity the Record tree `tree` describes, its scan signature taken from
/// `slot`.
pub(crate) fn mineable_entity<'f>(
    tree: &RecordTree<'f>,
    slot: SignatureSlot,
) -> MineableEntity<'f> {
    let mining = component(tree, MINEABLE_PARAMS);
    let radar = structure(component(tree, SIGNATURE_SYSTEM_PARAMS), "radarProperties");
    let base = structure(radar, "baseSignatureParams");
    let signatures: Vec<f32> = elements(base, "signatures")
        .iter()
        .map(|value| match value {
            Value::Single(signature) if signature.is_finite() => *signature,
            _ => 0.0,
        })
        .collect();
    MineableEntity {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        mineable_composition_guid: reference(mining, "composition"),
        mining_global_params_guid: reference(mining, "globalParams"),
        scan_signature: slot.value(&signatures),
        radar_contact_type_guid: reference(radar, "contactType"),
        signatures,
    }
}

fn mineable_composition<'f>(
    tree: &RecordTree<'f>,
    deposit_name: SourcedValue<GameTextRef>,
) -> MineableComposition<'f> {
    let root = Some(tree.root());
    MineableComposition {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        deposit_name,
        minimum_distinct_elements: integer(root, "minimumDistinctElements"),
        parts: structs(root, "compositionArray")
            .map(|part| {
                let part = Some(part);
                CompositionPart {
                    mineable_element_guid: reference(part, "mineableElement"),
                    min_percentage: number(part, "minPercentage"),
                    max_percentage: number(part, "maxPercentage"),
                    probability: number(part, "probability"),
                    curve_exponent: number(part, "curveExponent"),
                    quality_scale: number(part, "qualityScale"),
                }
            })
            .collect(),
    }
}

/// Returns the mineable element the Record tree `tree` describes.
pub(crate) fn mineable_element<'f>(tree: &RecordTree<'f>) -> MineableElement<'f> {
    let root = Some(tree.root());
    MineableElement {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        resource_type_guid: reference(root, "resourceType"),
        instability: single(root, "elementInstability"),
        resistance: single(root, "elementResistance"),
        optimal_window_midpoint: single(root, "elementOptimalWindowMidpoint"),
        optimal_window_midpoint_randomness: single(root, "elementOptimalWindowMidpointRandomness"),
        optimal_window_thinness: single(root, "elementOptimalWindowThinness"),
        explosion_multiplier: single(root, "elementExplosionMultiplier"),
        cluster_factor: single(root, "elementClusterFactor"),
    }
}

fn refining_process<'f>(
    tree: &RecordTree<'f>,
    process_name: SourcedValue<GameTextRef>,
) -> RefiningProcess<'f> {
    let root = Some(tree.root());
    RefiningProcess {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        process_name,
        refining_speed: text(root, "refiningSpeed"),
        refining_quality: text(root, "refiningQuality"),
    }
}
