// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{WriteProtectionError, ensure_writable};

/// Returns the protection refusal for a chosen output folder.
pub(crate) fn refusal_of(folder: Option<&Path>) -> Option<WriteProtectionError> {
    ensure_writable(folder?).err()
}

/// Describes established installation protection separately from unavailable identity evidence.
pub(crate) fn message(texts: &crate::Texts, error: &WriteProtectionError) -> String {
    use crate::ProtectionText;
    match error {
        WriteProtectionError::Installation(refused) => texts.format(
            crate::ExtractionText::TargetInInstallation,
            &[(
                "installation",
                refused.installation.display().to_string().into(),
            )],
        ),
        WriteProtectionError::SourceOverwrite { .. } => texts.get(ProtectionText::SourceOverwrite),
        WriteProtectionError::TargetIdentity { .. } => texts.get(ProtectionText::TargetIdentity),
        WriteProtectionError::SourceIdentity { .. } => texts.get(ProtectionText::SourceIdentity),
        WriteProtectionError::SourceMembership { parent, .. } => texts.format(
            ProtectionText::SourceMembership,
            &[("parent", parent.display().to_string().into())],
        ),
        _ => texts.get(ProtectionText::IdentityUnavailable),
    }
}

enum ProtectionCause<'a> {
    Refused(&'a WriteProtectionError),
    PreserveFailure,
    Unclassified,
}

