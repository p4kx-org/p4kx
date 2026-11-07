// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree};

use crate::CatalogFailure;
use crate::crafting_quantization::{self, Malformed};
use crate::crafting_records;
use crate::crafting_tier::TooDeep;
use crate::crafting_types::{
    BlueprintPool, BlueprintTiers, DismantleProcess, GameplayProperty, GlobalParams,
    QualityDistribution, QualityQuantization,
};
use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_ledger::{ExportLedger, SourceDiagnosis};

use super::selection::Action;

pub(super) enum Piece<'f> {
    Blueprint(BlueprintTiers<'f>),
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
    Failed(CatalogFailure),
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ProjectionError {
    #[error(transparent)]
    Blueprint(#[from] TooDeep),
    #[error(transparent)]
    Quantization(#[from] Malformed),
    #[error(transparent)]
    Quality(#[from] crate::source_fields::FieldError),
}

impl ProjectionError {
    pub(super) fn diagnosis(&self) -> SourceDiagnosis {
        let (code, field, source_type) = match self {
            Self::Quality(error) => return crate::export_source_failure::diagnosis(error),
            Self::Blueprint(_) => (
                ExportDiagnosticCode::ProjectionFailure,
                "blueprint.tiers".to_owned(),
                "CraftingCost_Select",
            ),
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
        }
    }
}

pub(super) fn project<'f>(
    action: Action,
    tree: &RecordTree<'f>,
    role: ExportSourceRole,
    ledger: &mut ExportLedger,
) -> Result<Option<Piece<'f>>, ProjectionError> {
    let piece = match action {
        Action::Blueprint => Some(Piece::Blueprint(crafting_records::blueprint(tree)?)),
        Action::Property => Some(Piece::Property(crafting_records::property(tree))),
        Action::Distribution => {
            let read = crafting_records::distribution(tree)?;
            crate::source_projection_notes::record(
                crate::Kind::QualityDistributions,
                role,
                tree,
                read.notes,
                ledger,
            );
            Some(Piece::Distribution(read.value))
        }
        Action::Quantization => Some(Piece::Quantization(crafting_quantization::quantization(
            tree,
        )?)),
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
    Ok(piece)
}
