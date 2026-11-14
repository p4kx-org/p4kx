// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use crate::discovery_context::DiscoveryContext;
use crate::discovery_report::DiscoveryIssueKind;
use crate::prefix_fs::{DiscoveryPathKind, PrefixFileSystem};

/// Resolves an exact component before accepting one unique ASCII-case alternative.
pub(crate) fn component<F: PrefixFileSystem>(
    context: &mut DiscoveryContext<'_, F>,
    parent: &Path,
    wanted: &OsStr,
) -> Option<PathBuf> {
    let exact = parent.join(wanted);
    let before = context.issues.len();
    match context.kind(&exact) {
        Some(DiscoveryPathKind::Directory) => return Some(exact),
        Some(_) => return None,
        None => {}
    }
    if context.issues.len() != before || context.budget.exhausted() {
        return None;
    }
    let names = context.entries(parent)?;
    let mut candidates = names.iter().filter(|name| {
        name.as_encoded_bytes()
            .eq_ignore_ascii_case(wanted.as_encoded_bytes())
    });
    let matched = candidates.next()?;
    if candidates.next().is_some() {
        context.issue(&exact, DiscoveryIssueKind::Ambiguous);
        return None;
    }
    let actual = parent.join(matched);
    (context.kind(&actual) == Some(DiscoveryPathKind::Directory)).then_some(actual)
}

pub(crate) fn descend<F: PrefixFileSystem>(
    context: &mut DiscoveryContext<'_, F>,
    parent: &Path,
    parts: &[&str],
) -> Option<PathBuf> {
    let mut current = parent.to_path_buf();
    for part in parts {
        current = component(context, &current, OsStr::new(part))?;
    }
    Some(current)
}

pub(crate) fn absolute<F: PrefixFileSystem>(
    context: &mut DiscoveryContext<'_, F>,
    path: &Path,
) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let before = context.issues.len();
    match context.kind(path) {
        Some(DiscoveryPathKind::Directory) => return Some(path.to_path_buf()),
        Some(_) => return None,
        None => {}
    }
    if context.issues.len() != before || context.budget.exhausted() {
        return None;
    }
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => resolved.push(component.as_os_str()),
            Component::Normal(name) => resolved = self::component(context, &resolved, name)?,
            Component::CurDir => {}
            Component::ParentDir => return None,
        }
    }
    Some(resolved)
}
