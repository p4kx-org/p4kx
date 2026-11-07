// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::portal_alias::{AliasUncertainty, UnknownAlias};
use crate::portal_path;

pub(crate) const MAX_MOUNT_BYTES: usize = 512 * 1024;
const MAX_MOUNTS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PortalMount {
    identity: Vec<u8>,
    pub(crate) target: Vec<u8>,
    pub(crate) filesystem: Vec<u8>,
    pub(crate) source: Vec<u8>,
}

impl PortalMount {
    pub(crate) fn is_documents(&self) -> bool {
        self.filesystem == b"fuse.portal" && self.source == b"portal"
    }
}

pub(crate) fn covering_mount(
    input: &[u8],
    path: &[u8],
) -> Result<Option<PortalMount>, UnknownAlias> {
    if input.len() > MAX_MOUNT_BYTES {
        return Err(UnknownAlias::new(AliasUncertainty::Limit));
    }
    let mut selected: Option<PortalMount> = None;
    let mut count = 0_usize;
    let lines = input
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty());
    for line in lines {
        count += 1;
        if count > MAX_MOUNTS {
            return Err(UnknownAlias::new(AliasUncertainty::Limit));
        }
        let mount = parse_mount(line)?;
        if path != mount.target && portal_path::strip_mount(path, &mount.target).is_none() {
            continue;
        }
        if let Some(previous) = &selected {
            match mount.target.len().cmp(&previous.target.len()) {
                std::cmp::Ordering::Less => continue,
                std::cmp::Ordering::Equal => {
                    return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
                }
                std::cmp::Ordering::Greater => {}
            }
        }
        selected = Some(mount);
    }
    Ok(selected)
}

fn parse_mount(line: &[u8]) -> Result<PortalMount, UnknownAlias> {
    let mut fields = line.split(|byte| *byte == b' ');
    let mut target = None;
    for index in 0..6 {
        let field = next_field(&mut fields)?;
        if index == 4 {
            target = Some(decode_mount_field(field)?);
        }
    }
    loop {
        if next_field(&mut fields)? == b"-" {
            break;
        }
    }
    let filesystem = decode_mount_field(next_field(&mut fields)?)?;
    let source = decode_mount_field(next_field(&mut fields)?)?;
    let _options = next_field(&mut fields)?;
    if fields.next().is_some() {
        return Err(UnknownAlias::new(AliasUncertainty::MountInventory));
    }
    let target = target.ok_or_else(|| UnknownAlias::new(AliasUncertainty::MountInventory))?;
    if !target.starts_with(b"/") || target.contains(&0) {
        return Err(UnknownAlias::new(AliasUncertainty::MountInventory));
    }
    Ok(PortalMount {
        identity: line.to_vec(),
        target,
        filesystem,
        source,
    })
}

fn next_field<'data>(
    fields: &mut impl Iterator<Item = &'data [u8]>,
) -> Result<&'data [u8], UnknownAlias> {
    fields
        .next()
        .filter(|field| !field.is_empty())
        .ok_or_else(|| UnknownAlias::new(AliasUncertainty::MountInventory))
}

fn decode_mount_field(input: &[u8]) -> Result<Vec<u8>, UnknownAlias> {
    let mut output = Vec::with_capacity(input.len());
    let mut bytes = input.iter().copied();
    while let Some(byte) = bytes.next() {
        if byte != b'\\' {
            output.push(byte);
            continue;
        }
        let decoded = match (bytes.next(), bytes.next(), bytes.next()) {
            (Some(b'0'), Some(b'4'), Some(b'0')) => b' ',
            (Some(b'0'), Some(b'1'), Some(b'1')) => b'\t',
            (Some(b'0'), Some(b'1'), Some(b'2')) => b'\n',
            (Some(b'1'), Some(b'3'), Some(b'4')) => b'\\',
            _ => return Err(UnknownAlias::new(AliasUncertainty::MountInventory)),
        };
        output.push(decoded);
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
