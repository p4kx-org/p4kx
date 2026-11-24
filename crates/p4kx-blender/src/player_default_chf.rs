// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::character::{Chf, ChfError};
use p4kx_assembly::{BodyVariant, SocArchiveMember, SocCaptureError, SocMemberError};
use p4kx_extraction::{ArchiveWatch, CancelToken, Entry, EntryOrigin, PathIndex};
use thiserror::Error;

use crate::player_source::{PlayerSourceError, check, index_error};

/// The exact authored default CHF selection for a player body variant.
#[derive(Debug)]
#[non_exhaustive]
pub enum PlayerDefaultChf<'archive> {
    /// No Entry matches the authored selector.
    Missing {
        /// The canonical Archive path.
        selector: &'static str,
    },
    /// Multiple physical Entries match the authored selector.
    Ambiguous {
        /// The canonical Archive path.
        selector: &'static str,
        /// Every matching physical Entry in Archive order.
        entries: Vec<&'archive Entry>,
    },
    /// Exactly one physical Entry matches the authored selector.
    Selected {
        /// The observed Entry and its containing source.
        origin: EntryOrigin,
        /// The interpreted original CHF or its read or decode failure.
        payload: Result<Box<Chf>, PlayerDefaultChfPayloadError>,
    },
}

/// Why the selected default CHF payload could not be interpreted.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PlayerDefaultChfPayloadError {
    /// The verified Entry payload could not be read.
    #[error(transparent)]
    Unreadable(#[from] SocMemberError),
    /// The payload does not contain a valid CHF.
    #[error(transparent)]
    Invalid(#[from] ChfError),
}

/// Reads the unique default CHF Entry selected by the original body variant.
///
/// # Errors
/// Returns cancellation, Archive change, physical selection or capture errors.
pub fn read_player_default_chf<'archive>(
    paths: &PathIndex<'archive>,
    variant: BodyVariant,
    cancel: &CancelToken,
) -> Result<PlayerDefaultChf<'archive>, PlayerSourceError> {
    let selector = match variant {
        BodyVariant::Male => "Data/Libs/CharacterCustomizer/masculinedefault.chf",
        BodyVariant::Female => "Data/Libs/CharacterCustomizer/femininedefault.chf",
        _ => return Err(PlayerSourceError::BodyVariant),
    };
    let archive = paths.archive();
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let entries = paths
        .matching_entries_with(selector, |_| check(cancel))
        .map_err(index_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(index_error)?;
    let result = match entries.as_slice() {
        [] => PlayerDefaultChf::Missing { selector },
        [entry] => {
            let (origin, payload) = SocArchiveMember::read(archive, entry, || {
                if cancel.is_cancelled() {
                    return Err(SocCaptureError::Cancelled);
                }
                watch.poll()?;
                Ok(())
            })?
            .into_parts();
            let payload = payload
                .map_err(PlayerDefaultChfPayloadError::from)
                .and_then(|bytes| Chf::read(&bytes).map(Box::new).map_err(Into::into));
            PlayerDefaultChf::Selected { origin, payload }
        }
        _ => PlayerDefaultChf::Ambiguous { selector, entries },
    };
    check(cancel)?;
    watch.check()?;
    Ok(result)
}
