// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use anyhow::Result;
use p4kx_assembly::character::Chf;
use p4kx_assembly::{BodyVariant, SocCaptureError, SocMemberError};
use p4kx_blender::{PlayerDefaultChf, PlayerDefaultChfPayloadError, PlayerSourceError};
use p4kx_extraction::{Archive, CancelToken, EntryOrigin, PathIndex};
use thiserror::Error;

use crate::chf::InvalidChf;
use crate::exit::{ARCHIVE_CHANGED, ARCHIVE_ERROR, INVALID_INPUT};

pub(super) struct Selected {
    pub(super) origin: EntryOrigin,
    pub(super) chf: Box<Chf>,
    variant: BodyVariant,
}

#[derive(Debug, Error)]
enum DefaultChfError {
    #[error("the default CHF Entry {selector} is missing")]
    Missing { selector: &'static str },
    #[error("the default CHF Entry {selector} matches {count} physical Entries")]
    Ambiguous {
        selector: &'static str,
        count: usize,
    },
    #[error("cannot read the default CHF Entry {path}")]
    Unreadable {
        path: String,
        #[source]
        cause: SocMemberError,
    },
    #[error("the default CHF Entry {path} uses the {actual} rig for the selected {expected} body")]
    BodyMismatch {
        path: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("the default CHF reader returned an unsupported result")]
    Unsupported,
    #[error(transparent)]
    Capture(#[from] PlayerSourceError),
}

pub(super) fn read(
    archive: &Archive,
    variant: BodyVariant,
    cancel: &CancelToken,
) -> Result<Selected> {
    let paths = PathIndex::new(archive);
    let selected = p4kx_blender::read_player_default_chf(&paths, variant, cancel)
        .map_err(DefaultChfError::from)?;
    let (origin, payload) = match selected {
        PlayerDefaultChf::Selected { origin, payload } => (origin, payload),
        PlayerDefaultChf::Missing { selector } => {
            return Err(DefaultChfError::Missing { selector }.into());
        }
        PlayerDefaultChf::Ambiguous { selector, entries } => {
            return Err(DefaultChfError::Ambiguous {
                selector,
                count: entries.len(),
            }
            .into());
        }
        _ => return Err(DefaultChfError::Unsupported.into()),
    };
    let chf = match payload {
        Ok(chf) => chf,
        Err(PlayerDefaultChfPayloadError::Invalid(source)) => {
            return Err(InvalidChf {
                path: PathBuf::from(origin.entry().path()),
                source,
            }
            .into());
        }
        Err(PlayerDefaultChfPayloadError::Unreadable(cause)) => {
            return Err(DefaultChfError::Unreadable {
                path: origin.entry().path().to_owned(),
                cause,
            }
            .into());
        }
        Err(_) => return Err(DefaultChfError::Unsupported.into()),
    };
    Ok(Selected {
        origin,
        chf,
        variant,
    })
}

impl Selected {
    pub(super) fn check_rig(&self, actual: BodyVariant) -> Result<()> {
        if self.variant == actual {
            return Ok(());
        }
        Err(DefaultChfError::BodyMismatch {
            path: self.origin.entry().path().to_owned(),
            expected: self.variant.code(),
            actual: actual.code(),
        }
        .into())
    }

    pub(super) fn name(&self) -> Option<&str> {
        self.origin.entry().path().rsplit(['/', '\\']).next()
    }
}

pub(crate) fn exit_code(error: &anyhow::Error) -> Option<u8> {
    error
        .downcast_ref::<DefaultChfError>()
        .map(|error| match error {
            DefaultChfError::Missing { .. }
            | DefaultChfError::Ambiguous { .. }
            | DefaultChfError::BodyMismatch { .. } => INVALID_INPUT,
            DefaultChfError::Capture(
                PlayerSourceError::ArchiveChanged(_)
                | PlayerSourceError::Capture(SocCaptureError::Changed(_)),
            ) => ARCHIVE_CHANGED,
            _ => ARCHIVE_ERROR,
        })
}
