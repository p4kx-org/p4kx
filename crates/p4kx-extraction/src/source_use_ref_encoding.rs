// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;

use crate::source_inventory::PhysicalSourceRef;
use crate::source_use::{
    InputScope, LocationView, SourceLocation, SourceUseRef, SourceUseRole, Subresource,
};

pub(crate) fn encode(
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
        LocationView::Original(original) => append_location(&mut key, original),
        LocationView::Subresource(site) => append_subresource(&mut key, &site),
    }
    SourceUseRef::registered(key)
}

fn append_location(key: &mut String, location: &SourceLocation) {
    match location {
        SourceLocation::Opaque { reason } => {
            let _written = write!(key, "o{}", reason.key());
        }
        SourceLocation::DataForge {
            record_index,
            field,
        } => {
            let _written = write!(key, "d{record_index}:{field}");
        }
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
        ),
        SourceLocation::Metadata { field } => {
            key.push('m');
            key.push_str(field);
        }
    }
}

fn append_subresource(key: &mut String, site: &Subresource<'_>) {
    let Subresource {
        namespace,
        indices,
        field,
    } = site;
    if *namespace == "xml" && append_xml_location(key, indices, field) {
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
    if let Some((&first, rest)) = indices.split_first() {
        append_coordinate(key, first);
        for &index in rest {
            key.push('.');
            append_coordinate(key, index);
        }
    }
    true
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
