// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;
#[path = "contract_scope_global_params.rs"]
mod contract_scope_global_params;
#[path = "location_resource_slot.rs"]
mod location_resource_slot;

pub(crate) use contract_scope_global_params::read_selected as read_contract_scope_selected;
pub use contract_scope_global_params::{ContractScopeGlobalParams, ContractScopeItem};
pub use location_resource_slot::LocationResourceSlot;

use super::location_entity_declaration::{self, LocationEntityDeclaration};
use super::location_vocabulary_definition::{
    LocationClassDefinition, LocationDictionaryEntry, LocationRegistry,
};
use super::prepared_definition::PreparedDefinition;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_row::CatalogRow;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) const CLASS: &str = "MissionLocationClass";
pub(crate) const FUNCTION: &str = "MissionLocationFunction";
pub(crate) const SIZE: &str = "MissionLocationSize";
pub(crate) const RESOURCE_SLOT: &str = location_resource_slot::SOURCE_TYPE;
pub(crate) const CLASS_REGISTRY: &str = "MissionLocationClasses";
pub(crate) const FUNCTION_REGISTRY: &str = "MissionLocationFunctions";
pub(crate) const SIZE_REGISTRY: &str = "MissionLocationSizes";
pub(crate) const CONTRACT_SCOPE: &str = contract_scope_global_params::SOURCE_TYPE;

pub(crate) fn kind_for_type(source_type: &str) -> Option<Kind> {
    Some(match source_type {
        CLASS => Kind::LocationClasses,
        FUNCTION => Kind::LocationFunctions,
        SIZE => Kind::LocationSizes,
        RESOURCE_SLOT => Kind::LocationResourceSlots,
        location_entity_declaration::SOURCE_TYPE => Kind::LocationEntityDeclarations,
        CLASS_REGISTRY => Kind::LocationClassRegistries,
        FUNCTION_REGISTRY => Kind::LocationFunctionRegistries,
        SIZE_REGISTRY => Kind::LocationSizeRegistries,
        CONTRACT_SCOPE => Kind::ContractScopes,
        _ => return None,
    })
}

pub(crate) const fn source_type(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::LocationClasses => CLASS,
        Kind::LocationFunctions => FUNCTION,
        Kind::LocationSizes => SIZE,
        Kind::LocationResourceSlots => RESOURCE_SLOT,
        Kind::LocationEntityDeclarations => location_entity_declaration::SOURCE_TYPE,
        Kind::LocationClassRegistries => CLASS_REGISTRY,
        Kind::LocationFunctionRegistries => FUNCTION_REGISTRY,
        Kind::LocationSizeRegistries => SIZE_REGISTRY,
        Kind::ContractScopes => CONTRACT_SCOPE,
        _ => return None,
    })
}

pub(crate) enum LocationVocabularyPick<'source> {
    Class(Box<PreparedDefinition<'source, LocationClassDefinition<'source>>>),
    Function(Box<PreparedDefinition<'source, LocationDictionaryEntry<'source>>>),
    Size(Box<PreparedDefinition<'source, LocationDictionaryEntry<'source>>>),
    ResourceSlot(Box<PreparedDefinition<'source, LocationResourceSlot<'source>>>),
    EntityDeclaration(Box<PreparedDefinition<'source, LocationEntityDeclaration<'source>>>),
    ClassRegistry(Box<PreparedDefinition<'source, LocationRegistry<'source>>>),
    FunctionRegistry(Box<PreparedDefinition<'source, LocationRegistry<'source>>>),
    SizeRegistry(Box<PreparedDefinition<'source, LocationRegistry<'source>>>),
    ContractScope(Box<PreparedDefinition<'source, ContractScopeGlobalParams<'source>>>),
}

impl<'source> LocationVocabularyPick<'source> {
    pub(crate) fn read(
        kind: Kind,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<FieldError>), CatalogError> {
        match (kind, tree.type_name()) {
            (Kind::ContractScopes, CONTRACT_SCOPE) => {
                Self::contract_scope(tree, projection, admission, cancel)
            }
            (Kind::LocationEntityDeclarations, location_entity_declaration::SOURCE_TYPE) => {
                location_entity_declaration::prepare(tree, projection, admission, cancel)
                    .map(|(row, notes)| (Self::EntityDeclaration(Box::new(row)), notes))
            }
            _ => Self::read_original(kind, tree, projection.original(), admission, cancel),
        }
    }

