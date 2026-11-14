// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::cryxml::{SourceAttributePosition, SourceElementPosition};
use p4kx_extraction::{
    EntryOrigin, InputScope, SourceInput, SourceLocation, SourceScope, SourceUses,
};

pub(crate) fn scope(
    uses: &SourceUses,
    origin: Option<&EntryOrigin>,
) -> io::Result<(SourceScope, bool)> {
    let input = origin.map_or(SourceInput::Unknown, SourceInput::Archive);
    let scope = uses
        .observe(input, InputScope::Payload)
        .map_err(io::Error::other)?;
    let complete = origin.is_some_and(|entry| entry.ancestors().all(|ancestor| ancestor.is_some()));
    Ok((scope, complete))
}

pub(crate) fn attribute_location(
    position: &SourceAttributePosition,
    field: &str,
) -> io::Result<SourceLocation> {
    match position {
        SourceAttributePosition::Text(range) => Ok(location(
            "textXmlAttribute",
            vec![range.start as u64, range.end as u64],
            field,
        )),
        SourceAttributePosition::CryXml {
            node_index,
            attribute_index,
        } => Ok(location(
            "cryxmlAttribute",
            vec![u64::from(*node_index), u64::from(*attribute_index)],
            field,
        )),
        _ => Err(io::Error::other(
            "unrecognized action-map attribute coordinate",
        )),
    }
}

pub(crate) fn node_location(
    position: &SourceElementPosition,
    field: &str,
) -> io::Result<SourceLocation> {
    match position {
        SourceElementPosition::Text(range) => Ok(location(
            "textXmlElement",
            vec![range.start as u64, range.end as u64],
            field,
        )),
        SourceElementPosition::CryXml(index) => {
            Ok(location("cryxmlNode", vec![u64::from(*index)], field))
        }
        _ => Err(io::Error::other(
            "unrecognized action-map element coordinate",
        )),
    }
}

pub(crate) fn location(namespace: &str, indices: Vec<u64>, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices,
        field: field.to_owned(),
    }
}
