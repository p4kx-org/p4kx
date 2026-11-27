// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree, SourcedValue, TreeBuilder, Value};

use crate::catalog_text_contract::GameTextRef;

use crate::contract::StarmapObject;
use crate::guid_text::CatalogGuid;
use crate::memo::Memo;
use crate::record_fields::{class_name, reference, text};
use crate::starmap_containers::Placements;
use crate::starmap_containers::proof::PlacementEvidence;

/// The `description` key all mining claim markers share and no other starmap object has (188 of
/// 2,123 on LIVE 4.10).
const MINING_CLAIM_DESCRIPTION: &str = "@MiningClaims_Desc_001";

/// The precision of a position, a millimetre.
const MILLI: f64 = 1000.0;

/// The names of the type Records the starmap objects link to, each read once.
#[derive(Default)]
pub(crate) struct TypeNames<'f> {
    by_guid: Memo<Option<&'f str>>,
}

impl<'f> TypeNames<'f> {
    /// Returns the `name` of the type Record `guid` links to when it is a plain string, else the
    /// Record name without its type.
    fn get(
        &self,
        trees: &mut TreeBuilder<'f>,
        forge: &'f DataForge<'f>,
        guid: CatalogGuid,
    ) -> Option<&'f str> {
        self.by_guid
            .get_or_make(guid.0, || type_of(trees, forge, guid))
    }
}

/// Projects a starmap object with its canonical name, linked type and selected placement.
pub(crate) fn starmap_object<'f>(
    trees: &mut TreeBuilder<'f>,
    tree: &RecordTree<'f>,
    name: SourcedValue<GameTextRef>,
    forge: &'f DataForge<'f>,
    types: &TypeNames<'f>,
    placements: &Placements,
) -> StarmapObject<'f> {
    let root = Some(tree.root());
    let type_guid = reference(root, "type");
    let guid = CatalogGuid(tree.guid());
    let placement = ObjectPlacement::select(guid, placements);
    StarmapObject {
        guid,
        record_index: tree.record_index(),
        class_name: class_name(tree),
        name,
        r#type: type_guid.and_then(|guid| types.get(trees, forge, guid)),
        system: placement.system,
        entity_name: placement.entity_name,
        placed_under_guid: placement.placed_under_guid,
        position: placement.position,
        placement_proof: placement.proof,
        mining_claim: tree.field("description").is_some_and(is_mining_claim),
        location_origin: None,
        shop_link_header: None,
    }
}

pub(crate) struct ObjectPlacement {
    pub(crate) system: Option<String>,
    pub(crate) entity_name: Option<String>,
    pub(crate) placed_under_guid: Option<CatalogGuid>,
    pub(crate) position: Option<[f64; 3]>,
    pub(crate) proof: Option<PlacementEvidence>,
}

impl ObjectPlacement {
    pub(crate) fn select(guid: CatalogGuid, placements: &Placements) -> Self {
        let placement = placements.get(guid.0);
        Self {
            system: placement.map(|placement| placement.system.to_string()),
            entity_name: placement
                .map(|placement| placement.entity_name.as_str())
                .filter(|name| !name.is_empty())
                .map(str::to_owned),
            placed_under_guid: placement
                .and_then(|placement| placement.placed_under)
                .map(CatalogGuid),
            position: placement.map(|placement| placement.position.map(millimetres)),
            proof: placement
                .map(|placement| PlacementEvidence::Selected(placement.proof.clone()))
                .or_else(|| placements.ambiguity_proof(guid.0)),
        }
    }
}

pub(crate) fn is_mining_claim(description: &Value<'_>) -> bool {
    matches!(
        description,
        Value::Locale(text) | Value::String(text) | Value::Enum(text)
            if *text == MINING_CLAIM_DESCRIPTION
    )
}

fn type_of<'f>(
    trees: &mut TreeBuilder<'f>,
    forge: &'f DataForge<'f>,
    guid: CatalogGuid,
) -> Option<&'f str> {
    let record = forge.record_by_guid(guid.0)?;
    let tree = trees.build(&record).ok()?;
    let plain = text(Some(tree.root()), "name").filter(|name| !name.starts_with('@'));
    Some(plain.unwrap_or_else(|| class_name(&tree)))
}

pub(crate) fn millimetres(metres: f64) -> f64 {
    (metres * MILLI).round() / MILLI + 0.0
}
