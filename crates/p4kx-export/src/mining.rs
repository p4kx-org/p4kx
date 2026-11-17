// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourcedValue, Value};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::{authored_fields, mineable_composition_definition};
use crate::contract::{MineableEntity, RefiningProcess};
use crate::guid_text::CatalogGuid;
use crate::harvest_link::MINEABLE_PARAMS;
use crate::picked::Picked;
use crate::record_fields::{class_name, component, elements, reference, structure, text};
use crate::record_observation::RecordObservation;
use crate::scan_signature::SignatureSlot;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

const SIGNATURE_SYSTEM_PARAMS: &str = "SSCSignatureSystemParams";

pub(crate) fn named_definition<'source>(
    kind: Kind,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(Picked<'source>, Vec<FieldError>), CatalogError> {
    match kind {
        Kind::RefiningProcesses => {
            let (name, notes) = authored_fields::read_root_locale(
                tree,
                projection,
                admission,
                cancel,
                "processName",
            )?;
            Ok((Picked::Refining(refining_process(tree, name)), notes))
        }
        Kind::MineableCompositions => {
            let read = mineable_composition_definition::read(tree, projection, admission, cancel)?;
            Ok((Picked::Composition(Box::new(read.value)), read.notes))
        }
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
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
