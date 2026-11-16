// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::source_use::{InputScope, SourceLocation, SourceUse, SourceUseRole};
use crate::{SourceInput, SourceUses};

use super::{LocationView, TestResult, subresource, verify_bytes, verify_roles};

#[test]
fn full_subresource_uses_share_exact_text_and_keep_wire_bytes() -> TestResult {
    let uses = SourceUses::new();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let ordinal = SourceUses::with_compact_use_refs();
    let ordinal_scope = ordinal.observe(SourceInput::Unknown, InputScope::Payload)?;
    for (namespace, indices, field) in [
        ("animation-byte-range", &[0, u64::MAX][..], "sampleChunk"),
        ("bnkHircField", &[23, 99], "value"),
        ("unicode:ü", &[3], "wörter:指\n0"),
        ("", &[], ""),
    ] {
        let location = subresource(namespace, indices, field);
        let payload = scope.at(location.clone(), SourceUseRole::Payload)?;
        let selector = scope.at_observed_location(&payload, SourceUseRole::Selector)?;
        for (used, prefix) in [
            (payload, "u2:u0:payload:payload:"),
            (selector, "u2:u0:payload:selector:"),
        ] {
            verify_full_subresource_backing(&used, prefix, &location, namespace, field)?;
        }
        verify_roles(&uses, &scope, &location, true)?;
        verify_roles(&ordinal, &ordinal_scope, &location, false)?;
    }
    verify_roles(
        &uses,
        &scope,
        &subresource("bnkHircField", &[1, 2, 3], "value"),
        false,
    )
}

fn verify_full_subresource_backing(
    used: &SourceUse,
    prefix: &str,
    location: &SourceLocation,
    namespace: &str,
    field: &str,
) -> TestResult {
    let id = used.reference().as_str();
    let key = id
        .strip_prefix(prefix)
        .ok_or("original full source, scope and role ID")?;
    assert_eq!(
        key,
        LocationView::Original(location).key(),
        "the ID retains its original full framing and exact coordinates"
    );
    let (_, namespace_and_rest) = key.split_once(':').ok_or("original namespace prefix")?;
    let namespace_in_id = namespace_and_rest
        .get(..namespace.len())
        .ok_or("namespace byte extent")?;
    let field_start = id.strip_suffix(field).ok_or("original field suffix")?.len();
    let field_in_id = id.get(field_start..).ok_or("field suffix extent")?;
    let LocationView::Subresource(site) = used.description().location.view() else {
        return Err("full subresource retains ID-backed text".into());
    };
    assert!(
        std::ptr::eq(site.namespace, namespace_in_id) && std::ptr::eq(site.field, field_in_id),
        "namespace and field borrow this role's exact reference string"
    );
    verify_bytes(used, location, Some(false))
}
