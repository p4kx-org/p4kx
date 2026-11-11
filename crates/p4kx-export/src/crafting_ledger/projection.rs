// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Guid, RecordTree};

use crate::CatalogError;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::crafting_quantization::{self, Malformed};
use crate::crafting_records;
use crate::crafting_types::{
    BlueprintPool, BlueprintTiers, DismantleProcess, GameplayProperty, GlobalParams,
    QualityDistribution, QualityQuantization,
};
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::SourceDiagnosis;

use super::selection::Action;

pub(super) enum Piece<'f> {
    Blueprint(BlueprintTiers<'f>),
    CanonicalBlueprint(Box<crate::blueprint_catalog_read::PreparedBlueprint<'f>>),
    Property(GameplayProperty<'f>),
    Distribution(QualityDistribution<'f>),
    Quantization(QualityQuantization<'f>),
    ResourceLinks {
        resource_type: Guid,
        quantizations: Vec<Guid>,
    },
    Pool(BlueprintPool<'f>),
    Dismantle(DismantleProcess<'f>),
    Globals(GlobalParams),
    Skipped,
    Failed(CatalogReadFailure<'f>),
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ProjectionError {
    #[error(transparent)]
    Quantization(#[from] Malformed),
    #[error(transparent)]
    Quality(#[from] crate::source_fields::FieldError),
}

impl ProjectionError {
    pub(super) fn diagnosis(&self) -> SourceDiagnosis {
        let (code, field, source_type) = match self {
            Self::Quality(error) => return crate::export_source_failure::diagnosis(error),
            Self::Quantization(error) => {
                let (code, field) = match error {
                    Malformed::NoQuantization => (
                        ExportDiagnosticCode::MalformedSource,
                        "qualityQuantization".to_owned(),
                    ),
                    Malformed::NoBands => (
                        ExportDiagnosticCode::EmptyValue,
                        "qualityQuantization.bands".to_owned(),
                    ),
                    Malformed::Incomplete(index) => (
                        ExportDiagnosticCode::MalformedSource,
                        format!("qualityQuantization.bands[{index}]"),
                    ),
                    Malformed::Reversed(index) => (
                        ExportDiagnosticCode::MalformedSource,
                        format!("qualityQuantization.bands[{index}].end"),
                    ),
                };
                (code, field, "CraftingQualityQuantizationRecord")
            }
        };
        SourceDiagnosis {
            code: Some(code),
            field: Some(field),
            source_type: Some(source_type.to_owned()),
            target_guid: None,
            body: crate::record_source_status::DiagnosisBody::Pending,
        }
    }
}

pub(super) enum ProjectError {
    Source(ProjectionError),
    Catalog(CatalogError),
}

impl From<ProjectionError> for ProjectError {
    fn from(value: ProjectionError) -> Self {
        Self::Source(value)
    }
}

pub(super) struct Projected<'source> {
    pub(super) piece: Option<Piece<'source>>,
    pub(super) notes: Vec<crate::source_fields::FieldError>,
}

pub(super) fn project<'f>(
    action: Action,
    tree: &RecordTree<'f>,
    cancel: Option<&CancelToken>,
) -> Result<Projected<'f>, ProjectError> {
    let mut notes = Vec::new();
    let piece = match action {
        Action::Blueprint => Some(Piece::Blueprint(
            crafting_records::blueprint(tree, cancel).map_err(ProjectError::Catalog)?,
        )),
        Action::Property => Some(Piece::Property(crafting_records::property(tree))),
        Action::Distribution => {
            let read = crafting_records::distribution(tree).map_err(ProjectionError::from)?;
            notes = read.notes;
            Some(Piece::Distribution(read.value))
        }
        Action::Quantization => Some(Piece::Quantization(
            crafting_quantization::quantization(tree).map_err(ProjectionError::from)?,
        )),
        Action::Pool => Some(Piece::Pool(crafting_records::pool(tree))),
        Action::Dismantle => crafting_records::dismantle(tree).map(Piece::Dismantle),
        Action::Globals => Some(Piece::Globals(crafting_records::globals(tree))),
        Action::ResourceLinks => {
            let quantizations = crafting_quantization::linked_quantizations(tree);
            (!quantizations.is_empty()).then(|| Piece::ResourceLinks {
                resource_type: tree.guid(),
                quantizations,
            })
        }
    };
    Ok(Projected { piece, notes })
}
