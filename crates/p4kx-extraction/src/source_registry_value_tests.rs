// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use super::SourceRegistry;
use crate::source_use_ref_encoding::UseReferences;
use crate::{
    InputScope, PhysicalSourceRef, ProvenanceMode, SourceEncryptionState, SourceLocation,
    SourceUseRole,
};

#[test]
fn direct_marker_observations_consume_no_reference_ordinals() {
    let mut registry = SourceRegistry::with_mode(
        Arc::new(()),
        UseReferences::ordinal(),
        ProvenanceMode::MarkersOnly,
    );
    let source = PhysicalSourceRef::new("physical");
    let location = || SourceLocation::Metadata {
        field: "path".into(),
    };
    for role in [
        SourceUseRole::Payload,
        SourceUseRole::Selector,
        SourceUseRole::Conversion,
        SourceUseRole::Reference,
    ] {
        registry.origin_at(
            &source,
            InputScope::Payload,
            location(),
            role,
            SourceEncryptionState::Encrypted,
            true,
        );
    }
    let attempted = registry.register_use(
        &source,
        InputScope::Payload,
        location(),
        SourceUseRole::Attempted,
        SourceEncryptionState::Encrypted,
    );
    assert_eq!(
        attempted.reference().as_str(),
        "u0",
        "the first requested detailed reference gets the first ordinal"
    );
}
