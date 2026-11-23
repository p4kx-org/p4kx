// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::source_inventory::PhysicalSourceRef;
use crate::source_use::{
    InputScope, LocationView, SourceLocation, SourceUseRef, SourceUseRole, Subresource,
};

pub(crate) const RECORD_PATH_TEXT_RANGES: &str = "dataforgeRecordPathTextRanges";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoordinateEncoding {
    Standard,
    FieldStatistics,
}

#[cfg(test)]
pub(crate) fn encode(
    source: &PhysicalSourceRef,
    scope: InputScope,
    role: SourceUseRole,
    location: LocationView<'_>,
) -> SourceUseRef {
    encode_with(CoordinateEncoding::Standard, source, scope, role, location)
}

pub(crate) fn encode_with(
    encoding: CoordinateEncoding,
    source: &PhysicalSourceRef,
    scope: InputScope,
    role: SourceUseRole,
    location: LocationView<'_>,
) -> SourceUseRef {
    let scope = match scope {
        InputScope::Payload => 'p',
        InputScope::ArchiveDirectoryMetadata => 'd',
        InputScope::FileMetadata => 'f',
    };
    let role = match role {
        SourceUseRole::Payload => 'p',
        SourceUseRole::Selector => 's',
        SourceUseRole::Conversion => 'c',
        SourceUseRole::Attempted => 'a',
        SourceUseRole::Reference => 'r',
    };

    // Lengths count UTF-8 bytes. Numeric tokens end at a fixed separator.
    let mut key = String::from("c");
    append_counted(&mut key, source.as_str());
    let _written = write!(key, "{scope}{role}");
    match location {
        LocationView::Original(original) => append_location(&mut key, original, encoding),
        LocationView::DataForgeField(site) => {
            append_dataforge(&mut key, site.record_index, site.field);
        }
        LocationView::Subresource(site) => append_subresource(&mut key, &site, encoding),
    }
    SourceUseRef::registered(key)
}

fn append_dataforge(key: &mut String, record_index: u32, field: &str) {
    let _written = write!(key, "d{record_index}:{field}");
}

fn append_location(key: &mut String, location: &SourceLocation, encoding: CoordinateEncoding) {
    match location {
        SourceLocation::Opaque { reason } => {
            let _written = write!(key, "o{}", reason.key());
        }
        SourceLocation::DataForge {
            record_index,
            field,
        } => append_dataforge(key, *record_index, field),
        SourceLocation::LocalizationLookup { language, selector } => {
            key.push('q');
            append_counted(key, language.code());
            key.push_str(selector.as_str());
        }
        SourceLocation::ArchiveDirectoryPaths {
            start_entry_index,
            end_entry_index,
        } => {
            let _written = write!(
                key,
                "a{}:{}",
                start_entry_index.get(),
                end_entry_index.get()
            );
        }
        SourceLocation::ArchiveDirectoryTreePaths {
            start_entry_index,
            end_entry_index,
        } => {
            let _written = write!(
                key,
                "t{}:{}",
                start_entry_index.get(),
                end_entry_index.get()
            );
        }
        SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } => append_subresource(
            key,
            &Subresource {
                namespace,
                indices,
                field,
            },
            encoding,
        ),
        SourceLocation::Metadata { field } => {
            key.push('m');
            key.push_str(field);
        }
    }
}

fn append_subresource(key: &mut String, site: &Subresource<'_>, encoding: CoordinateEncoding) {
    if encoding == CoordinateEncoding::FieldStatistics
        && let Some(tag) = statistics_location_tag(site)
    {
        key.push('g');
        key.push(tag);
        append_coordinates(key, site.indices);
        return;
    }
    let Subresource {
        namespace,
        indices,
        field,
    } = site;
    if *namespace == RECORD_PATH_TEXT_RANGES && *field == "text" {
        append_path_text_ranges(key, indices);
        return;
    }
    if (*namespace == "xml" && append_xml_location(key, indices, field))
        || append_physical_xml_location(key, namespace, indices, field)
    {
        return;
    }
    key.push('s');
    append_namespace(key, namespace);
    for index in *indices {
        let _written = write!(key, "{index}.");
    }
    key.push(':');
    key.push_str(field);
}