fn extraction_cause<'a>(error: &'a (dyn std::error::Error + 'static)) -> ProtectionCause<'a> {
    use p4kx_extraction::{ConversionError, ExtractionError, QueryExportError};

    if let Some(error) = error.downcast_ref::<ConversionError>() {
        return match error {
            ConversionError::Installation(refused) => ProtectionCause::Refused(refused),
            ConversionError::OwnedOutput(_) | ConversionError::CleanupFailed { .. } => {
                ProtectionCause::PreserveFailure
            }
            _ => ProtectionCause::Unclassified,
        };
    }
    if let Some(error) = error.downcast_ref::<ExtractionError>() {
        return match error {
            ExtractionError::Installation(refused) => ProtectionCause::Refused(refused),
            ExtractionError::OwnedOutput(_) | ExtractionError::CleanupFailed { .. } => {
                ProtectionCause::PreserveFailure
            }
            _ => ProtectionCause::Unclassified,
        };
    }
    if let Some(error) = error.downcast_ref::<QueryExportError>() {
        return match error {
            QueryExportError::Installation(refused) => ProtectionCause::Refused(refused),
            QueryExportError::OwnedOutput(_) | QueryExportError::CleanupFailed { .. } => {
                ProtectionCause::PreserveFailure
            }
            _ => ProtectionCause::Unclassified,
        };
    }
    ProtectionCause::Unclassified
}

/// Finds a protection refusal without losing a typed service error's cause.
pub(crate) fn refusal_in<'a>(
    error: &'a (dyn std::error::Error + 'static),
) -> Option<&'a WriteProtectionError> {
    let mut current = Some(error);
    while let Some(cause) = current {
        if let Some(refused) = cause.downcast_ref::<WriteProtectionError>() {
            return Some(refused);
        }
        match extraction_cause(cause) {
            ProtectionCause::Refused(refused) => return Some(refused),
            ProtectionCause::PreserveFailure => return None,
            ProtectionCause::Unclassified => {}
        }
        if matches!(
            cause.downcast_ref::<p4kx_export::CatalogError>(),
            Some(p4kx_export::CatalogError::CleanupFailed { .. })
        ) {
            return None;
        }
        match cause.downcast_ref::<p4kx_blender::PackageError>() {
            Some(p4kx_blender::PackageError::Refused(refused)) => return Some(refused),
            Some(p4kx_blender::PackageError::Write { .. }) | None => {}
            Some(_) => return None,
        }
        match cause.downcast_ref::<p4kx_extraction::OwnedOutputError>() {
            Some(p4kx_extraction::OwnedOutputError::Protection(refused)) => return Some(refused),
            Some(_) => return None,
            None => {}
        }
        current = cause
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            .map(|inner| inner as &(dyn std::error::Error + 'static))
            .or_else(|| cause.source());
    }
    None
}

/// Returns whether cancellation stopped a protection lookup.
pub(crate) fn guard_cancelled(error: &(dyn std::error::Error + 'static)) -> bool {
    refusal_in(error).is_some_and(WriteProtectionError::is_cancelled)
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use p4kx_extraction::{WriteProtectionError, WriteRefused};

    use super::{message, refusal_in};
    use crate::{Language, Texts};

    #[test]
    fn unknown_source_membership_is_not_presented_as_an_established_installation() {
        let parent = PathBuf::from("ordinary-copy-source");
        let uncertain = WriteProtectionError::SourceMembership {
            archive: parent.join("Data.p4k"),
            target: parent.join("export"),
            parent,
        };
        for (language, unknown, installation) in [
            (
                Language::BritishEnglish,
                "could not be determined",
                "lies inside the Star Citizen installation",
            ),
            (
                Language::German,
                "konnte nicht festgestellt",
                "liegt in der Star-Citizen-Installation",
            ),
        ] {
            let text = message(&Texts::new(language), &uncertain);
            assert!(
                text.contains(unknown),
                "the uncertainty is explicit: {text}"
            );
            assert!(
                text.contains("ordinary-copy-source"),
                "the parent is identified: {text}"
            );
            assert!(
                !text.contains(installation),
                "no installation is invented: {text}"
            );
        }
    }

    #[test]
    fn an_established_installation_keeps_its_localised_explanation_and_path() {
        let refused = WriteProtectionError::Installation(WriteRefused {
            installation: PathBuf::from("synthetic-game"),
            target: PathBuf::from("synthetic-game/export"),
        });
        for (language, expected) in [
            (
                Language::BritishEnglish,
                "lies inside the Star Citizen installation",
            ),
            (Language::German, "liegt in der Star-Citizen-Installation"),
        ] {
            let text = message(&Texts::new(language), &refused);
            assert!(
                text.contains(expected),
                "the established boundary remains explicit: {text}"
            );
            assert!(
                text.contains("synthetic-game"),
                "the protected path remains visible: {text}"
            );
        }
    }

    #[test]
    fn a_refusal_retains_its_type_through_an_io_wrapper() {
        let error = io::Error::new(
            io::ErrorKind::PermissionDenied,
            WriteProtectionError::SourceMembership {
                archive: PathBuf::from("source/Data.p4k"),
                target: PathBuf::from("source/output"),
                parent: PathBuf::from("source"),
            },
        );
        assert!(
            matches!(
                refusal_in(&error),
                Some(WriteProtectionError::SourceMembership { .. })
            ),
            "the typed cause survives"
        );
        let conversion =
            p4kx_extraction::ConversionError::Installation(WriteProtectionError::SourceOverwrite {
                archive: PathBuf::from("source/Data.p4k"),
                target: PathBuf::from("source/alias.p4k"),
            });
        assert!(
            matches!(
                refusal_in(&conversion),
                Some(WriteProtectionError::SourceOverwrite { .. })
            ),
            "transparent conversion errors retain the refusal"
        );
        let nested = io::Error::other(io::Error::other(conversion));
        assert!(
            matches!(
                refusal_in(&nested),
                Some(WriteProtectionError::SourceOverwrite { .. })
            ),
            "nested IO payloads retain the refusal"
        );
        assert!(
            refusal_in(&io::Error::other("unrelated IO failure")).is_none(),
            "ordinary errors remain ordinary"
        );
    }

    #[test]
    fn extraction_cleanup_failure_keeps_the_complete_error_after_a_guard_cancel() {
        use p4kx_extraction::{ConversionError, ExtractionError, QueryExportError};

        let primary = || {
            let cancel = std::sync::atomic::AtomicBool::new(true);
            let refusal = p4kx_extraction::ArchiveWriteProtection::new(
                std::path::Path::new("not-observed.p4k"),
                &cancel,
            )
            .expect_err("cancel before observing the source");
            Some(Box::new(ConversionError::Installation(refusal))
                as Box<dyn std::error::Error + Send + Sync>)
        };
        let cleanup = || {
            Box::new(p4kx_extraction::OwnedOutputCleanupError::Io {
                path: PathBuf::from("retained.part"),
                source: io::Error::other("cleanup failed"),
            })
        };
        let errors: [Box<dyn std::error::Error + Send + Sync>; 3] = [
            Box::new(ConversionError::CleanupFailed {
                primary: primary(),
                cleanup: cleanup(),
            }),
            Box::new(ExtractionError::CleanupFailed {
                primary: primary(),
                cleanup: cleanup(),
            }),
            Box::new(QueryExportError::CleanupFailed {
                primary: primary(),
                cleanup: cleanup(),
            }),
        ];
        for error in errors {
            let wrapped = io::Error::other(error);
            assert!(
                refusal_in(&wrapped).is_none(),
                "retain the complete extraction failure"
            );
            assert!(
                !super::guard_cancelled(&wrapped),
                "cleanup failure is not a plain cancel"
            );
        }
    }

    #[test]
    fn owned_file_factory_preserves_its_pure_protection_refusal() {
        let error =
            p4kx_extraction::OwnedOutputError::Protection(WriteProtectionError::SourceOverwrite {
                archive: PathBuf::from("source.p4k"),
                target: PathBuf::from("alias.p4k"),
            })
            .into_io_error();
        assert!(
            matches!(
                refusal_in(&error),
                Some(WriteProtectionError::SourceOverwrite { .. })
            ),
            "the file factory keeps the localisable typed refusal"
        );
    }

    #[test]
    fn failed_restoration_is_not_presented_as_only_its_cancelled_cause() {
        let cancel = std::sync::atomic::AtomicBool::new(true);
        let refusal = p4kx_extraction::ArchiveWriteProtection::new(
            std::path::Path::new("not-observed.p4k"),
            &cancel,
        )
        .expect_err("cancel before observing the source");
        let error = p4kx_blender::PackageError::RestoreRefused {
            path: PathBuf::from("out"),
            old: PathBuf::from("out.old"),
            cause: Box::new(p4kx_blender::PackageError::Refused(refusal)),
            replacement: Box::new(p4kx_blender::PackageError::Cancelled),
        };
        let wrapped = io::Error::other(error);
        assert!(
            refusal_in(&wrapped).is_none(),
            "retain the complete restoration failure"
        );
        assert!(
            !super::guard_cancelled(&wrapped),
            "cleanup or restoration failure is not a plain cancel"
        );
    }
}
