// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::portal_alias::{AliasUncertainty, DocumentGrant, UnknownAlias};

pub(crate) const MAX_PATH_BYTES: usize = 16 * 1024;
const MAX_DOCUMENT_ID_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DocumentPath {
    pub(crate) id: String,
    pub(crate) name: Vec<u8>,
    pub(crate) suffix: Vec<Vec<u8>>,
}

pub(crate) fn document_path(path: &[u8], mount: &[u8]) -> Result<DocumentPath, UnknownAlias> {
    if path.len() > MAX_PATH_BYTES || !path.starts_with(b"/") || path.contains(&0) {
        return Err(UnknownAlias::new(AliasUncertainty::DocumentPath));
    }
    let relative = strip_mount(path, mount)
        .ok_or_else(|| UnknownAlias::new(AliasUncertainty::UntrustedMount))?;
    let mut components = relative.split(|byte| *byte == b'/');
    let id = components.next().unwrap_or_default();
    if id.is_empty() || id.len() > MAX_DOCUMENT_ID_BYTES || !id.iter().all(u8::is_ascii_hexdigit) {
        return Err(UnknownAlias::new(AliasUncertainty::DocumentPath));
    }
    let name = components
        .next()
        .filter(|component| valid_component(component))
        .ok_or_else(|| UnknownAlias::new(AliasUncertainty::DocumentPath))?;
    let mut suffix = Vec::new();
    for component in components {
        if !valid_component(component) {
            return Err(UnknownAlias::new(AliasUncertainty::DocumentPath));
        }
        suffix.push(component.to_vec());
    }
    let id = String::from_utf8(id.to_vec())
        .map_err(|error| UnknownAlias::with_source(AliasUncertainty::DocumentPath, error))?;
    Ok(DocumentPath {
        id,
        name: name.to_vec(),
        suffix,
    })
}

pub(crate) fn strip_mount<'path>(path: &'path [u8], mount: &[u8]) -> Option<&'path [u8]> {
    let remainder = path.strip_prefix(mount)?;
    if mount == b"/" {
        return Some(remainder);
    }
    remainder.strip_prefix(b"/")
}

pub(crate) fn bytestring_path(bytes: &[u8]) -> Result<&[u8], UnknownAlias> {
    let Some(path) = bytes.strip_suffix(&[0]) else {
        return Err(UnknownAlias::new(AliasUncertainty::MalformedReply));
    };
    if path.len() > MAX_PATH_BYTES {
        return Err(UnknownAlias::new(AliasUncertainty::Limit));
    }
    if !path.starts_with(b"/")
        || path.contains(&0)
        || path
            .split(|byte| *byte == b'/')
            .any(|component| component == b"." || component == b"..")
    {
        return Err(UnknownAlias::new(AliasUncertainty::MalformedReply));
    }
    Ok(path)
}

pub(crate) fn mapped_descendant(
    document: &DocumentPath,
    host: &[u8],
    grant: DocumentGrant,
) -> Result<Vec<u8>, UnknownAlias> {
    let root = bytestring_path(host)?;
    if root.rsplit(|byte| *byte == b'/').next() != Some(document.name.as_slice()) {
        return Err(UnknownAlias::new(AliasUncertainty::MalformedReply));
    }
    if grant == DocumentGrant::File && !document.suffix.is_empty() {
        return Err(UnknownAlias::new(AliasUncertainty::FileDescendant));
    }
    let mut result = root.to_vec();
    for component in &document.suffix {
        result.push(b'/');
        result.extend_from_slice(component);
    }
    if result.len() > MAX_PATH_BYTES {
        return Err(UnknownAlias::new(AliasUncertainty::Limit));
    }
    Ok(result)
}

fn valid_component(component: &[u8]) -> bool {
    !component.is_empty() && component != b"." && component != b".." && !component.contains(&0)
}
