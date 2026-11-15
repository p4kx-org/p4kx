// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;

use crate::source_inventory::PhysicalSourceRef;
use crate::source_use::{InputScope, SourceLocation, SourceUseRef, SourceUseRole};

pub(crate) fn encode(
    source: &PhysicalSourceRef,
    scope: InputScope,
    role: SourceUseRole,
    location: &SourceLocation,
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
    append_location(&mut key, location);
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
        } => {
            key.push('s');
            append_namespace(key, namespace);
            for index in indices {
                let _written = write!(key, "{index}.");
            }
            key.push(':');
            key.push_str(field);
        }
        SourceLocation::Metadata { field } => {
            key.push('m');
            key.push_str(field);
        }
    }
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
