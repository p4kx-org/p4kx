// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_archive::ArchiveError;

use crate::{
    ConversionError, DataForgeLoadError, EntryReadError, ExtractionError, LocalizationLoadError,
};

/// Finds the original [`ArchiveError`] through source chains and typed Extraction wrappers.
/// Direct and boxed wrappers retain the original Archive diagnosis.
#[must_use]
pub fn archive_error_in<'a>(error: &'a (dyn Error + 'static)) -> Option<&'a ArchiveError> {
    let mut current: Option<&'a (dyn Error + 'static)> = Some(error);
    while let Some(cause) = current {
        if let Some(found) = archive_error_of(cause) {
            return Some(found);
        }
        current = cause.source();
    }
    None
}

fn archive_error_of<'a>(error: &'a (dyn Error + 'static)) -> Option<&'a ArchiveError> {
    if let Some(found) = downcast::<ArchiveError>(error) {
        return Some(found);
    }
    match downcast::<EntryReadError>(error) {
        Some(EntryReadError::Archive(found)) => return Some(found),
        Some(_) => return None,
        None => {}
    }
    match downcast::<ConversionError>(error) {
        Some(ConversionError::Archive(found)) => return Some(found),
        Some(_) => return None,
        None => {}
    }
    match downcast::<DataForgeLoadError>(error) {
        Some(DataForgeLoadError::Archive(found)) => return Some(found),
        Some(_) => return None,
        None => {}
    }
    match downcast::<LocalizationLoadError>(error) {
        Some(LocalizationLoadError::Archive(found)) => return Some(found),
        Some(_) => return None,
        None => {}
    }
    match downcast::<ExtractionError>(error) {
        Some(ExtractionError::Archive(found)) => Some(found),
        _ => None,
    }
}

fn downcast<'a, T: Error + 'static>(error: &'a (dyn Error + 'static)) -> Option<&'a T> {
    error
        .downcast_ref::<T>()
        .or_else(|| error.downcast_ref::<Box<T>>().map(Box::as_ref))
}
#[cfg(test)]
mod tests {
    use p4kx_archive::ArchiveError;

    use super::archive_error_in;
    use crate::{ConversionError, EntryReadError};

    fn key_required() -> ArchiveError {
        ArchiveError::KeyRequired {
            path: "Data.p4k".into(),
            entry: "a".to_owned(),
        }
    }

    #[test]
    fn archive_errors_are_found_directly_and_behind_passing_wrappers() {
        assert!(archive_error_in(&key_required()).is_some(), "direct");
        let read = EntryReadError::from(key_required());
        assert!(
            archive_error_in(&read).is_some_and(ArchiveError::is_key_required),
            "entry read"
        );
        let conversion = ConversionError::from(key_required());
        assert!(archive_error_in(&conversion).is_some(), "conversion");
    }

    #[test]
    fn boxed_typed_wrappers_keep_the_original_archive_key_diagnosis() {
        let cases: [Box<dyn std::error::Error>; 6] = [
            Box::new(Box::new(key_required())),
            Box::new(Box::new(crate::ExtractionError::from(key_required()))),
            Box::new(Box::new(EntryReadError::from(key_required()))),
            Box::new(Box::new(ConversionError::from(key_required()))),
            Box::new(Box::new(crate::DataForgeLoadError::Archive(key_required()))),
            Box::new(Box::new(crate::LocalizationLoadError::Archive(
                key_required(),
            ))),
        ];
        for error in cases {
            assert!(
                archive_error_in(error.as_ref()).is_some_and(ArchiveError::is_key_required),
                "boxing must preserve the original typed Archive diagnosis"
            );
        }
    }
    #[test]
    fn other_errors_have_no_archive_error() {
        assert!(
            archive_error_in(&EntryReadError::Binary).is_none(),
            "binary"
        );
    }
}
