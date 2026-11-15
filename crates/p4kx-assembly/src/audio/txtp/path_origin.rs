// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io;
use std::ops::Range;
use std::ptr;

use p4kx_extraction::{
    Archive, InputScope, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses,
};

use crate::audio::hirc_path_source::{
    HircChildFields, HircChildOwner, HircChildSource, HircRouteSource,
};

pub(super) fn children(
    archive: &Archive,
    sources: &SourceUses,
    children: &[&HircChildSource],
    actual: &mut Vec<SourceUse>,
) -> io::Result<()> {
    let mut scopes = HashMap::new();
    for child in children {
        let owner = child.owner();
        let Some(fields) = owner.child_fields() else {
            continue;
        };
        let scope = match scopes.entry(ptr::from_ref(owner)) {
            Entry::Occupied(existing) => existing.into_mut(),
            Entry::Vacant(new) => {
                new.insert(child_owner(archive, sources, owner, &fields, actual)?)
            }
        };
        if let Some(scope) = scope {
            actual.push(positioned_use(
                scope,
                fields.namespace,
                fields.id,
                child.id_bytes(),
                Some(child.ordinal()),
                SourceUseRole::Selector,
            )?);
        }
    }
    Ok(())
}

fn child_owner(
    archive: &Archive,
    sources: &SourceUses,
    owner: &HircChildOwner,
    fields: &HircChildFields,
    actual: &mut Vec<SourceUse>,
) -> io::Result<Option<SourceScope>> {
    use SourceUseRole::Selector;

    let Some(entry) = owner.entry(archive) else {
        return Ok(None);
    };
    let scope = sources
        .observe(SourceInput::ArchiveView(entry), InputScope::Payload)
        .map_err(io::Error::other)?;
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
    version_use(&scope, owner.version_bytes(), actual)?;
    if let Some(range) = owner.random_mode_bytes() {
        actual.push(source_use(
            &scope,
            fields.namespace,
            "randomSequenceMode",
            range,
            Selector,
        )?);
    }
    Ok(Some(scope))
}

pub(super) fn route(
    archive: &Archive,
    sources: &SourceUses,
    route: &HircRouteSource,
    actual: &mut Vec<SourceUse>,
) -> io::Result<()> {
    let Some(entry) = route.entry(archive) else {
        return Ok(());
    };
    let scope = sources
        .observe(SourceInput::ArchiveView(entry), InputScope::Payload)
        .map_err(io::Error::other)?;
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
    version_use(&scope, route.version_bytes(), actual)
}

fn version_use(
    scope: &SourceScope,
    version: Option<Range<usize>>,
    actual: &mut Vec<SourceUse>,
) -> io::Result<()> {
    if let Some(range) = version {
        let range = offset(range.start)?..offset(range.end)?;
        actual.push(source_use(
            scope,
            "bnkBkhd",
            "version",
            range,
            SourceUseRole::Selector,
        )?);
    }
    Ok(())
}

pub(super) fn source_use(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    range: Range<u64>,
    role: SourceUseRole,
) -> io::Result<SourceUse> {
    positioned_use(scope, namespace, field, range, None, role)
}

fn positioned_use(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    range: Range<u64>,
    ordinal: Option<usize>,
    role: SourceUseRole,
) -> io::Result<SourceUse> {
    let mut indices = vec![range.start, range.end];
    if let Some(ordinal) = ordinal {
        indices.push(offset(ordinal)?);
    }
    scope
        .at(
            SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices,
                field: field.to_owned(),
            },
            role,
        )
        .map_err(io::Error::other)
}

pub(super) fn offset(position: usize) -> io::Result<u64> {
    u64::try_from(position).map_err(io::Error::other)
}
