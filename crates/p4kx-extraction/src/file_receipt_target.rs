// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{ArchiveWriteProtection, CancelToken, OriginArtifactName, is_redirected_metadata};

use super::lock::{LOCK_NAME, invalid};
use super::{COMPANION_SUFFIX, check};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BindTarget {
    Receipt,
    Replacement,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetLeaf {
    File,
    SymbolicLink,
}

pub(super) struct BoundTarget {
    pub(super) path: PathBuf,
    pub(super) leaf: TargetLeaf,
}

pub(super) fn target(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &CancelToken,
) -> io::Result<PathBuf> {
    bind_target(path, protection, cancel, BindTarget::Receipt).map(|bound| bound.path)
}

pub(super) fn bind_target(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &CancelToken,
    mode: BindTarget,
) -> io::Result<BoundTarget> {
    check(path, protection, cancel)?;
    let name = path
        .file_name()
        .ok_or_else(|| invalid("the output names no file"))?;
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let lexical = fs::canonicalize(parent)?.join(name);
    let bound = match fs::symlink_metadata(&lexical) {
        Ok(metadata) if metadata.file_type().is_symlink() && mode == BindTarget::Replacement => {
            BoundTarget {
                path: lexical,
                leaf: TargetLeaf::SymbolicLink,
            }
        }
        Ok(metadata) if metadata.is_file() && !is_redirected_metadata(&metadata) => BoundTarget {
            path: fs::canonicalize(lexical)?,
            leaf: TargetLeaf::File,
        },
        Ok(_) => {
            return Err(invalid(
                "the origin target must be a regular, non-redirected file",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => BoundTarget {
            path: lexical,
            leaf: TargetLeaf::File,
        },
        Err(error) => return Err(error),
    };
    let name = bound
        .path
        .file_name()
        .ok_or_else(|| invalid("the output names no file"))?;
    if name.eq_ignore_ascii_case(LOCK_NAME) {
        return Err(invalid("the output names the origin publication lock"));
    }
    if name
        .to_string_lossy()
        .to_ascii_lowercase()
        .ends_with(COMPANION_SUFFIX)
    {
        return Err(invalid("the output names a reserved origin companion"));
    }
    #[cfg(windows)]
    if name.to_string_lossy().contains(':') {
        return Err(invalid("the origin target names an alternate data stream"));
    }
    check(&bound.path, protection, cancel)?;
    Ok(bound)
}

pub(super) fn sibling_receipt(path: &Path) -> io::Result<PathBuf> {
    let mut name = path
        .file_name()
        .ok_or_else(|| invalid("the output names no file"))?
        .to_os_string();
    name.push(COMPANION_SUFFIX);
    Ok(path.with_file_name(name))
}

pub(super) fn lexical_receipt(path: &Path) -> io::Result<Option<OriginArtifactName>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || is_redirected_metadata(&metadata) {
        return Ok(None);
    }
    let actual = fs::canonicalize(path)?;
    if actual.parent() != path.parent() {
        return Ok(None);
    }
    let Some(stem) = actual
        .file_stem()
        .and_then(|name| Path::new(name).file_stem())
    else {
        return Ok(None);
    };
    if sibling_receipt(&actual.with_file_name(stem))? != actual {
        return Ok(None);
    }
    OriginArtifactName::from_basename(stem).map(Some)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::io;

    use super::lexical_receipt;
    use crate::OriginArtifactName;

    #[test]
    fn lexical_companion_owner_comes_from_the_exact_regular_basename() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let root = fs::canonicalize(dir.path())?;
        let path = root.join("Panel.preview.png.p4kx-origin.jsonl");
        fs::write(&path, b"fixture")?;
        assert_eq!(
            lexical_receipt(&path)?,
            Some(OriginArtifactName::from_basename(OsStr::new(
                "Panel.preview.png"
            ))?)
        );
        assert_eq!(
            lexical_receipt(&root.join("absent.p4kx-origin.jsonl"))?,
            None
        );
        let foreign = root.join("Panel.preview.png.user.jsonl");
        fs::write(&foreign, b"fixture")?;
        assert_eq!(lexical_receipt(&foreign)?, None);
        Ok(())
    }
}
