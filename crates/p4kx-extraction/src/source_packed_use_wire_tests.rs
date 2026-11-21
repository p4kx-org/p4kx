// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::source_inventory::{PhysicalSourceRef, SourceEncryptionState};
use crate::source_use::storage::RetainedLocation;
use crate::source_use::{
    InputScope, LocationView, SourceLocation, SourceUseDescription, SourceUseRole,
};
use crate::source_use_ref_encoding;

use super::{PackedSourceUse, PackedUseFacts};

#[test]
fn packed_use_storage_preserves_every_scope_role_and_encryption()
-> Result<(), Box<dyn std::error::Error>> {
    let locations = packed_locations();
    let scopes = [
        InputScope::Payload,
        InputScope::ArchiveDirectoryMetadata,
        InputScope::FileMetadata,
    ];
    let roles = [
        SourceUseRole::Payload,
        SourceUseRole::Selector,
        SourceUseRole::Conversion,
        SourceUseRole::Attempted,
        SourceUseRole::Reference,
    ];
    let encryption_states = [
        SourceEncryptionState::Encrypted,
        SourceEncryptionState::Unencrypted,
        SourceEncryptionState::Unknown,
    ];
    let physical = PhysicalSourceRef::new("synthetic-\u{54c1}\u{8cea}");
    let mut sources = BTreeSet::new();
    for location in &locations {
        for scope in scopes {
            for role in roles {
                for encryption in encryption_states {
                    let original = describe(&physical, location, scope, role, encryption);
                    let id = original.id().registered_text().ok_or("registered ID")?;
                    let packed =
                        PackedSourceUse::pack(&original, &mut sources).ok_or("packed use")?;
                    let restored = packed.restore(id);
                    assert_eq!(
                        serde_json::to_vec(&restored)?,
                        serde_json::to_vec(&original)?,
                        "packing preserves authored coordinates, scope, role and encryption",
                    );
                    assert!(
                        Arc::ptr_eq(id, restored.id().registered_text().ok_or("restored ID")?),
                        "restoration retains the canonical registered ID allocation",
                    );
                    let contribution = packed.contribution(id);
                    assert_eq!(contribution.role, role, "packed role");
                    assert_eq!(
                        contribution.encryption, encryption,
                        "packed effective encryption"
                    );
                }
            }
        }
    }
    assert_eq!(
        sources.len(),
        1,
        "every use shares the original physical source"
    );
    Ok(())
}

#[test]
fn out_of_range_record_keeps_its_original_unpacked_coordinate() {
    let record = u64::from(u32::MAX) + 1;
    let physical = PhysicalSourceRef::new("synthetic-source");
    let locations = [
        subresource("dataforgeRecordHeader", vec![record], "guid"),
        subresource("dataforgeRecordDeclarations", vec![record], "$fields"),
        subresource("dataforgeValue", vec![record, u64::MAX], "$value"),
        subresource("custom", vec![record], "value"),
    ];
    for location in &locations {
        let original = describe(
            &physical,
            location,
            InputScope::Payload,
            SourceUseRole::Payload,
            SourceEncryptionState::Unknown,
        );
        let mut sources = BTreeSet::new();
        assert!(
            PackedSourceUse::pack(&original, &mut sources).is_none(),
            "unsupported compact coordinates remain live"
        );
        assert_eq!(
            original.location.original(),
            location,
            "the full coordinate is retained"
        );
    }
}

#[test]
#[cfg(target_pointer_width = "64")]
fn packed_use_entry_layout_fits_forty_bytes() {
    assert_eq!(
        size_of::<PackedUseFacts>(),
        2,
        "logical use metadata occupies two bytes"
    );
    assert_eq!(
        size_of::<(Arc<str>, PackedSourceUse)>(),
        40,
        "packed key and use layout"
    );
}

fn packed_locations() -> [SourceLocation; 6] {
    let record = u64::from(u32::MAX);
    [
        SourceLocation::DataForge {
            record_index: 0,
            field: String::new(),
        },
        SourceLocation::DataForge {
            record_index: u32::MAX,
            field: "array[23].m\u{00fc}ller:\u{54c1}\u{8cea}\nvalue".to_owned(),
        },
        subresource("dataforgeRecordHeader", vec![record], "metadataTag"),
        subresource("dataforgeRecordDeclarations", vec![record], "$fields"),
        subresource("dataforgeValue", vec![0, 0], "$value"),
        subresource("dataforgeValue", vec![record, u64::MAX], "$error.guid"),
    ]
}

fn describe(
    physical: &PhysicalSourceRef,
    location: &SourceLocation,
    scope: InputScope,
    role: SourceUseRole,
    effective_encryption: SourceEncryptionState,
) -> SourceUseDescription {
    let id =
        source_use_ref_encoding::encode(physical, scope, role, LocationView::Original(location));
    let location = RetainedLocation::new(location.clone(), Some(&id), None);
    SourceUseDescription {
        id,
        source: physical.clone(),
        scope,
        role,
        location,
        effective_encryption,
    }
}

fn subresource(namespace: &str, indices: Vec<u64>, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices,
        field: field.to_owned(),
    }
}