fn statistics_location_tag(site: &Subresource<'_>) -> Option<char> {
    // Each tag fixes the namespace, field and number of encoded unsigned coordinates.
    Some(match (site.namespace, site.field, site.indices) {
        ("dataforgePropertyDefinition", "instanceLayout", [_]) => 'p',
        ("dataforgePropertyDefinition", "targetedInstanceLayout", [_]) => 'q',
        ("dataforgePropertyDefinition", "name", [_]) => 'n',
        ("dataforgeStructDefinition", "firstProperty", [_]) => 'f',
        ("dataforgeStructDefinition", "parent", [_]) => 'h',
        ("dataforgeStructDefinition", "propertyCount", [_]) => 'c',
        ("dataforgeStructDefinition", "size", [_]) => 'z',
        ("dataforgeStructDefinition", "sourceType", [_]) => 'y',
        ("dataforgeDecodedBytes", "arrayHeader", [_, _]) => 'a',
        ("dataforgeDecodedBytes", "pointer", [_, _]) => 'r',
        ("dataforgeDecodedBytes", "slot", [_, _]) => 's',
        ("dataforgeDecodedBytes", "text", [_, _]) => 't',
        ("dataforgeDecodedBytes", "textTerminator", [_, _]) => 'e',
        ("dataforgeInstance", "selection", [_, _]) => 'i',
        _ => return None,
    })
}

fn append_path_text_ranges(key: &mut String, indices: &[u64]) {
    let mut digest = Sha256::new();
    for index in indices {
        digest.update(index.to_le_bytes());
    }
    key.push('b');
    for byte in digest.finalize() {
        let _written = write!(key, "{byte:02x}");
    }
}

fn append_xml_location(key: &mut String, indices: &[u64], field: &str) -> bool {
    let tag = match field {
        "$name" => 'n',
        "@pos" => 'p',
        "@rot" => 'r',
        "$absent:@pos" => 'a',
        "$absent:@rot" => 'b',
        _ => return false,
    };
    key.push('x');
    key.push(tag);
    append_coordinates(key, indices);
    true
}

fn append_physical_xml_location(
    key: &mut String,
    namespace: &str,
    indices: &[u64],
    field: &str,
) -> bool {
    let namespace_tag = match (namespace, indices) {
        ("cryxmlNode", [node]) if u32::try_from(*node).is_ok() => 'n',
        ("cryxmlAttribute", [node, attribute])
            if u32::try_from(*node).is_ok() && u32::try_from(*attribute).is_ok() =>
        {
            'a'
        }
        ("textXmlElement", [start, end]) if start < end => 'e',
        ("textXmlAttribute", [start, end]) if start < end => 't',
        _ => return false,
    };
    let field_tag = match field {
        "$attributes" => 'a',
        "$children" => 'c',
        "$name" => 'n',
        "$node" => 'o',
        "name" => 'i',
        "skipPart" => 's',
        _ => return false,
    };
    key.push('y');
    key.push(namespace_tag);
    key.push(field_tag);
    append_coordinates(key, indices);
    true
}

fn append_coordinates(key: &mut String, indices: &[u64]) {
    if let Some((&first, rest)) = indices.split_first() {
        append_coordinate(key, first);
        for &index in rest {
            key.push('.');
            append_coordinate(key, index);
        }
    }
}

fn append_coordinate(key: &mut String, index: u64) {
    const DIGIT_BITS: u32 = 6;
    const DIGIT_MASK: u8 = (1 << DIGIT_BITS) - 1;
    let higher = index >> DIGIT_BITS;
    if higher != 0 {
        append_coordinate(key, higher);
    }
    let [least_significant_byte, ..] = index.to_le_bytes();
    let character = match least_significant_byte & DIGIT_MASK {
        digit @ 0..=25 => b'A' + digit,
        digit @ 26..=51 => b'a' + (digit - 26),
        digit @ 52..=61 => b'0' + (digit - 52),
        62 => b'-',
        _ => b'_',
    };
    key.push(char::from(character));
}

fn append_counted(key: &mut String, value: &str) {
    let _written = write!(key, "{}:{value}", value.len());
}

fn append_namespace(key: &mut String, namespace: &str) {
    let tag = match namespace {
        "dataforgeValue" => 'v',
        "dataforgeRecordHeader" => 'h',
        "dataforgeStruct" => 's',
        "xml" => 'x',
        _ => {
            key.push('?');
            append_counted(key, namespace);
            return;
        }
    };
    key.push(tag);
}

#[cfg(test)]
#[path = "source_use_ref_encoding_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "source_use_ref_encoding_statistics_tests.rs"]
mod statistics_tests;
