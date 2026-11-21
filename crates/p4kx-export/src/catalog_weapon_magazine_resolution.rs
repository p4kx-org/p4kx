// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, DataType, Record, RecordTree, SourceEvidenceError,
    SourceUseRole, SourcedValue, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::ComponentOwner;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::catalog_table::CatalogTable;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::item_stats::{MagazineRead, MagazineRoute, PRIMARY_MAGAZINE_FIELD, WEAPON_COMPONENT};
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

#[path = "catalog_magazine_dependencies.rs"]
mod dependency;
pub(crate) use dependency::{MagazineDependencies, MagazineDependency};

#[path = "weapon_magazine_selection_evidence.rs"]
mod evidence;

pub(in crate::catalog_v3) use self::evidence::SelectionEvidence;

type Fields<'scope, 'read, 'source> = OriginalFlowFields<'scope, 'read, 'source>;

/// The legacy primary selection actually attempted for one physical weapon component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct WeaponMagazineResolution {
    #[serde(flatten)]
    owner: ComponentOwner,
    outcome: SourcedValue<Outcome>,
}

/// The observed primary magazine route of a physical weapon component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum Outcome {
    /// The weapon's own first matching container.
    OwnContainer {
        /// The selected original Components slot.
        container_ordinal: SourcedValue<u32>,
    },
    /// The first matching container on the linked magazine.
    LinkedContainer {
        /// The physical Record selected by the authored reference.
        magazine_record_index: SourcedValue<u32>,
        /// The selected slot in the magazine's Components array.
        container_ordinal: SourcedValue<u32>,
    },
    /// The linked magazine was readable and supplied no matching container.
    LinkedWithoutContainer {
        /// The physical Record selected by the authored reference.
        magazine_record_index: SourcedValue<u32>,
    },
    /// The linked magazine's components could not establish container absence.
    LinkedContainerUndetermined {
        /// The physical Record selected by the authored reference.
        magazine_record_index: SourcedValue<u32>,
    },
    /// The original reference reached a Record whose body could not be read.
    LinkedTargetUnreadable {
        /// The physical Record selected by the authored reference.
        magazine_record_index: SourcedValue<u32>,
        /// The reason the linked target was unavailable.
        failure_code: ExportDiagnosticCode,
    },
}

impl WeaponMagazineResolution {
    /// Borrows the physical weapon component owner.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original attempt and its field-level source evidence.
    #[must_use]
    pub const fn outcome(&self) -> &SourcedValue<Outcome> {
        &self.outcome
    }
}

pub(crate) fn read<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    fields: &Fields<'_, '_, 'source>,
    observed: MagazineRead<'_, 'source>,
    cancel: Option<&CancelToken>,
    dependencies: Option<&MagazineDependencies<'_, 'source>>,
) -> Result<Option<WeaponMagazineResolution>, CatalogError> {
    fields.check_tree(tree)?;
    let context = input.record_context(record)?;
    fields.check_context(context.sources(), context.dataforge(), record.index())?;
    let (weapon_value, route) = observed.into_parts();
    let weapon = fields.fact(weapon_value)?;
    let owner = ComponentOwner::read(tree, &context, fields, weapon)?;
    let mut evidence = SelectionEvidence::default();

    let outcome = match route {
        MagazineRoute::Own(value) => {
            let container = fields.fact(value)?;
            let through = if weapon.ordinal() > container.ordinal() {
                weapon_value
            } else {
                value
            };
            evidence.retain(fields, Some(through))?;
            Outcome::OwnContainer {
                container_ordinal: ComponentOwner::ordinal(fields, container)?,
            }
        }
        MagazineRoute::Linked {
            caller,
            record,
            tree,
            container,
        } => {
            let Some(selected) = linked_caller(fields, weapon, caller)? else {
                return Ok(None);
            };
            evidence.retain(fields, None)?;
            linked(
                input,
                selected,
                &LinkedTarget {
                    record,
                    tree,
                    container,
                },
                cancel,
                &mut evidence,
                dependencies,
            )?
        }
    };
    Ok(Some(WeaponMagazineResolution {
        owner,
        outcome: evidence.value(fields.sources(), outcome),
    }))
}

fn linked_caller<'fields, 'scope, 'read, 'source>(
    fields: &'fields Fields<'scope, 'read, 'source>,
    weapon: &OriginalValueFact<'scope, 'read, 'source>,
    caller: &Value<'source>,
) -> Result<Option<&'fields OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
    let (_, source) = fields
        .field(Some(weapon), PRIMARY_MAGAZINE_FIELD)?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    if !std::ptr::eq(source.value(), caller) {
        return Err(SourceEvidenceError::ForeignEvidence.into());
    }
    let storage = source.field().storage();
    Ok((storage.data_type() == DataType::Reference && !storage.is_array()).then_some(source))
}

struct LinkedTarget<'read, 'source> {
    record: Record<'source>,
    tree: Result<&'read RecordTree<'source>, &'read DataForgeError>,
    container: Option<&'read Value<'source>>,
}

fn linked<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    caller: &OriginalValueFact<'_, '_, 'source>,
    target: &LinkedTarget<'_, 'source>,
    cancel: Option<&CancelToken>,
    evidence: &mut SelectionEvidence,
    dependencies: Option<&MagazineDependencies<'_, 'source>>,
) -> Result<Outcome, CatalogError> {
    caller.check_input(input)?;
    let original = caller.reference_observation()?;
    if original.record_index() != target.record.index()
        || original.is_instance()
        || !original
            .reference()
            .original_target()
            .shares_source_with(&target.record)
    {
        return Err(SourceEvidenceError::ForeignEvidence.into());
    }
    let context = input.record_context(&target.record)?;
    evidence.add(caller.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
    evidence.add(context.observe_header(RecordHeaderField::SourceType, SourceUseRole::Selector)?);
    let magazine_record_index = original
        .capture()?
        .record_index
        .into_cell(context.sources());
    match target.tree {
        Err(DataForgeError::ObservationStopped) => Err(CatalogError::Cancelled),
        Err(_) => {
            evidence.attempt(context.observe("", SourceUseRole::Attempted)?);
            Ok(Outcome::LinkedTargetUnreadable {
                magazine_record_index,
                failure_code: ExportDiagnosticCode::SourceReadFailure,
            })
        }
        Ok(tree) => {
            let scope = input.record_scope(&target.record, tree)?;
            let fields = OriginalFlowFields::read(&scope, cancel)?;
            let resolved = evidence.retain(&fields, target.container)?;
            Ok(match target.container {
                Some(container) => {
                    let fact = fields.fact(container)?;
                    if let Some(dependencies) = dependencies {
                        dependencies.retain(input, &target.record, tree, &fields, fact, cancel)?;
                    }
                    Outcome::LinkedContainer {
                        magazine_record_index,
                        container_ordinal: ComponentOwner::ordinal(&fields, fact)?,
                    }
                }
                None if resolved => Outcome::LinkedWithoutContainer {
                    magazine_record_index,
                },
                None => Outcome::LinkedContainerUndetermined {
                    magazine_record_index,
                },
            })
        }
    }
}

pub(super) fn table(
    forge: &DataForge<'_>,
    mut rows: Vec<WeaponMagazineResolution>,
) -> CatalogTable<WeaponMagazineResolution> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[WEAPON_COMPONENT], rows)
}