    /// Reads the original vocabulary families that require no localized projection.
    pub(crate) fn read_original(
        kind: Kind,
        tree: &RecordTree<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<FieldError>), CatalogError> {
        if kind_for_type(tree.type_name()) != Some(kind) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        match kind {
            Kind::LocationClasses => PreparedDefinition::read_original(
                tree,
                input,
                admission,
                cancel,
                LocationClassDefinition::read,
            )
            .map(|(row, notes)| (Self::Class(Box::new(row)), notes)),
            Kind::LocationFunctions | Kind::LocationSizes => {
                let (row, notes) = PreparedDefinition::read_original(
                    tree,
                    input,
                    admission,
                    cancel,
                    |header, fields, _| LocationDictionaryEntry::read(header, fields),
                )?;
                let row = Box::new(row);
                let pick = if kind == Kind::LocationFunctions {
                    Self::Function(row)
                } else {
                    Self::Size(row)
                };
                Ok((pick, notes))
            }
            Kind::LocationClassRegistries
            | Kind::LocationFunctionRegistries
            | Kind::LocationSizeRegistries => Self::registry(kind, tree, input, admission, cancel),
            Kind::LocationResourceSlots => PreparedDefinition::read_original(
                tree,
                input,
                admission,
                cancel,
                LocationResourceSlot::read,
            )
            .map(|(row, notes)| (Self::ResourceSlot(Box::new(row)), notes)),
            _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }

    fn registry(
        kind: Kind,
        tree: &RecordTree<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<FieldError>), CatalogError> {
        let (member, target, variant) = match kind {
            Kind::LocationClassRegistries => {
                ("locationClasses", CLASS, Self::ClassRegistry as fn(_) -> _)
            }
            Kind::LocationFunctionRegistries => (
                "locationFunctions",
                FUNCTION,
                Self::FunctionRegistry as fn(_) -> _,
            ),
            _ => ("locationSizes", SIZE, Self::SizeRegistry as fn(_) -> _),
        };
        PreparedDefinition::read_original(
            tree,
            input,
            admission,
            cancel,
            |header, fields, notes| LocationRegistry::read(header, fields, (member, target), notes),
        )
        .map(|(row, notes)| (variant(Box::new(row)), notes))
    }

    fn contract_scope(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<FieldError>), CatalogError> {
        let (row, notes) =
            contract_scope_global_params::prepare(tree, projection, admission, cancel)?;
        Ok((Self::ContractScope(Box::new(row)), notes))
    }
}

#[derive(Default)]
pub(crate) struct LocationVocabulary<'source> {
    classes: Vec<PreparedDefinition<'source, LocationClassDefinition<'source>>>,
    functions: Vec<PreparedDefinition<'source, LocationDictionaryEntry<'source>>>,
    sizes: Vec<PreparedDefinition<'source, LocationDictionaryEntry<'source>>>,
    resource_slots: Vec<PreparedDefinition<'source, LocationResourceSlot<'source>>>,
    entity_declarations: Vec<PreparedDefinition<'source, LocationEntityDeclaration<'source>>>,
    class_registries: Vec<PreparedDefinition<'source, LocationRegistry<'source>>>,
    function_registries: Vec<PreparedDefinition<'source, LocationRegistry<'source>>>,
    size_registries: Vec<PreparedDefinition<'source, LocationRegistry<'source>>>,
    contract_scopes: Vec<PreparedDefinition<'source, ContractScopeGlobalParams<'source>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationVocabularyTables<'source> {
    #[serde(rename = "locationClasses")]
    pub(crate) classes: CatalogTable<LocationClassDefinition<'source>>,
    #[serde(rename = "locationFunctions")]
    pub(crate) functions: CatalogTable<LocationDictionaryEntry<'source>>,
    #[serde(rename = "locationSizes")]
    pub(crate) sizes: CatalogTable<LocationDictionaryEntry<'source>>,
    #[serde(rename = "locationResourceSlots")]
    pub(crate) resource_slots: CatalogTable<LocationResourceSlot<'source>>,
    #[serde(rename = "locationEntityDeclarations")]
    entity_declarations: CatalogTable<LocationEntityDeclaration<'source>>,
    #[serde(rename = "locationRegistries")]
    pub(crate) registries: LocationRegistries<'source>,
    contract_scopes: CatalogTable<ContractScopeGlobalParams<'source>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationRegistries<'source> {
    pub(crate) classes: CatalogTable<LocationRegistry<'source>>,
    pub(crate) functions: CatalogTable<LocationRegistry<'source>>,
    pub(crate) sizes: CatalogTable<LocationRegistry<'source>>,
}

impl<'source> LocationVocabulary<'source> {
    pub(crate) fn push(&mut self, pick: LocationVocabularyPick<'source>) {
        match pick {
            LocationVocabularyPick::Class(row) => self.classes.push(*row),
            LocationVocabularyPick::Function(row) => self.functions.push(*row),
            LocationVocabularyPick::Size(row) => self.sizes.push(*row),
            LocationVocabularyPick::ResourceSlot(row) => self.resource_slots.push(*row),
            LocationVocabularyPick::EntityDeclaration(row) => self.entity_declarations.push(*row),
            LocationVocabularyPick::ClassRegistry(row) => self.class_registries.push(*row),
            LocationVocabularyPick::FunctionRegistry(row) => self.function_registries.push(*row),
            LocationVocabularyPick::SizeRegistry(row) => self.size_registries.push(*row),
            LocationVocabularyPick::ContractScope(row) => self.contract_scopes.push(*row),
        }
    }

