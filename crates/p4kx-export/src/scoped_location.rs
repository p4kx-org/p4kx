// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::scoped_location_link::{DefinitionReference, PropertyReference};

/// The six independently optional source filters of a scoped mission location.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ScopedLocation {
    /// The `factionFilter` declaration.
    #[serde(rename = "factionFilter")]
    pub faction: Option<FactionFilter>,
    /// The `locationClassFilter` declaration.
    #[serde(rename = "locationClassFilter")]
    pub class: Option<ClassFilter>,
    /// The `locationSizeFilter` declaration.
    #[serde(rename = "locationSizeFilter")]
    pub size: Option<SizeFilter>,
    /// The `locationFunctionFilter` declaration.
    #[serde(rename = "locationFunctionFilter")]
    pub function: Option<FunctionFilter>,
    /// The `otherLocationFilter` declaration.
    #[serde(rename = "otherLocationFilter")]
    pub other: Option<OtherLocationFilter>,
    /// The `relativeLocationFilter` declaration.
    #[serde(rename = "relativeLocationFilter")]
    pub relative: Option<RelativeLocationFilter>,
}

/// A source faction-hostility condition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum FactionFilter {
    /// `LocationCondition_RequiredFactionHostility`.
    RequiredFactionHostility(Hostility),
    /// `LocationCondition_ExcludedFactionHostility`.
    ExcludedFactionHostility(Hostility),
}

/// A raw `ReactionType` name.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct Hostility {
    /// The hostility enum option without interpretation.
    pub hostility: Option<String>,
}

/// A source `locationClass` condition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ClassFilter {
    /// `LocationCondition_RequiredLocationClass`.
    RequiredLocationClass(ClassTarget),
    /// `LocationCondition_RequiredLocationClasses`.
    RequiredLocationClasses(ClassTargets),
    /// `LocationCondition_ExcludedLocationClass`.
    ExcludedLocationClass(ClassTarget),
    /// `LocationCondition_ExcludedLocationClasses`.
    ExcludedLocationClasses(ClassTargets),
}

/// One source `locationClass` Reference.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ClassTarget {
    /// The `locationClass` target, including a dangling GUID.
    pub location_class: Option<DefinitionReference>,
}

/// An ordered source `locationClasses` Reference array.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ClassTargets {
    /// The `locationClasses` targets, retaining null elements and repetitions.
    pub location_classes: Option<Vec<Option<DefinitionReference>>>,
}

/// A source `locationSize` condition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum SizeFilter {
    /// `LocationCondition_RequiredLocationSize`.
    RequiredLocationSize(SizeTarget),
    /// `LocationCondition_RequiredLocationSizes`.
    RequiredLocationSizes(SizeTargets),
    /// `LocationCondition_ExcludedLocationSize`.
    ExcludedLocationSize(SizeTarget),
    /// `LocationCondition_ExcludedLocationSizes`.
    ExcludedLocationSizes(SizeTargets),
}

/// One source `locationSize` Reference.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SizeTarget {
    /// The `locationSize` target, including a dangling GUID.
    pub location_size: Option<DefinitionReference>,
}

/// An ordered source `locationSizes` Reference array.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SizeTargets {
    /// The `locationSizes` targets, retaining null elements and repetitions.
    pub location_sizes: Option<Vec<Option<DefinitionReference>>>,
}

/// A source `locationFunction` condition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum FunctionFilter {
    /// `LocationCondition_RequiredLocationFunction`.
    RequiredLocationFunction(FunctionTarget),
    /// `LocationCondition_RequiredLocationFunctions`.
    RequiredLocationFunctions(FunctionTargets),
    /// `LocationCondition_ExcludedLocationFunction`.
    ExcludedLocationFunction(FunctionTarget),
    /// `LocationCondition_ExcludedLocationFunctions`.
    ExcludedLocationFunctions(FunctionTargets),
}

/// One source `locationFunction` Reference.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct FunctionTarget {
    /// The `locationFunction` target, including a dangling GUID.
    pub location_function: Option<DefinitionReference>,
}

/// An ordered source `locationFunctions` Reference array.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct FunctionTargets {
    /// The `locationFunctions` targets, retaining null elements and repetitions.
    pub location_functions: Option<Vec<Option<DefinitionReference>>>,
}

/// A source relation to another location property.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum OtherLocationFilter {
    /// `LocationCondition_NotLocation`.
    NotLocation(PropertyTarget),
    /// `LocationCondition_SameLocation`.
    SameLocation(PropertyTarget),
    /// `LocationCondition_NotLocations`.
    NotLocations(PropertyTargets),
}

/// One declared weak link to a mission property.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct PropertyTarget {
    /// The location link and its owner-scoped resolution.
    pub location: Option<PropertyReference>,
}

/// An ordered array of declared weak links to mission properties.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct PropertyTargets {
    /// The locations links, retaining null elements and repetitions.
    pub locations: Option<Vec<Option<PropertyReference>>>,
}

/// A source scope relation to another location property.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum RelativeLocationFilter {
    /// `LocationCondition_SameScope`.
    SameScope(RelativeTarget),
    /// `LocationCondition_DifferentScope`.
    DifferentScope(RelativeTarget),
}

/// A raw `ContractScope` name and its declared location property.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct RelativeTarget {
    /// The scope enum option without interpretation.
    pub scope: Option<String>,
    /// The location link and its owner-scoped resolution.
    pub location: Option<PropertyReference>,
}
