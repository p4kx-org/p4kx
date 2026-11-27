// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::Range;
use std::ptr;

use p4kx_extraction::{
    Archive, InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
};

use super::hirc_path_source::{HircChildFields, HircChildOwner, HircChildSource};
use super::hirc_route_source::HircRouteSource;

struct ChildProof {
    scope: SourceScope,
    complete: bool,
}

pub(super) fn children(
    archive: &Archive,
    sources: &SourceUses,
    children: &[&HircChildSource],
    actual: &mut Vec<SourceUse>,
) -> Result<(), SourceEvidenceError> {
    let mut proofs = HashMap::new();
    for child in children {
        let owner = child.owner();
        let Some(fields) = owner.child_fields() else {
            continue;
        };
        let proof = match proofs.entry(ptr::from_ref(owner)) {
            Entry::Occupied(existing) => existing.into_mut(),
            Entry::Vacant(new) => {
                new.insert(child_owner(archive, sources, owner, &fields, actual)?)
            }
        };
        if let Some(proof) = proof {
            actual.push(child_use(&proof.scope, &fields, child)?);
        }
    }
    Ok(())
}

pub(super) fn child(
    archive: &Archive,
    sources: &SourceUses,
    child: &HircChildSource,
    actual: &mut Vec<SourceUse>,
) -> Result<bool, SourceEvidenceError> {
    let owner = child.owner();
    let Some(fields) = owner.child_fields() else {
        return Ok(false);
    };
    let Some(proof) = child_owner(archive, sources, owner, &fields, actual)? else {
        return Ok(false);
    };
    actual.push(child_use(&proof.scope, &fields, child)?);
    Ok(proof.complete)
}

fn child_use(
    scope: &SourceScope,
    fields: &HircChildFields,
    child: &HircChildSource,
) -> Result<SourceUse, SourceEvidenceError> {
    positioned_use(
        scope,
        fields.namespace,
        fields.id,
        child.id_bytes(),
        Some(child.ordinal()),
        SourceUseRole::Selector,
    )
}

fn child_owner(
    archive: &Archive,
    sources: &SourceUses,
    owner: &HircChildOwner,
    fields: &HircChildFields,
    actual: &mut Vec<SourceUse>,
) -> Result<Option<ChildProof>, SourceEvidenceError> {
    use SourceUseRole::Selector;

    let Some(entry) = owner.entry(archive) else {
        return Ok(None);
    };
    let scope = sources.observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
    actual.push(source_use(
        &scope,
        "bnkHircObject",
        "typeCode",
        owner.kind_bytes(),
        Selector,
    )?);
    actual.push(source_use(
        &scope,
        fields.namespace,
        fields.count,
        owner.child_count_bytes(),
        Selector,
    )?);
    let complete = version_use(&scope, owner.version_bytes(), actual)?
        && entry.ancestors().all(|ancestor| ancestor.is_some());
    if let Some(range) = owner.random_mode_bytes() {
        actual.push(source_use(
            &scope,
            fields.namespace,
            "randomSequenceMode",
            range,
            Selector,
        )?);
    }
    Ok(Some(ChildProof { scope, complete }))
}

pub(super) fn route(
    archive: &Archive,
    sources: &SourceUses,
    route: &HircRouteSource,
    actual: &mut Vec<SourceUse>,
) -> Result<bool, SourceEvidenceError> {
    let Some(entry) = route.entry(archive) else {
        return Ok(false);
    };
    let scope = sources.observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
    for word in route.words() {
        actual.push(positioned_use(
            &scope,
            word.namespace,
            word.field,
            word.bytes.clone(),
            word.ordinal,
            SourceUseRole::Selector,
        )?);
    }
    Ok(version_use(&scope, route.version_bytes(), actual)?
        && entry.ancestors().all(|ancestor| ancestor.is_some()))
}

pub(super) fn version_use(
    scope: &SourceScope,
    version: Option<Range<usize>>,
    actual: &mut Vec<SourceUse>,
) -> Result<bool, SourceEvidenceError> {
    let Some(range) = version else {
        return Ok(false);
    };
    actual.push(source_use(
        scope,
        "bnkBkhd",
        "version",
        range.start as u64..range.end as u64,
        SourceUseRole::Selector,
    )?);
    Ok(true)
}

pub(super) fn source_use(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    range: Range<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    positioned_use(scope, namespace, field, range, None, role)
}

fn positioned_use(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    range: Range<u64>,
    ordinal: Option<usize>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    let mut indices = vec![range.start, range.end];
    indices.extend(ordinal.map(|index| index as u64));
    scope.at(
        SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices,
            field: field.to_owned(),
        },
        role,
    )
}
