// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourcedValue, is_placeholder_key};

use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::commodity_inputs::CommodityInputs;
use crate::commodity_text_input::{CommodityTextInput, CommodityTextSelector};

/// Returns whether the original own key permits consulting the resource fallback.
#[must_use]
pub(crate) fn needs_fallback(own: Option<&CommodityTextInput<'_>>) -> bool {
    own.and_then(raw_key).is_none_or(is_placeholder_key)
}

/// Chooses the established public name before any language lookup.
pub(crate) fn name<'source>(
    projection: &CatalogProjection<'_, 'source>,
    own: Option<&CommodityTextInput<'source>>,
    fallback: Option<&CommodityTextInput<'source>>,
    fallback_inputs: &[SourceUse],
    fallback_complete: bool,
) -> Result<SourcedValue<Option<PublicNameRef>>, CatalogError> {
    choice(
        projection,
        own,
        fallback,
        fallback_inputs,
        fallback_complete,
        |input| projection.name(input.key.clone(), input.record_index, &input.field),
    )
}

/// Chooses an original protected text key without a per-language fallback.
pub(crate) fn game_text<'source>(
    projection: &CatalogProjection<'_, 'source>,
    own: Option<&CommodityTextInput<'source>>,
    fallback: Option<&CommodityTextInput<'source>>,
    fallback_inputs: &[SourceUse],
    fallback_complete: bool,
) -> Result<SourcedValue<Option<GameTextRef>>, CatalogError> {
    choice(
        projection,
        own,
        fallback,
        fallback_inputs,
        fallback_complete,
        |input| projection.description(input.key.clone(), input.record_index, &input.field),
    )
}

fn choice<'source, R: Clone>(
    projection: &CatalogProjection<'_, 'source>,
    own: Option<&CommodityTextInput<'source>>,
    fallback: Option<&CommodityTextInput<'source>>,
    fallback_inputs: &[SourceUse],
    fallback_complete: bool,
    register: impl FnOnce(&CommodityTextInput<'source>) -> Result<SourcedValue<R>, CatalogError>,
) -> Result<SourcedValue<Option<R>>, CatalogError> {
    let mut inputs = CommodityInputs::new(projection.origins());
    if let Some(own) = own {
        decision(&mut inputs, own)?;
    }
    if needs_fallback(own) {
        inputs.extend(fallback_inputs.iter().cloned(), fallback_complete);
        if let Some(fallback) = fallback {
            decision(&mut inputs, fallback)?;
        }
    }
    let Some(selected) = selected(own, fallback) else {
        return Ok(inputs.value(None));
    };
    inputs.field(
        selected.record_index,
        &selected.field,
        &selected.key,
        SourceUseRole::Payload,
    )?;
    inputs.field(
        selected.record_index,
        &selected.field,
        &selected.key,
        SourceUseRole::Conversion,
    )?;
    let reference = register(selected)?;
    inputs.require_complete(reference.origin().source_set_complete());
    Ok(inputs.value(reference.value().cloned()))
}

fn decision(
    inputs: &mut CommodityInputs<'_>,
    input: &CommodityTextInput<'_>,
) -> Result<(), CatalogError> {
    for selector in &input.selectors {
        match selector {
            CommodityTextSelector::Body(path) => {
                inputs.at(input.record_index, path, SourceUseRole::Selector)?;
            }
            CommodityTextSelector::Header(field) => {
                inputs.header(input.record_index, *field, SourceUseRole::Selector)?;
            }
        }
    }
    inputs.field(
        input.record_index,
        &input.field,
        &input.key,
        SourceUseRole::Selector,
    )?;
    inputs.require_complete(input.selectors_complete);
    Ok(())
}

