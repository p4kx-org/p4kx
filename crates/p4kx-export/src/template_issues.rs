// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Mutex;

use p4kx_extraction::StructValue;
use thiserror::Error;

use crate::catalog_read::original::ExampleBodyReceipt;
use crate::health_diagnostic_observation::ExampleObservation;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::{CatalogError, ExportAccountingError};

#[path = "template_error_description.rs"]
mod description;
#[path = "template_warning_evidence.rs"]
mod warning_evidence;

pub(crate) use description::TemplateDescription;
use warning_evidence::TemplateWarningEvidence;

/// A contract template whose structure the catalog does not follow to its end.
#[derive(Debug, Error)]
pub(crate) enum TemplateError {
    /// A field that holds one of several structs holds one the catalog does not know.
    #[error("{field} holds the unknown struct {name}")]
    UnknownStruct {
        /// The field, e.g. `modifiers`.
        field: &'static str,
        /// The struct name.
        name: String,
    },
    #[error("{error}")]
    Captured {
        #[source]
        error: Box<Self>,
        body: ExampleBodyReceipt,
    },
}

impl TemplateError {
    pub(crate) fn with_body(self, body: ExampleBodyReceipt) -> Self {
        Self::Captured {
            error: Box::new(self),
            body,
        }
    }

    pub(crate) const fn body(&self) -> Option<&ExampleBodyReceipt> {
        match self {
            Self::Captured { body, .. } => Some(body),
            Self::UnknownStruct { .. } => None,
        }
    }

    /// Returns the error for the struct `value` in the field `field`.
    pub(crate) fn unknown(field: &'static str, value: &StructValue<'_>) -> Self {
        Self::UnknownStruct {
            field,
            name: value.type_name().to_owned(),
        }
    }
}

/// Counts unsupported vocabulary by technical category.
#[derive(Debug, Default)]
pub(crate) struct Unknowns(Mutex<WarningState>);

#[derive(Debug, Default)]
struct WarningState {
    counts: BTreeMap<Cow<'static, str>, usize>,
    evidence: TemplateWarningEvidence,
}

pub(crate) struct UnknownVocabularyReport {
    pub(crate) count: usize,
    pub(crate) details: Vec<String>,
    pub(crate) evidence: TemplateWarningEvidence,
}

impl Unknowns {
    /// Interprets an authored value and counts an unsupported value by category.
    pub(crate) fn parse<T>(
        &self,
        owner: FlowOwnerKind,
        category: &'static str,
        stored: &str,
        parse: impl FnOnce(&str) -> Option<T>,
        example: impl FnOnce() -> Result<Option<ExampleObservation>, CatalogError>,
    ) -> Result<Option<T>, CatalogError> {
        self.parse_counted(owner, stored, parse, || Cow::Borrowed(category), example)
    }

    /// Interprets a declared source type and retains its technical name when unsupported.
    pub(crate) fn parse_type<T>(
        &self,
        owner: FlowOwnerKind,
        category: &'static str,
        source_type: &str,
        parse: impl FnOnce(&str) -> Option<T>,
        example: impl FnOnce() -> Result<Option<ExampleObservation>, CatalogError>,
    ) -> Result<Option<T>, CatalogError> {
        self.parse_counted(
            owner,
            source_type,
            parse,
            || Cow::Owned(format!("{category} {source_type}")),
            example,
        )
    }

    fn parse_counted<T>(
        &self,
        owner: FlowOwnerKind,
        stored: &str,
        parse: impl FnOnce(&str) -> Option<T>,
        label: impl FnOnce() -> Cow<'static, str>,
        example: impl FnOnce() -> Result<Option<ExampleObservation>, CatalogError>,
    ) -> Result<Option<T>, CatalogError> {
        let parsed = parse(stored);
        if parsed.is_none() {
            let example = example()?;
            let mut state = crate::export_evidence_lock::lock(&self.0)?;
            let count = state.counts.entry(label()).or_insert(0);
            *count = count
                .checked_add(1)
                .ok_or(ExportAccountingError::CounterOverflow)?;
            state.evidence.observe(owner, example)?;
        }
        Ok(parsed)
    }

    /// Returns exact category counts and bounded original field examples.
    pub(crate) fn into_report(self) -> Result<UnknownVocabularyReport, CatalogError> {
        let WarningState { counts, evidence } = self
            .0
            .into_inner()
            .map_err(|_| ExportAccountingError::PoisonedEvidence)?;
        let count = counts.values().try_fold(0_usize, |total, count| {
            total
                .checked_add(*count)
                .ok_or(ExportAccountingError::CounterOverflow)
        })?;
        let details = counts
            .into_iter()
            .map(|(name, count)| format!("{name} ({count})"))
            .collect();
        Ok(UnknownVocabularyReport {
            count,
            details,
            evidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Unknowns;
    use crate::mission_flow_owner::FlowOwnerKind;
    use crate::{CatalogError, ExportAccountingError, export_evidence_lock};

    #[test]
    fn vocabulary_counts_do_not_require_a_retained_example() -> Result<(), CatalogError> {
        use FlowOwnerKind::{ContractTemplate, MissionBrokerEntry};

        let unknowns = Unknowns::default();
        for owner in [MissionBrokerEntry, ContractTemplate, MissionBrokerEntry] {
            assert_eq!(
                unknowns.parse(owner, "category", "synthetic", |_| None::<()>, || Ok(None))?,
                None,
                "unsupported vocabulary leaves the typed value empty"
            );
        }
        let report = unknowns.into_report()?;
        assert_eq!(report.count, 3, "one build-wide compatibility count");
        assert_eq!(
            report.details,
            ["category (3)"],
            "one build-wide category count"
        );
        let diagnostics: Vec<_> = report.evidence.into_diagnostics("warning").collect();
        assert_eq!(
            diagnostics.len(),
            2,
            "one diagnostic per affected owner family"
        );
        for (diagnostic, owner, count) in [
            (&diagnostics[0], ContractTemplate, 1),
            (&diagnostics[1], MissionBrokerEntry, 2),
        ] {
            assert_eq!(
                diagnostic.kind,
                Some(owner.catalog_kind()),
                "physical owner kind"
            );
            assert_eq!(diagnostic.count, count, "exact per-kind count");
            assert!(diagnostic.examples.is_empty(), "no invented example");
        }
        Ok(())
    }

    #[test]
    fn poisoned_warning_accounting_refuses_both_observation_and_finish() {
        let unknowns = Unknowns::default();
        assert!(
            export_evidence_lock::poison(&unknowns.0),
            "the actual warning lock is poisoned"
        );
        assert!(
            matches!(
                unknowns.parse(
                    FlowOwnerKind::ContractTemplate,
                    "category",
                    "synthetic",
                    |_| None::<()>,
                    || Ok(None)
                ),
                Err(CatalogError::Accounting(
                    ExportAccountingError::PoisonedEvidence
                ))
            ),
            "a rejected warning observation cannot report success"
        );
        assert!(
            matches!(
                unknowns.into_report(),
                Err(CatalogError::Accounting(
                    ExportAccountingError::PoisonedEvidence
                ))
            ),
            "a poisoned warning collector cannot silently produce an empty report"
        );
    }
}
