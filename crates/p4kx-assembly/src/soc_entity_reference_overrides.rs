// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;

use super::{SocArchiveMember, SocCaptureError};

const FILE_NAME: &str = "entitycomponententityreferenceoverride.xml";

pub(super) fn read(
    archive: &Archive,
    mut check: impl FnMut() -> Result<(), SocCaptureError>,
) -> Result<Vec<SocArchiveMember>, SocCaptureError> {
    let mut members = Vec::new();
    for entry in archive.entries() {
        check()?;
        if matches_path(entry.path()) {
            members.try_reserve(1)?;
            members.push(SocArchiveMember::read(archive, entry, &mut check)?);
        }
    }
    Ok(members)
}

fn matches_path(path: &str) -> bool {
    let mut components = path.rsplit(['/', '\\']);
    matches!(
        (components.next(), components.next(), components.next()),
        (Some(file), Some(slot), Some(directory))
            if file.eq_ignore_ascii_case(FILE_NAME)
                && !slot.is_empty()
                && slot.bytes().all(|byte| byte.is_ascii_digit())
                && directory.eq_ignore_ascii_case("metadata")
    )
}
