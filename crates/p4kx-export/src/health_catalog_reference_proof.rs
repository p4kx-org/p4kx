// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::Guid;

use super::RecordSubject;
use super::health_reference_data::ReferenceData;
use super::health_reference_observation::{ReferenceObservation, check_frames};
use super::health_reference_receipt::ReferenceFrame;
use super::health_reference_recipe::ReferenceRecipe;
use crate::{ExportAccountingError, ExportReferenceStatus, HealthText};

/// The observed header and caller values of one resolved root-Record candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogReferenceData {
    pub(crate) field: HealthText,
    pub(crate) source_type: HealthText,
    pub(crate) target_guid: Guid,
    pub(crate) target_record_index: u32,
    pub(crate) target_record_path: HealthText,
    pub(crate) target_type: HealthText,
}

impl From<CatalogReferenceData> for ReferenceData {
    fn from(data: CatalogReferenceData) -> Self {
        Self {
            field: data.field,
            source_type: data.source_type,
            target_guid: Some(data.target_guid),
            target_record_index: Some(data.target_record_index),
            target_record_path: Some(data.target_record_path),
            target_type: Some(data.target_type),
            target_location: None,
            source_pointer: None,
        }
    }
}

#[derive(Debug, Clone)]
enum Evidence {
    Original {
        frame: Arc<ReferenceFrame>,
        recipe: ReferenceRecipe,
    },
    Accounting(RecordSubject),
}

/// A resolved candidate paired with its original caller recipe or unbound accounting.
#[derive(Debug, Clone)]
pub(crate) struct CatalogReferenceProof {
    data: CatalogReferenceData,
    evidence: Evidence,
}

impl CatalogReferenceProof {
    pub(super) const fn admitted(
        data: CatalogReferenceData,
        frame: Arc<ReferenceFrame>,
        recipe: ReferenceRecipe,
    ) -> Self {
        Self {
            data,
            evidence: Evidence::Original { frame, recipe },
        }
    }

    pub(crate) const fn accounting(subject: RecordSubject, data: CatalogReferenceData) -> Self {
        Self {
            data,
            evidence: Evidence::Accounting(subject),
        }
    }

    pub(crate) const fn data(&self) -> &CatalogReferenceData {
        &self.data
    }

    pub(crate) fn materialize(
        &self,
        status: Option<ExportReferenceStatus>,
    ) -> Result<ReferenceObservation, ExportAccountingError> {
        self.clone().into_observation(status)
    }

    pub(super) fn into_observation(
        self,
        status: Option<ExportReferenceStatus>,
    ) -> Result<ReferenceObservation, ExportAccountingError> {
        let data = self.data.into();
        Ok(match self.evidence {
            Evidence::Original { frame, recipe } => {
                let body = recipe.body(&data)?;
                ReferenceObservation::captured(data, status, frame, body)
            }
            Evidence::Accounting(subject) => {
                ReferenceObservation::accounting(subject, data, status)
            }
        })
    }

    fn subject(&self) -> &RecordSubject {
        match &self.evidence {
            Evidence::Original { frame, .. } => frame.subject(),
            Evidence::Accounting(subject) => subject,
        }
    }

    pub(crate) fn check_duplicate(&self, other: &Self) -> Result<(), ExportAccountingError> {
        if self != other {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        match (&self.evidence, &other.evidence) {
            (
                Evidence::Original { frame, recipe },
                Evidence::Original {
                    frame: other_frame,
                    recipe: other_recipe,
                },
            ) => {
                if recipe != other_recipe {
                    return Err(ExportAccountingError::ConflictingSourceDatum);
                }
                check_frames(frame, other_frame)
            }
            (Evidence::Accounting(subject), Evidence::Accounting(other)) => {
                subject.clone().merge(other.clone())
            }
            _ => Err(ExportAccountingError::UnboundHealthSubject),
        }
    }

    #[cfg(test)]
    pub(super) const fn recipe_for_test(&self) -> Option<&ReferenceRecipe> {
        match &self.evidence {
            Evidence::Original { recipe, .. } => Some(recipe),
            Evidence::Accounting(_) => None,
        }
    }
}

impl PartialEq for CatalogReferenceProof {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data && self.subject().key() == other.subject().key()
    }
}

impl Eq for CatalogReferenceProof {}

#[cfg(test)]
mod tests {
    use super::{
        CatalogReferenceData, CatalogReferenceProof, RecordSubject, ReferenceObservation,
        ReferenceRecipe,
    };
    use crate::{ExportAccountingError, HealthText};
    use p4kx_extraction::Guid;

    #[test]
    fn candidate_storage_excludes_negative_shape_and_separate_recipe_allocation() {
        assert!(
            size_of::<CatalogReferenceProof>()
                < size_of::<ReferenceObservation>() + size_of::<ReferenceRecipe>(),
            "inline proof is smaller than the prior row and recipe payload, even before Arc overhead"
        );
    }

    #[test]
    fn unbound_accounting_reuses_the_description_without_becoming_original_evidence()
    -> Result<(), ExportAccountingError> {
        let data = CatalogReferenceData {
            field: HealthText::new("same"),
            source_type: HealthText::new("Caller"),
            target_guid: Guid::default(),
            target_record_index: 7,
            target_record_path: HealthText::new("fixture/target.xml"),
            target_type: HealthText::new("Tag"),
        };
        let subject = RecordSubject::accounting(2, Guid::default(), None);
        let first = CatalogReferenceProof::accounting(subject.clone(), data.clone());
        let second = CatalogReferenceProof::accounting(subject, data);
        first.check_duplicate(&second)?;
        let original = first.materialize(None)?;
        assert_eq!(
            original.data().target_record_index,
            Some(7),
            "typed target is preserved"
        );
        assert!(
            matches!(
                original.evidence,
                super::super::health_reference_observation::ReferenceEvidence::Accounting(_)
            ),
            "accounting state stays unbound"
        );
        Ok(())
    }
}
