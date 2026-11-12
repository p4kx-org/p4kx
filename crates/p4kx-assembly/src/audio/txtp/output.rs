// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use p4kx_extraction::{
    ConflictPolicy, ConversionError, check_path_length, write_replacing_counted,
};

use crate::audio::ExportState;

pub(super) struct PublishedFile {
    pub(super) size: u64,
    pub(super) state: ExportState,
    pub(super) verified_content: bool,
}

pub(super) fn write(
    path: &Path,
    text: &str,
    policy: ConflictPolicy,
) -> Result<PublishedFile, String> {
    let absolute = std::path::absolute(path).map_err(|error| error.to_string())?;
    check_path_length(&absolute).map_err(|limit| limit.to_string())?;
    if policy == ConflictPolicy::Skip
        && let Ok(existing) = std::fs::symlink_metadata(path)
    {
        return Ok(PublishedFile {
            size: existing.len(),
            state: ExportState::Kept,
            verified_content: false,
        });
    }
    let replaced = write_replacing_counted(path, policy == ConflictPolicy::IfChanged, |out| {
        out.write_all(text.as_bytes())
            .map_err(|source| ConversionError::Write {
                path: path.to_path_buf(),
                source,
            })
    })
    .map_err(|error| error.to_string())?;
    let state = if replaced.changed {
        ExportState::Written
    } else {
        ExportState::Kept
    };
    // The writer's length and CRC comparison does not verify the retained bytes.
    let verified_content = replaced.changed
        || matches_content(path, text.as_bytes()).map_err(|error| error.to_string())?;
    Ok(PublishedFile {
        size: replaced.len,
        state,
        verified_content,
    })
}

fn matches_content(path: &Path, mut expected: &[u8]) -> io::Result<bool> {
    let mut retained = BufReader::new(File::open(path)?);
    loop {
        let bytes = retained.fill_buf()?;
        if bytes.is_empty() {
            return Ok(expected.is_empty());
        }
        let Some(remaining) = expected.strip_prefix(bytes) else {
            return Ok(false);
        };
        let checked = bytes.len();
        retained.consume(checked);
        expected = remaining;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;

    use super::matches_content;

    #[test]
    fn retained_text_is_compared_through_its_last_byte() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("retained.txtp");
        let mut candidate = "# generated example\n".repeat(2_048);
        fs::write(&path, &candidate)?;
        assert!(
            matches_content(&path, candidate.as_bytes())?,
            "the complete retained text matches"
        );
        candidate.replace_range(candidate.len() - 1.., " ");
        assert!(
            !matches_content(&path, candidate.as_bytes())?,
            "a changed final byte cannot verify retained content"
        );
        Ok(())
    }
}
