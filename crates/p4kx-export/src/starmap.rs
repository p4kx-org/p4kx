// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree, TreeBuilder};

use crate::contract::StarmapObject;
use crate::guid_text::CatalogGuid;
use crate::memo::Memo;
use crate::record_fields::{class_name, locale_key, reference, text};
use crate::starmap_containers::Placements;
use crate::starmap_containers::proof::PlacementEvidence;
use crate::texts::Texts;

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

/// Returns the starmap object the Record tree `tree` describes: its name resolved by `texts`, its
/// type from the Record the Reference `type` links to in `forge`, its place from `placements`.
pub(crate) fn starmap_object<'f>(
    trees: &mut TreeBuilder<'f>,
    tree: &RecordTree<'f>,
    texts: &Texts<'f>,
    forge: &'f DataForge<'f>,
    types: &TypeNames<'f>,
    placements: &Placements,
) -> StarmapObject<'f> {
    let root = Some(tree.root());
    let key =
        locale_key(root, "name").or_else(|| text(root, "name").filter(|v| v.starts_with('@')));
    let type_guid = reference(root, "type");
    let placement = placements.get(tree.guid());
    StarmapObject {
        guid: CatalogGuid(tree.guid()),
        record_index: tree.record_index(),
        class_name: class_name(tree),
        name: texts.localised(key),
        r#type: type_guid.and_then(|guid| types.get(trees, forge, guid)),
        system: placement.map(|placement| placement.system.to_string()),
        entity_name: placement
            .map(|placement| placement.entity_name.as_str())
            .filter(|name| !name.is_empty())
            .map(str::to_owned),
        placed_under_guid: placement
            .and_then(|placement| placement.placed_under)
            .map(CatalogGuid),
        position: placement.map(|placement| placement.position.map(millimetres)),
        placement_proof: placement
            .map(|placement| PlacementEvidence::Selected(placement.proof.clone()))
            .or_else(|| placements.ambiguity_proof(tree.guid())),
        mining_claim: locale_key(root, "description").or_else(|| text(root, "description"))
            == Some(MINING_CLAIM_DESCRIPTION),
    }
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
