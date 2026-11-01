// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree, StructValue, Value};

use crate::guid_text::CatalogGuid;

/// Returns the Record name without the leading `<type>.`, e.g. `ACAS` for
/// `SCItemManufacturer.ACAS`; a name without that start is returned whole.
pub(crate) fn class_name<'f>(tree: &RecordTree<'f>) -> &'f str {
    let name = tree.name();
    name.strip_prefix(tree.type_name())
        .and_then(|rest| rest.strip_prefix('.'))
        .filter(|rest| !rest.is_empty())
        .unwrap_or(name)
}

/// Returns the struct the field `name` of `parent` holds.
pub(crate) fn structure<'t, 'f>(
    parent: Option<&'t StructValue<'f>>,
    name: &str,
) -> Option<&'t StructValue<'f>> {
    match parent?.field(name)? {
        Value::Struct(value) => Some(value),
        _ => None,
    }
}

/// Returns the first element of the `Components` array of `tree` whose struct is `type_name`.
pub(crate) fn component<'t, 'f>(
    tree: &'t RecordTree<'f>,
    type_name: &str,
) -> Option<&'t StructValue<'f>> {
    let Some(Value::Array(components)) = tree.field("Components") else {
        return None;
    };
    components.iter().find_map(|component| match component {
        Value::Struct(value) if value.type_name() == type_name => Some(value),
        _ => None,
    })
}

/// Returns the elements of the array the field `name` of `parent` holds, none for a field that is
/// absent or no array.
pub(crate) fn elements<'t, 'f>(parent: Option<&'t StructValue<'f>>, name: &str) -> &'t [Value<'f>] {
    match parent.and_then(|parent| parent.field(name)) {
        Some(Value::Array(items)) => items,
        _ => &[],
    }
}

/// Returns the structs among the elements of the array the field `name` of `parent` holds, in
/// stored order.
pub(crate) fn structs<'t, 'f>(
    parent: Option<&'t StructValue<'f>>,
    name: &str,
) -> impl Iterator<Item = &'t StructValue<'f>> + use<'t, 'f> {
    elements(parent, name)
        .iter()
        .filter_map(|value| match value {
            Value::Struct(value) => Some(value),
            _ => None,
        })
}

/// Returns the GUIDs of the Records the References among the elements of the array the field
/// `name` of `parent` holds link to, in stored order.
pub(crate) fn references(parent: Option<&StructValue<'_>>, name: &str) -> Vec<CatalogGuid> {
    elements(parent, name)
        .iter()
        .filter_map(record_reference)
        .collect()
}

/// Returns the first struct of type `type_name` among the elements of the array the field `name`
/// of `parent` holds.
pub(crate) fn element<'t, 'f>(
    parent: Option<&'t StructValue<'f>>,
    name: &str,
    type_name: &str,
) -> Option<&'t StructValue<'f>> {
    elements(parent, name)
        .iter()
        .find_map(|element| match element {
            Value::Struct(value) if value.type_name() == type_name => Some(value),
            _ => None,
        })
}

/// Returns the localisation key the field `name` of `parent` holds; an empty key is none.
pub(crate) fn locale_key<'f>(parent: Option<&StructValue<'f>>, name: &str) -> Option<&'f str> {
    match parent?.field(name)? {
        Value::Locale(key) if !key.is_empty() => Some(*key),
        _ => None,
    }
}

/// Returns the string or enum option the field `name` of `parent` holds; an empty one is none.
pub(crate) fn text<'f>(parent: Option<&StructValue<'f>>, name: &str) -> Option<&'f str> {
    match parent?.field(name)? {
        Value::String(text) | Value::Enum(text) if !text.is_empty() => Some(*text),
        _ => None,
    }
}

