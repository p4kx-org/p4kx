// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_export::{ExportDiagnosticCode, ExportDiagnosticStage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceLevel {
    Handler,
    Contract,
    SubContract,
}

impl SourceLevel {
    #[must_use]
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Handler => "handler",
            Self::Contract => "contract",
            Self::SubContract => "subContract",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArrayShape {
    Empty,
    Populated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProductionFailure {
    NoneReported,
    Reported {
        stage: ExportDiagnosticStage,
        code: ExportDiagnosticCode,
        family: MetadataFamily,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MetadataFamily {
    NotApplicable,
    ExampleUnavailable,
    MetadataAbsent,
    ConflictingExamples,
    PropertyValue,
    LocationCondition,
    DataSetCondition,
    Other,
}

/// A verification failure containing only static stage and category discriminants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Failure {
    Setup(SetupFault),
    ArchiveDcb(ArchiveFault),
    CatalogBuild(CatalogFault),
    OutputParse(OutputFault),
    IndependentRead(ReadFault),
    SourceCompare(SourceFault),
    Joins(JoinFault),
    HealthReconcile(HealthFault),
    PositiveScopedCoverage,
    CountReconcile(CountFault),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SetupFault {
    ArchivePath,
    ArchiveKey,
    CoverageConfiguration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArchiveFault {
    Open,
    DcbRead,
    DcbOpen,
    SourceChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CatalogFault {
    Build,
    Accounting(AccountingFault),
    UnexpectedPublicationRefusal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccountingFault {
    MissingCatalogRows,
    ConflictingCatalogRows,
    MissingCatalogRowSource,
    MissingPhysicalReferenceTarget,
    PoisonedEvidence,
    MissingInventory,
    UnobservedFailure,
    MissingFieldInspection,
    ConflictingFieldInspection,
    FieldPath,
    ConflictingDisposition,
    Unreconciled,
    CounterOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutputFault {
    JsonDecode,
    MissingSets,
    MissingContracts,
    SetIdentity,
    DuplicateSetIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadFault {
    Metadata,
    UnreportedCanonicalFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceFault {
    SourceArrayShape,
    SourceStructShape,
    MissingAuthoredArray {
        level: SourceLevel,
        shape: ArrayShape,
        production: ProductionFailure,
    },
    SetPath,
    SetType,
    SetLevel,
    PropertyArrayShape,
    DeclarationCount,
    NullDeclaration,
    DeclarationTextShape,
    DeclarationTextValue,
    ValuePresence,
    ValueType,
    ScopedType,
    FilterFieldMissing,
    FilterPresence,
    FilterType,
    FilterPayloadFieldMissing,
    UnsupportedPayload,
    PayloadNull,
    PayloadEnum,
    PayloadArray,
    PayloadCount,
    TargetIdentity,
    TargetPath,
    TargetType,
    TargetIndex,
    TargetLocation,
    OutsideOwnerStatus,
    OutsideOwnerVariable,
    SourcePointer,
    PointerStructIndex,
    PointerInstanceIndex,
    UnreadablePayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JoinFault {
    ContractIdentity,
    MissingRawContract,
    ArrayShape,
    Count,
    Target,
    Owner,
    Location,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HealthFault {
    MissingKind,
    SelectedCount,
    ReadOmissionCount,
    ProjectionOmission,
    ProjectedCount,
    IntentionalSkip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CountFault {
    Sets,
    Contracts,
    EmptyCoverage,
}

impl Failure {
    pub(super) fn classify<T, E>(result: Result<T, E>, failure: Self) -> Result<T, Self> {
        result.map_err(|_| failure)
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "property_set_oracle {self:?}")
    }
}

impl std::error::Error for Failure {}

pub(super) const fn require(condition: bool, failure: Failure) -> Result<(), Failure> {
    if condition { Ok(()) } else { Err(failure) }
}
