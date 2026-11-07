// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the shared parent-and-name output selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OutputTargetText {
    /// The modal title.
    Title,
    /// The parent directory label.
    Parent,
    /// Opens the native parent directory chooser.
    ChooseParent,
    /// No parent directory was selected.
    NoParent,
    /// The output file name label.
    FileName,
    /// The package directory name label.
    PackageName,
    /// The final resolved destination label.
    Destination,
    /// The destination is not ready.
    NoDestination,
    /// Explains the required parent directory grant.
    ParentHint,
    /// Explains where the file is saved.
    FileHint,
    /// Explains package staging in the selected parent.
    PackageHint,
    /// Diagnostic permission applies only to this selection.
    DiagnosticHint,
    /// Commits the selection.
    Confirm,
    /// Commits the selection and starts its direct export job.
    StartExport,
    /// Closes the selection without committing it.
    Cancel,
    /// A name is required.
    Empty,
    /// A single relative file-name component is required.
    Component,
    /// Control characters cannot be used in the name.
    Control,
    /// Native picker creation failed; takes `reason`.
    PickerFailed,
    /// The native picker ended without a reply.
    PickerDisconnected,
    /// The picker returned a relative path.
    PickerRelative,
}

impl OutputTargetText {
    pub(crate) const ALL: [Self; 21] = [
        Self::Title,
        Self::Parent,
        Self::ChooseParent,
        Self::NoParent,
        Self::FileName,
        Self::PackageName,
        Self::Destination,
        Self::NoDestination,
        Self::ParentHint,
        Self::FileHint,
        Self::PackageHint,
        Self::DiagnosticHint,
        Self::Confirm,
        Self::StartExport,
        Self::Cancel,
        Self::Empty,
        Self::Component,
        Self::Control,
        Self::PickerFailed,
        Self::PickerDisconnected,
        Self::PickerRelative,
    ];

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Title => "output-target-title",
            Self::Parent => "output-target-parent",
            Self::ChooseParent => "output-target-choose-parent",
            Self::NoParent => "output-target-no-parent",
            Self::FileName => "output-target-file-name",
            Self::PackageName => "output-target-package-name",
            Self::Destination => "output-target-destination",
            Self::NoDestination => "output-target-no-destination",
            Self::ParentHint => "output-target-parent-hint",
            Self::FileHint => "output-target-file-hint",
            Self::PackageHint => "output-target-package-hint",
            Self::DiagnosticHint => "output-target-diagnostic-hint",
            Self::Confirm => "output-target-confirm",
            Self::StartExport => "output-target-start-export",
            Self::Cancel => "output-target-cancel",
            Self::Empty => "output-target-empty",
            Self::Component => "output-target-component",
            Self::Control => "output-target-control",
            Self::PickerFailed => "output-target-picker-failed",
            Self::PickerDisconnected => "output-target-picker-disconnected",
            Self::PickerRelative => "output-target-picker-relative",
        }
    }
}

impl From<OutputTargetText> for crate::Text {
    fn from(text: OutputTargetText) -> Self {
        Self::OutputTarget(text)
    }
}
