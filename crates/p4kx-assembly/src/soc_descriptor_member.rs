// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry, EntryOrigin, PathIndex};
use p4kx_model::SocContainerDescriptor;

use super::soc_archive_member::{SocArchiveMember, origin, select};

/// The result of selecting a physical member in its owning archive.
#[derive(Debug)]
#[non_exhaustive]
pub enum SocMemberSelection {
    /// The declared name has no matching member.
    Missing,
    /// Several physical Entries match the same path.
    Ambiguous(Vec<EntryOrigin>),
    /// Exactly one physical Entry was selected.
    Selected(SocArchiveMember),
}

impl SocMemberSelection {
    pub(super) fn read(
        index: &PathIndex<'_>,
        paths: impl IntoIterator<Item = impl AsRef<str>>,
        mut check: impl FnMut() -> Result<(), super::SocCaptureError>,
    ) -> Result<Self, super::SocCaptureError> {
        let matches = select(index, paths, &mut check)?;
        Self::from_matches(index.archive(), &matches, check)
    }

    fn from_matches(
        archive: &Archive,
        matches: &[&Entry],
        check: impl FnMut() -> Result<(), super::SocCaptureError>,
    ) -> Result<Self, super::SocCaptureError> {
        match matches {
            [] => Ok(Self::Missing),
            [entry] => Ok(Self::Selected(SocArchiveMember::read(
                archive, entry, check,
            )?)),
            entries => entries
                .iter()
                .map(|entry| origin(archive, entry))
                .collect::<Result<_, _>>()
                .map(Self::Ambiguous),
        }
    }

    /// Returns the selected physical member when there is one.
    #[must_use]
    pub const fn selected(&self) -> Option<&SocArchiveMember> {
        match self {
            Self::Selected(member) => Some(member),
            _ => None,
        }
    }
}

pub(super) fn descriptor(
    index: &PathIndex<'_>,
    stem: &str,
    check: impl FnMut() -> Result<(), super::SocCaptureError>,
) -> Result<SocMemberSelection, super::SocCaptureError> {
    let paths = [format!("{stem}.xml"), format!("{stem}\\{stem}.xml")];
    SocMemberSelection::read(index, paths, check)
}

pub(super) fn registry(
    archive: &Archive,
    check: impl FnMut() -> Result<(), super::SocCaptureError>,
) -> Result<SocMemberSelection, super::SocCaptureError> {
    let mut check = check;
    let mut found = Vec::new();
    for entry in archive.entries() {
        check()?;
        let mut parts = entry.path().rsplit(['/', '\\']);
        if parts
            .next()
            .is_some_and(|part| part.eq_ignore_ascii_case("entitycomponentroom.xml"))
            && parts
                .next()
                .is_some_and(|part| part.eq_ignore_ascii_case("metadata"))
        {
            found.push(entry);
        }
    }
    SocMemberSelection::from_matches(archive, &found, check)
}

pub(super) fn parsed(selection: &SocMemberSelection) -> Option<SocContainerDescriptor<'_>> {
    SocContainerDescriptor::parse(selection.selected()?.payload().ok()?).ok()
}
