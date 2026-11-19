// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, DataType, RecordTree, TreeBuilder, Value};

use super::item_definition::ItemDefinition;
use super::{Component, ItemComponentsRead, resolution};
use crate::CatalogError;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_ammo_bindings::AmmoBinding;
use crate::catalog_v3::catalog_ammo_containers::AmmoContainer;
use crate::catalog_v3::catalog_weapon_actions::WeaponActions;
use crate::catalog_v3::catalog_weapon_projectile_ammo::{
    self, LinkedAmmunitionReader, WeaponProjectileAmmoSelection,
};
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::export_ledger::ExportLedger;
use crate::item_stats::{PRIMARY_MAGAZINE_FIELD, with_primary_magazine};
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

/// Canonical authored weapon and ammunition components of one physical Item.
#[derive(Debug)]
#[non_exhaustive]
pub struct WeaponSource<'source> {
    /// Physical weapon actions in original component order.
    pub weapon_actions: Vec<WeaponActions<'source>>,
    /// Ammo containers authored directly on this Item.
    pub ammo_containers: Vec<AmmoContainer<'source>>,
    /// Original ammunition and magazine slot bindings authored directly on this Item.
    pub ammo_bindings: Vec<AmmoBinding<'source>>,
}

/// Reads the canonical Item definition and its recognized physical components.
pub(crate) fn read<'source, T>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    project: impl FnOnce(
        &ItemDefinition<'source>,
        &OriginalFlowFields<'_, '_, 'source>,
        &mut ItemComponentsRead<'source>,
    ) -> Result<T, CatalogError>,
) -> Result<(ItemDefinition<'source>, ItemComponentsRead<'source>, T), CatalogError> {
    with_components(
        tree,
        projection,
        admission,
        cancel,
        ComponentScope::All,
        |mut result, originals, capture, numbers| {
            let definition = ItemDefinition::read(
                tree,
                originals,
                capture,
                projection,
                &numbers,
                &mut result.notes,
            )?;
            result.rows.numbers = numbers.into_inner();
            let projected = project(&definition, originals, &mut result)?;
            Ok((definition, result, projected))
        },
    )
}

pub(crate) fn read_weapon_source<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<WeaponSource<'source>>>, CatalogError> {
    let selection_complete = weapon_component_selection_complete(tree);
    with_components(
        tree,
        projection,
        admission,
        cancel,
        ComponentScope::Weapon,
        |result, _, _, _| {
            let rows = result.rows;
            let value = (!rows.weapon_actions.is_empty() || !rows.ammo_containers.is_empty())
                .then(|| WeaponSource {
                    weapon_actions: rows.weapon_actions,
                    ammo_containers: rows.ammo_containers,
                    ammo_bindings: rows.ammo_bindings,
                })
                .into_iter()
                .collect();
            Ok(ScopedSourceRead::new(
                value,
                result.notes,
                selection_complete,
            ))
        },
    )
}

pub(crate) fn read_magazine_resolution<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<resolution::WeaponMagazineResolution>>, CatalogError> {
    let input = projection.original();
    input.check_forge(forge)?;
    let result = with_primary_magazine(
        tree,
        forge,
        &mut TreeBuilder::new(),
        |weapon, observed, _| {
            let Some(observed) = observed else {
                let failed = matches!(
                    weapon.field(PRIMARY_MAGAZINE_FIELD),
                    Some(Value::Error(error)) if error.dangling_guid().is_none()
                );
                return Ok::<_, CatalogError>(ScopedSourceRead::new(
                    Vec::new(),
                    Vec::new(),
                    weapon_component_selection_complete(tree) && !failed,
                ));
            };
            let scope = input.record_scope(admission.record(), tree)?;
            let fields = OriginalFlowFields::read(&scope, cancel)?;
            let row = resolution::read(
                input,
                admission.record(),
                tree,
                &fields,
                observed,
                cancel,
                None,
            )?;
            let complete = row
                .as_ref()
                .is_some_and(|row| row.outcome().origin().source_set_complete());
            Ok(ScopedSourceRead::new(
                row.into_iter().collect(),
                Vec::new(),
                complete,
            ))
        },
    )?;
    Ok(result.unwrap_or_else(|| {
        ScopedSourceRead::new(
            Vec::new(),
            Vec::new(),
            weapon_component_selection_complete(tree),
        )
    }))
}

pub(crate) struct WeaponProjectileSources<'source> {
    pub(crate) actions: Vec<WeaponActions<'source>>,
    pub(crate) selections: Vec<WeaponProjectileAmmoSelection>,
}

pub(crate) fn read_projectile_ammo<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    ledger: &mut ExportLedger,
) -> Result<ScopedSourceRead<WeaponProjectileSources<'source>>, CatalogError> {
    projection.original().check_forge(forge)?;
    let selection_complete = weapon_component_selection_complete(tree);
    with_components(
        tree,
        projection,
        admission,
        cancel,
        ComponentScope::Weapon,
        |result, originals, _, _| {
            let reader = LinkedAmmunitionReader {
                projection,
                forge,
                cancel,
            };
            let rows = catalog_weapon_projectile_ammo::read(
                originals,
                &reader,
                &result.rows.weapon_actions,
                &result.rows.ammo_bindings,
                None,
                ledger,
            )?;
            Ok(ScopedSourceRead::new(
                WeaponProjectileSources {
                    actions: result.rows.weapon_actions,
                    selections: rows,
                },
                result.notes,
                selection_complete,
            ))
        },
    )
}

#[derive(Clone, Copy)]
enum ComponentScope {
    All,
    Weapon,
}

impl ComponentScope {
    const fn includes(self, kind: Component) -> bool {
        match self {
            Self::All => true,
            Self::Weapon => matches!(kind, Component::Weapon | Component::AmmoContainer),
        }
    }
}

fn with_components<'source, T>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    selected: ComponentScope,
    finish: impl FnOnce(
        ItemComponentsRead<'source>,
        &OriginalFlowFields<'_, '_, 'source>,
        &TemplateFieldCapture<'_, '_, '_, 'source>,
        RefCell<Vec<CapturedNumber>>,
    ) -> Result<T, CatalogError>,
) -> Result<T, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let context = projection
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let numbers = RefCell::new(Vec::new());
    let mut result = ItemComponentsRead::default();
    for (value, kind) in components(tree).iter().filter_map(Component::of) {
        if !selected.includes(kind) {
            continue;
        }
        let component = originals.fact(value)?;
        let owner = ComponentOwner::read(tree, &context, &originals, component)?;
        let observed = Observed::new(&originals, input, component, &capture, &numbers);
        result.push(kind, owner, observed)?;
    }
    finish(result, &originals, &capture, numbers)
}

fn components<'read, 'source>(tree: &'read RecordTree<'source>) -> &'read [Value<'source>] {
    match tree.field("Components") {
        Some(Value::Array(components)) => components.as_ref(),
        _ => &[],
    }
}

fn weapon_component_selection_complete(tree: &RecordTree<'_>) -> bool {
    let mut declarations = tree
        .fields()
        .iter()
        .filter(|field| field.name() == "Components");
    let Some(component_field) = declarations.next() else {
        return true;
    };
    let storage = component_field.storage();
    if declarations.next().is_some()
        || storage.data_type() != DataType::StrongPointer
        || !storage.is_array()
        || tree.struct_at(u32::from(storage.target())).is_none()
    {
        return false;
    }
    match component_field.value() {
        Value::Null => true,
        Value::Array(components) => components
            .iter()
            .all(|value| matches!(value, Value::Struct(_) | Value::Null)),
        _ => false,
    }
}
