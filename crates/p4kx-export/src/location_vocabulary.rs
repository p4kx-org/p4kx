// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

use super::location_vocabulary_definition::{
    LocationClassDefinition, LocationDictionaryEntry, LocationRegistry,
};
use super::prepared_definition::PreparedDefinition;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) const CLASS: &str = "MissionLocationClass";
pub(crate) const FUNCTION: &str = "MissionLocationFunction";
pub(crate) const SIZE: &str = "MissionLocationSize";
pub(crate) const CLASS_REGISTRY: &str = "MissionLocationClasses";
pub(crate) const FUNCTION_REGISTRY: &str = "MissionLocationFunctions";
pub(crate) const SIZE_REGISTRY: &str = "MissionLocationSizes";

pub(crate) fn kind_for_type(source_type: &str) -> Option<Kind> {
    Some(match source_type {
        CLASS => Kind::LocationClasses,
        FUNCTION => Kind::LocationFunctions,
        SIZE => Kind::LocationSizes,
        CLASS_REGISTRY => Kind::LocationClassRegistries,
        FUNCTION_REGISTRY => Kind::LocationFunctionRegistries,
        SIZE_REGISTRY => Kind::LocationSizeRegistries,
        _ => return None,
    })
}

pub(crate) const fn source_type(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::LocationClasses => CLASS,
        Kind::LocationFunctions => FUNCTION,
        Kind::LocationSizes => SIZE,
        Kind::LocationClassRegistries => CLASS_REGISTRY,
        Kind::LocationFunctionRegistries => FUNCTION_REGISTRY,
        Kind::LocationSizeRegistries => SIZE_REGISTRY,
        _ => return None,
    })
}

pub(crate) enum LocationVocabularyPick<'source> {
    Class(Box<PreparedDefinition<'source, LocationClassDefinition<'source>>>),
    Function(Box<PreparedDefinition<'source, LocationDictionaryEntry<'source>>>),
    Size(Box<PreparedDefinition<'source, LocationDictionaryEntry<'source>>>),
    ClassRegistry(Box<PreparedDefinition<'source, LocationRegistry<'source>>>),
    FunctionRegistry(Box<PreparedDefinition<'source, LocationRegistry<'source>>>),
    SizeRegistry(Box<PreparedDefinition<'source, LocationRegistry<'source>>>),
}

impl<'source> LocationVocabularyPick<'source> {
    pub(crate) fn read(
        kind: Kind,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<FieldError>), CatalogError> {
        if kind_for_type(tree.type_name()) != Some(kind) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        match kind {
            Kind::LocationClasses => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                |header, fields, _, notes| LocationClassDefinition::read(header, fields, notes),
            )
            .map(|(row, notes)| (Self::Class(Box::new(row)), notes)),
            Kind::LocationFunctions | Kind::LocationSizes => {
                let (row, notes) = PreparedDefinition::read(
                    tree,
                    projection,
                    admission,
                    cancel,
                    |header, fields, _, _| LocationDictionaryEntry::read(header, fields),
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
            | Kind::LocationSizeRegistries => {
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
                PreparedDefinition::read(
                    tree,
                    projection,
                    admission,
                    cancel,
                    |header, fields, _, notes| {
                        LocationRegistry::read(header, fields, (member, target), notes)
                    },
                )
                .map(|(row, notes)| (variant(Box::new(row)), notes))
            }
            _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
}

#[derive(Default)]
pub(crate) struct LocationVocabulary<'source> {
    classes: Vec<PreparedDefinition<'source, LocationClassDefinition<'source>>>,
    functions: Vec<PreparedDefinition<'source, LocationDictionaryEntry<'source>>>,
    sizes: Vec<PreparedDefinition<'source, LocationDictionaryEntry<'source>>>,
    class_registries: Vec<PreparedDefinition<'source, LocationRegistry<'source>>>,
    function_registries: Vec<PreparedDefinition<'source, LocationRegistry<'source>>>,
    size_registries: Vec<PreparedDefinition<'source, LocationRegistry<'source>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationVocabularyTables<'source> {
    #[serde(rename = "locationClasses")]
    classes: CatalogTable<LocationClassDefinition<'source>>,
    #[serde(rename = "locationFunctions")]
    functions: CatalogTable<LocationDictionaryEntry<'source>>,
    #[serde(rename = "locationSizes")]
    sizes: CatalogTable<LocationDictionaryEntry<'source>>,
    #[serde(rename = "locationRegistries")]
    registries: LocationRegistries<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LocationRegistries<'source> {
    classes: CatalogTable<LocationRegistry<'source>>,
    functions: CatalogTable<LocationRegistry<'source>>,
    sizes: CatalogTable<LocationRegistry<'source>>,
}

impl<'source> LocationVocabulary<'source> {
    pub(crate) fn push(&mut self, pick: LocationVocabularyPick<'source>) {
        match pick {
            LocationVocabularyPick::Class(row) => self.classes.push(*row),
            LocationVocabularyPick::Function(row) => self.functions.push(*row),
            LocationVocabularyPick::Size(row) => self.sizes.push(*row),
            LocationVocabularyPick::ClassRegistry(row) => self.class_registries.push(*row),
            LocationVocabularyPick::FunctionRegistry(row) => self.function_registries.push(*row),
            LocationVocabularyPick::SizeRegistry(row) => self.size_registries.push(*row),
        }
    }

    pub(crate) fn order(&mut self) {
        PreparedDefinition::order(&mut self.classes);
        PreparedDefinition::order(&mut self.functions);
        PreparedDefinition::order(&mut self.sizes);
        PreparedDefinition::order(&mut self.class_registries);
        PreparedDefinition::order(&mut self.function_registries);
        PreparedDefinition::order(&mut self.size_registries);
    }

    pub(crate) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::LocationClasses => self.classes.len(),
            Kind::LocationFunctions => self.functions.len(),
            Kind::LocationSizes => self.sizes.len(),
            Kind::LocationClassRegistries => self.class_registries.len(),
            Kind::LocationFunctionRegistries => self.function_registries.len(),
            Kind::LocationSizeRegistries => self.size_registries.len(),
            _ => 0,
        }
    }

    pub(crate) fn sources(
        &self,
    ) -> impl Iterator<Item = (Kind, Guid, &'source str, &[CapturedNumber])> {
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
    }

    pub(crate) fn number_groups(&self) -> impl Iterator<Item = (Kind, &[CapturedNumber])> {
        self.sources().map(|(kind, _, _, numbers)| (kind, numbers))
    }

    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> LocationVocabularyTables<'source> {
        LocationVocabularyTables {
            classes: PreparedDefinition::table(forge, CLASS, self.classes),
            functions: PreparedDefinition::table(forge, FUNCTION, self.functions),
            sizes: PreparedDefinition::table(forge, SIZE, self.sizes),
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
