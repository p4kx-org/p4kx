// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_projection::{CatalogProjection, ProtectedKey};
use crate::catalog_text_contract::GameTextRef;
use crate::mission_property_source::SourceTagList;
use crate::owned_reference::OwnedReference;
use crate::scoped_location_link::PropertyReference;
use crate::spawn_model::TagPaths;
use crate::template_property::PropertyValue;
use crate::{CatalogError, ExportAccountingError};

/// Metadata inherited from `BaseMissionPropertyValueOption`.
#[derive(Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct OptionMetadata<'f> {
    /// The original nullable `textId`, retaining empty and placeholder tokens.
    pub(crate) text_id: Option<&'f str>,
    /// The protected text reference derived from this option's original key.
    // The unprojected metadata compares authored payloads before choosing an exact source owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "SourcedValue<GameTextRef>", extend("x-gameText" = true))]
    pub(crate) display_name: Option<SourcedValue<GameTextRef>>,
    #[serde(skip)]
    pub(crate) pending_name: Option<ProtectedKey<'f>>,
    /// The source `Single` weighting, including fractional values.
    pub(crate) weighting: Option<f32>,
    /// The authored `DEBUG_forceChooseThisOption` flag.
    pub(crate) debug_force_choose_this_option: Option<bool>,
    /// Source dependencies in order, retaining null declarations.
    pub(crate) dependent_properties: Option<Vec<Option<DependentProperty>>>,
}

/// A dependent mission property and the exact option nodes it names.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct DependentProperty {
    /// The original owner-local property link and resolved variable when available.
    pub(crate) property: Option<PropertyReference>,
    /// The owner-local option links in source order.
    pub(crate) values: Option<Vec<Option<OwnedReference>>>,
}

/// A string-hash option with complete authored metadata.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct StringOption<'f> {
    #[serde(flatten)]
    pub(crate) metadata: OptionMetadata<'f>,
    /// The exact source string, retaining an empty value.
    pub(crate) value: Option<String>,
}

/// An exact signed integer option.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct IntegerOption<'f> {
    #[serde(flatten)]
    pub(crate) metadata: OptionMetadata<'f>,
    /// The source `Int32`, without float conversion.
    #[schemars(range(min = i32::MIN, max = i32::MAX))]
    pub(crate) value: Option<i32>,
    /// The source `Int32` variation.
    #[schemars(range(min = i32::MIN, max = i32::MAX))]
    pub(crate) variation: Option<i32>,
}

/// A floating-point option with complete authored metadata.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct FloatOption<'f> {
    #[serde(flatten)]
    pub(crate) metadata: OptionMetadata<'f>,
    /// The finite source value.
    pub(crate) value: Option<f32>,
    /// The finite source variation.
    pub(crate) variation: Option<f32>,
}

/// A tag option with complete authored metadata and raw tag identities.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct TagOption<'f> {
    #[serde(flatten)]
    pub(crate) metadata: OptionMetadata<'f>,
    /// Resolved positive tag paths.
    pub(crate) tags: TagPaths,
    /// Resolved negative tag paths.
    pub(crate) negative_tags: TagPaths,
    /// The source positive tag list with raw identities.
    pub(crate) source_tags: Option<SourceTagList>,
    /// The source negative tag list with raw identities.
    pub(crate) source_negative_tags: Option<SourceTagList>,
}

impl<'source> PropertyValue<'source> {
    pub(crate) fn publish_option_names(
        &mut self,
        projection: &CatalogProjection<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        match self {
            Self::StringHash { options } => publish_options(
                options.as_deref_mut(),
                |option| &mut option.metadata,
                projection,
                cancel,
            ),
            Self::Integer { options } => publish_options(
                options.as_deref_mut(),
                |option| &mut option.metadata,
                projection,
                cancel,
            ),
            Self::Float { options } => publish_options(
                options.as_deref_mut(),
                |option| &mut option.metadata,
                projection,
                cancel,
            ),
            Self::Tags { options, .. } => publish_options(
                options.as_deref_mut(),
                |option| &mut option.metadata,
                projection,
                cancel,
            ),
            _ => Ok(()),
        }
    }
}

fn publish_options<'source, T>(
    options: Option<&mut [Option<T>]>,
    mut metadata: impl FnMut(&mut T) -> &mut OptionMetadata<'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let Some(options) = options else {
        return Ok(());
    };
    for option in options.iter_mut().flatten() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let metadata = metadata(option);
        let key = metadata
            .pending_name
            .take()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        metadata.display_name = Some(projection.finish_protected_key(key)?);
    }
    Ok(())
}