/// Returns the integer the field `name` of `parent` holds, `None` for one beyond `i64`.
pub(crate) fn integer(parent: Option<&StructValue<'_>>, name: &str) -> Option<i64> {
    match *parent?.field(name)? {
        Value::Int8(value) => Some(value.into()),
        Value::Int16(value) => Some(value.into()),
        Value::Int32(value) => Some(value.into()),
        Value::Int64(value) => Some(value),
        Value::UInt8(value) => Some(value.into()),
        Value::UInt16(value) => Some(value.into()),
        Value::UInt32(value) => Some(value.into()),
        Value::UInt64(value) => i64::try_from(value).ok(),
        _ => None,
    }
}

/// Returns the boolean the field `name` of `parent` holds.
pub(crate) fn boolean(parent: Option<&StructValue<'_>>, name: &str) -> Option<bool> {
    match *parent?.field(name)? {
        Value::Bool(value) => Some(value),
        _ => None,
    }
}

/// Returns the finite 32-bit float the field `name` of `parent` holds.
pub(crate) fn single(parent: Option<&StructValue<'_>>, name: &str) -> Option<f32> {
    match *parent?.field(name)? {
        Value::Single(value) if value.is_finite() => Some(value),
        _ => None,
    }
}

/// Returns the finite number the field `name` of `parent` holds, whether stored as a float or as
/// an integer.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "a catalog number is a 32-bit float; larger integers lose their low digits"
)]
pub(crate) fn number(parent: Option<&StructValue<'_>>, name: &str) -> Option<f32> {
    match *parent?.field(name)? {
        Value::Single(value) if value.is_finite() => Some(value),
        Value::Double(value) => Some(value as f32).filter(|narrowed| narrowed.is_finite()),
        _ => integer(parent, name).map(|value| value as f32),
    }
}

/// Returns the GUID value the field `name` of `parent` holds.
pub(crate) fn guid_value(parent: Option<&StructValue<'_>>, name: &str) -> Option<Guid> {
    match *parent?.field(name)? {
        Value::Guid(guid) => Some(guid),
        _ => None,
    }
}

/// Returns the GUID of the Record the Reference in the field `name` of `parent` links to.
pub(crate) fn reference(parent: Option<&StructValue<'_>>, name: &str) -> Option<CatalogGuid> {
    record_reference(parent?.field(name)?)
}

/// Returns the GUID of the Record the field `name` of `parent` links to, including a Reference to
/// a Record the DataForge does not have.
pub(crate) fn linked(parent: Option<&StructValue<'_>>, name: &str) -> Option<CatalogGuid> {
    link_of(parent?.field(name)?)
}

/// Returns the GUIDs of the Records the References among the elements of the array the field
/// `name` of `parent` holds link to, including References to Records the DataForge does not
/// have, in stored order.
pub(crate) fn linked_list(parent: Option<&StructValue<'_>>, name: &str) -> Vec<CatalogGuid> {
    elements(parent, name).iter().filter_map(link_of).collect()
}

fn link_of(value: &Value<'_>) -> Option<CatalogGuid> {
    record_reference(value).or_else(|| match value {
        Value::Error(error) => error.dangling_guid().map(CatalogGuid),
        _ => None,
    })
}

/// Returns the GUID of the Record `value` links to, `None` for a value that is no Reference or
/// links into a Record rather than to it.
pub(crate) fn record_reference(value: &Value<'_>) -> Option<CatalogGuid> {
    match value {
        Value::Reference(reference) if reference.location().is_none() => {
            Some(CatalogGuid(reference.guid()))
        }
        _ => None,
    }
}

/// Returns `guid` unless it is the nil GUID, which the Archive writes for an unset identifier.
pub(crate) fn nonnil_guid(guid: Guid) -> Option<Guid> {
    (guid != Guid::default()).then_some(guid)
}

/// Returns the non-nil `id` of the struct `structure`, the identity of a contract or sub contract.
pub(crate) fn identity(structure: &StructValue<'_>) -> Option<Guid> {
    guid_value(Some(structure), "id").and_then(nonnil_guid)
}
