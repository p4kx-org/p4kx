// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Normalizes an authored Data- or Animations-rooted archive selector.
#[must_use]
pub fn archive_animation_path(path: &str) -> Option<String> {
    if path.split(['/', '\\']).any(|part| {
        matches!(part, "" | "." | "..")
            || part.contains([':', '\0', '!', '*', '?'])
            || part.ends_with([' ', '.'])
    }) {
        return None;
    }
    let mut parts = path.split(['/', '\\']);
    let first = parts.next()?;
    if first.eq_ignore_ascii_case("data") && parts.next().is_some() {
        Some(path.replace('\\', "/"))
    } else if first.eq_ignore_ascii_case("animations") && parts.next().is_some() {
        Some(format!("Data/{}", path.replace('\\', "/")))
    } else {
        None
    }
}

/// Calculates the image checksum for an archive-relative DBA Entry.
#[must_use]
pub fn animation_bank_path_crc32(path: &str) -> Option<u32> {
    let (root, child) = path.split_once(['/', '\\'])?;
    if !root.eq_ignore_ascii_case("data") {
        return None;
    }
    let (_, extension) = child.rsplit_once('.')?;
    if !extension.eq_ignore_ascii_case("dba") {
        return None;
    }
    let mut hash = crc32fast::Hasher::new();
    for byte in child.bytes() {
        let normalized = if byte == b'\\' {
            b'/'
        } else {
            byte.to_ascii_lowercase()
        };
        hash.update(&[normalized]);
    }
    Some(hash.finalize())
}

/// A concrete `AnimationList` CAF alias with its original path spelling.
#[derive(Debug, Clone, Copy)]
pub struct NamedCafSelector<'a> {
    path: &'a str,
    rooted: bool,
}

impl<'a> NamedCafSelector<'a> {
    /// Identifies a concrete CAF alias without expanding wildcards or directives.
    #[must_use]
    pub fn new(name: &str, path: &'a str) -> Option<Self> {
        let (_, extension) = path.rsplit_once('.')?;
        if name.is_empty()
            || name.starts_with(['#', '$'])
            || name.contains(['*', '?'])
            || path.contains(['*', '?'])
            || !extension.eq_ignore_ascii_case("caf")
        {
            return None;
        }
        let rooted = path.split_once(['/', '\\']).is_some_and(|(first, _)| {
            first.eq_ignore_ascii_case("animations") || first.eq_ignore_ascii_case("data")
        });
        Some(Self { path, rooted })
    }

    /// Whether resolution uses the preceding folder of this `AnimationList`.
    #[must_use]
    pub const fn uses_scope(self) -> bool {
        !self.rooted
    }

    /// Resolves a rooted alias or a relative alias under its authored folder.
    #[must_use]
    pub fn archive_path(self, scope: Option<&str>) -> Option<String> {
        if self.rooted {
            return archive_animation_path(self.path);
        }
        let folder = scope?.trim_end_matches(['/', '\\']);
        if folder.is_empty() {
            return None;
        }
        archive_animation_path(&format!("{folder}/{}", self.path))
    }
}