    pub(crate) fn order(&mut self) {
        PreparedDefinition::order(&mut self.classes);
        PreparedDefinition::order(&mut self.functions);
        PreparedDefinition::order(&mut self.sizes);
        PreparedDefinition::order(&mut self.resource_slots);
        PreparedDefinition::order(&mut self.entity_declarations);
        PreparedDefinition::order(&mut self.class_registries);
        PreparedDefinition::order(&mut self.function_registries);
        PreparedDefinition::order(&mut self.size_registries);
        PreparedDefinition::order(&mut self.contract_scopes);
    }

    pub(crate) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::LocationClasses => self.classes.len(),
            Kind::LocationFunctions => self.functions.len(),
            Kind::LocationSizes => self.sizes.len(),
            Kind::LocationResourceSlots => self.resource_slots.len(),
            Kind::LocationEntityDeclarations => self.entity_declarations.len(),
            Kind::LocationClassRegistries => self.class_registries.len(),
            Kind::LocationFunctionRegistries => self.function_registries.len(),
            Kind::LocationSizeRegistries => self.size_registries.len(),
            Kind::ContractScopes => self.contract_scopes.len(),
            _ => 0,
        }
    }

    pub(crate) fn catalog_rows(&self) -> impl Iterator<Item = CatalogRow> + '_ {
        PreparedDefinition::catalog_rows(&self.classes, Kind::LocationClasses)
            .chain(PreparedDefinition::catalog_rows(
                &self.functions,
                Kind::LocationFunctions,
            ))
            .chain(PreparedDefinition::catalog_rows(
                &self.sizes,
                Kind::LocationSizes,
            ))
            .chain(PreparedDefinition::catalog_rows(
                &self.resource_slots,
                Kind::LocationResourceSlots,
            ))
            .chain(PreparedDefinition::catalog_rows(
                &self.entity_declarations,
                Kind::LocationEntityDeclarations,
            ))
            .chain(PreparedDefinition::catalog_rows(
                &self.class_registries,
                Kind::LocationClassRegistries,
            ))
            .chain(PreparedDefinition::catalog_rows(
                &self.function_registries,
                Kind::LocationFunctionRegistries,
            ))
            .chain(PreparedDefinition::catalog_rows(
                &self.size_registries,
                Kind::LocationSizeRegistries,
            ))
            .chain(PreparedDefinition::catalog_rows(
                &self.contract_scopes,
                Kind::ContractScopes,
            ))
    }

    fn sources(&self) -> impl Iterator<Item = (Kind, Guid, &'source str, &[CapturedNumber])> {
        PreparedDefinition::sources(Kind::LocationClasses, &self.classes)
            .chain(PreparedDefinition::sources(
                Kind::LocationFunctions,
                &self.functions,
            ))
            .chain(PreparedDefinition::sources(
                Kind::LocationSizes,
                &self.sizes,
            ))
            .chain(PreparedDefinition::sources(
                Kind::LocationResourceSlots,
                &self.resource_slots,
            ))
            .chain(PreparedDefinition::sources(
                Kind::LocationEntityDeclarations,
                &self.entity_declarations,
            ))
            .chain(PreparedDefinition::sources(
                Kind::LocationClassRegistries,
                &self.class_registries,
            ))
            .chain(PreparedDefinition::sources(
                Kind::LocationFunctionRegistries,
                &self.function_registries,
            ))
            .chain(PreparedDefinition::sources(
                Kind::LocationSizeRegistries,
                &self.size_registries,
            ))
            .chain(PreparedDefinition::sources(
                Kind::ContractScopes,
                &self.contract_scopes,
            ))
    }

    pub(crate) fn number_groups(&self) -> impl Iterator<Item = (Kind, &[CapturedNumber])> {
        self.sources().map(|(kind, _, _, numbers)| (kind, numbers))
    }

    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> LocationVocabularyTables<'source> {
        LocationVocabularyTables {
            classes: PreparedDefinition::table(forge, CLASS, self.classes),
            functions: PreparedDefinition::table(forge, FUNCTION, self.functions),
            sizes: PreparedDefinition::table(forge, SIZE, self.sizes),
            resource_slots: PreparedDefinition::table(forge, RESOURCE_SLOT, self.resource_slots),
            entity_declarations: PreparedDefinition::table(
                forge,
                location_entity_declaration::SOURCE_TYPE,
                self.entity_declarations,
            ),
            contract_scopes: PreparedDefinition::table(forge, CONTRACT_SCOPE, self.contract_scopes),
            registries: LocationRegistries {
                classes: PreparedDefinition::table(forge, CLASS_REGISTRY, self.class_registries),
                functions: PreparedDefinition::table(
                    forge,
                    FUNCTION_REGISTRY,
                    self.function_registries,
                ),
                sizes: PreparedDefinition::table(forge, SIZE_REGISTRY, self.size_registries),
            },
        }
    }
}