fn selected<'input, 'source>(
    own: Option<&'input CommodityTextInput<'source>>,
    fallback: Option<&'input CommodityTextInput<'source>>,
) -> Option<&'input CommodityTextInput<'source>> {
    if !needs_fallback(own) {
        return own;
    }
    fallback
        .filter(|input| raw_key(input).is_some())
        .or_else(|| own.filter(|input| raw_key(input).is_some()))
}

fn raw_key<'source>(input: &CommodityTextInput<'source>) -> Option<&'source str> {
    input.key.value().copied().filter(|key| !key.is_empty())
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::{SourceUses, SourceValue};

    use super::{CommodityTextInput, needs_fallback, raw_key, selected};

    fn input(value: SourceValue<&str>, record_index: u32) -> CommodityTextInput<'_> {
        let complete = !matches!(&value, SourceValue::Unreadable);
        CommodityTextInput {
            key: SourceUses::new().value(value, &[], complete),
            record_index,
            field: "synthetic.displayName".to_owned(),
            selectors: Vec::new(),
            selectors_complete: true,
        }
    }

    #[test]
    fn fallback_is_decided_from_raw_key_presence_and_placeholder_spelling() {
        for value in [
            SourceValue::Absent,
            SourceValue::Null,
            SourceValue::Unreadable,
            SourceValue::Unsupported,
            SourceValue::Invalid,
            SourceValue::Present(""),
            SourceValue::Present("@LOC_PLACEHOLDER"),
            SourceValue::Present("loc_empty"),
        ] {
            assert!(
                needs_fallback(Some(&input(value, 3))),
                "missing and placeholder keys permit fallback"
            );
        }
        for key in ["@valid_without_translation", "  ", "@original"] {
            assert!(
                !needs_fallback(Some(&input(SourceValue::Present(key), 3))),
                "a nonempty own key wins independently of translation availability"
            );
        }
        assert!(
            needs_fallback(None),
            "a resource-only presentation may select its own source as fallback"
        );
    }

    #[test]
    fn a_valid_own_key_keeps_its_physical_owner_even_for_identical_fallback_text() {
        let own = input(SourceValue::Present("@same"), 3);
        let fallback = input(SourceValue::Present("@same"), 7);
        let chosen = selected(Some(&own), Some(&fallback));
        assert_eq!(
            chosen.map(|input| input.record_index),
            Some(3),
            "equal text does not merge source owners"
        );
        assert_eq!(
            chosen.and_then(raw_key),
            Some("@same"),
            "the original spelling is retained"
        );
    }

    #[test]
    fn a_nonempty_resource_key_replaces_an_own_placeholder_even_when_both_are_placeholders() {
        let own = input(SourceValue::Present("@LOC_EMPTY"), 3);
        for key in ["@resource", "@blank_space"] {
            let fallback = input(SourceValue::Present(key), 7);
            let chosen = selected(Some(&own), Some(&fallback));
            assert_eq!(
                chosen.map(|input| input.record_index),
                Some(7),
                "fallback precedes original placeholder"
            );
            assert_eq!(
                chosen.and_then(raw_key),
                Some(key),
                "no placeholder or spelling is normalized"
            );
        }
    }

    #[test]
    fn missing_resource_keys_keep_an_original_placeholder_and_do_not_invent_a_key() {
        let own = input(SourceValue::Present("@LOC_PLACEHOLDER"), 3);
        for value in [
            SourceValue::Absent,
            SourceValue::Null,
            SourceValue::Unreadable,
            SourceValue::Present(""),
        ] {
            let fallback = input(value, 7);
            assert_eq!(
                selected(Some(&own), Some(&fallback)).and_then(raw_key),
                Some("@LOC_PLACEHOLDER"),
                "an unusable fallback preserves the own placeholder"
            );
            assert!(
                selected(None, Some(&fallback)).is_none(),
                "absent keys produce no chosen text"
            );
        }
        let empty = input(SourceValue::Present(""), 3);
        assert!(
            selected(Some(&empty), None).is_none(),
            "explicit empty is retained only as an authored fact"
        );
    }
}
