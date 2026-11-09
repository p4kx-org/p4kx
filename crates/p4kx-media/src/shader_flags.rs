// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_shader::{Definition, PropertyFieldKind, RootFieldKind};

use crate::shader_lookup::ShaderLookup;
use crate::{InputReceipt, MediaError, MediaWriter, Subresource};

pub(crate) struct NamedProperty {
    pub(crate) name: String,
    pub(crate) target: Subresource,
    pub(crate) name_ordinal: u64,
}

pub(crate) struct ShaderRead {
    pub(crate) receipt: Option<InputReceipt>,
    pub(crate) properties: Vec<NamedProperty>,
    pub(crate) supported: bool,
    pub(crate) unknown_names: bool,
    pub(crate) common_flags: bool,
}

pub(crate) fn project(
    definition: &Definition<'_>,
    receipt: InputReceipt,
    lookup: &ShaderLookup<'_>,
    writer: &MediaWriter<'_>,
) -> Result<ShaderRead, MediaError> {
    let mut read = ShaderRead {
        receipt: Some(receipt),
        properties: Vec::new(),
        supported: true,
        unknown_names: false,
        common_flags: definition
            .fields_of(RootFieldKind::UsesCommonGlobalFlags)
            .next()
            .is_some(),
    };
    let mut properties = definition.properties().peekable();
    for (index, field) in definition.fields().enumerate() {
        lookup.work(1, writer)?;
        let Some(property) =
            properties.next_if(|property| property.declaration().span == field.declaration().span)
        else {
            continue;
        };
        for (name_ordinal, name) in property
            .fields()
            .enumerate()
            .filter(|(_, field)| field.property_kind() == Some(PropertyFieldKind::Name))
        {
            let Some(name) = name.literal().and_then(symbol) else {
                read.unknown_names = true;
                continue;
            };
            lookup.work(name.len().saturating_add(1), writer)?;
            read.properties
                .try_reserve(1)
                .map_err(|cause| MediaError::Io {
                    path: writer.root.clone(),
                    source: std::io::Error::new(std::io::ErrorKind::OutOfMemory, cause),
                })?;
            read.properties.push(NamedProperty {
                name: name.to_owned(),
                name_ordinal: crate::shader_values::count(name_ordinal)?,
                target: Subresource::new(
                    crate::shader_projection::PROPERTY_NAMESPACE,
                    vec![crate::shader_values::count(index)?],
                ),
            });
        }
    }
    Ok(read)
}

pub(crate) fn symbol(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = if let Some(quoted) = value.strip_prefix('"') {
        quoted.strip_suffix('"')?
    } else {
        value
    };
    let value = value.strip_prefix('%').unwrap_or(value);
    (!value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
    .then_some(value)
}
