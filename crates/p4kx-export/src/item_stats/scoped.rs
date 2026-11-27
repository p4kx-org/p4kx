// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForgeError, Record, RecordTree, TreeBuilder, Value};

use super::gear::{ARMOR_COMPONENT, ARMOR_LINK, RESISTANCE_MACRO};
use super::weapon::{AMMO_CAPACITY_FIELD, AMMO_PARAMS_FIELD, ENTITY};
use super::{
    ItemStats, MagazineRead, MagazineRoute, PRIMARY_MAGAZINE_FIELD, WEAPON_COMPONENT, ammo,
    read_with_sources,
};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::catalog_durability;
use crate::record_fields::component_occurrence;
use crate::variant_evidence::VariantEvidence;

#[path = "scoped_evidence.rs"]
mod evidence;
use evidence::Evidence;

/// The canonical compatibility value and its original source coverage.
pub(crate) struct ScopedStats<'source> {
    pub(crate) stats: Option<ItemStats<'source>>,
    pub(crate) evidence: VariantEvidence,
}

/// Reads the original operands of the owning Record's compatibility stats.
pub(crate) fn read_scoped<'source>(
    input: &OriginalCatalogInput<'source, 'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    trees: &mut TreeBuilder<'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedStats<'source>, CatalogError> {
    let context = input.record_context(record)?;
    fields.check_context(context.sources(), context.dataforge(), record.index())?;
    fields.check_tree(tree)?;
    let mut owner = Evidence::default();
    owner.owner(input, record)?;
    let mut magazine = Evidence::default();
    let mut ammunition = Evidence::default();
    let mut armor = Evidence::default();
    let mut magazine_observed = false;
    let mut resistance_observed = false;

    let stats = read_with_sources(
        tree,
        input.forge(),
        trees,
        || catalog_durability::legacy_health(tree),
        |target, trees| {
            checkpoint(cancel)?;
            let body = trees.build(&target.record);
            ammunition.body(input, &target.record, body.as_ref(), None, cancel)?;
            Ok(body.as_ref().ok().map(ammo))
        },
        |selected| {
            magazine_observed = true;
            observe_magazine(input, fields, selected, &mut magazine, cancel)
        },
        |target, body| {
            resistance_observed = true;
            armor.selected_link(input, fields, target.selector, &target.record)?;
            armor.body(input, &target.record, body, Some(ARMOR_LINK), cancel)
        },
    )?;

    let first_weapon = component_occurrence(tree, WEAPON_COMPONENT).map(|(value, _)| value);
    owner.select(fields, first_weapon)?;
    if let Some(weapon) = first_weapon
        && !magazine_observed
    {
        magazine.select(fields, None)?;
        magazine.link_field(input, fields, weapon, PRIMARY_MAGAZINE_FIELD, Some(ENTITY))?;
    }
    let first_suit = component_occurrence(tree, ARMOR_COMPONENT).map(|(value, _)| value);
    owner.select(fields, first_suit)?;
    if let Some(suit) = first_suit
        && !resistance_observed
    {
        armor.link_field(input, fields, suit, ARMOR_LINK, Some(RESISTANCE_MACRO))?;
    }
    owner.extend(magazine);
    owner.extend(ammunition);
    owner.extend(armor);
    checkpoint(cancel)?;
    Ok(ScopedStats {
        stats,
        evidence: owner.finish(fields.sources()),
    })
}

fn observe_magazine<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    selected: MagazineRead<'_, 'source>,
    evidence: &mut Evidence,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let (_, route) = selected.into_parts();
    match route {
        MagazineRoute::Own(container) => {
            observe_container(input, fields, Some(container), evidence)
        }
        MagazineRoute::Linked {
            caller,
            record,
            tree,
            container,
        } => {
            evidence.select(fields, None)?;
            evidence.selected_link(input, fields, caller, &record)?;
            match tree {
                Ok(tree) => {
                    let scope = input.record_scope(&record, tree)?;
                    let fields = OriginalFlowFields::read(&scope, cancel)?;
                    observe_container(input, &fields, container, evidence)
                }
                Err(DataForgeError::ObservationStopped) => Err(CatalogError::Cancelled),
                Err(_) => evidence.failed_body(input, &record),
            }
        }
    }
}

fn observe_container(
    input: &OriginalCatalogInput<'_, '_>,
    fields: &OriginalFlowFields<'_, '_, '_>,
    container: Option<&Value<'_>>,
    evidence: &mut Evidence,
) -> Result<(), CatalogError> {
    evidence.select(fields, container)?;
    if let Some(container) = container {
        evidence.named_value(fields, container, AMMO_CAPACITY_FIELD)?;
        evidence.link_field(input, fields, container, AMMO_PARAMS_FIELD, None)?;
    }
    Ok(())
}
