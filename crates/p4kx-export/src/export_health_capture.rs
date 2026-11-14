// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "catalog_provenance.rs"]
mod catalog_provenance;

pub use catalog_provenance::CatalogProvenance;

#[path = "export_diagnostic.rs"]
pub(crate) mod export_diagnostic;

#[path = "export_entry_counts.rs"]
pub(crate) mod export_entry_counts;

#[path = "export_entry_ledger.rs"]
pub(crate) mod export_entry_ledger;

#[path = "export_health.rs"]
pub(crate) mod export_health;

#[path = "export_health_api.rs"]
pub(crate) mod export_health_api;

#[path = "export_health_aggregate.rs"]
pub(crate) mod export_health_aggregate;

#[path = "export_health_prepared.rs"]
pub(crate) mod export_health_prepared;

#[path = "health_diagnostic_observation.rs"]
pub(crate) mod health_diagnostic_observation;

#[path = "health_example_samples.rs"]
pub(crate) mod health_example_samples;

#[path = "health_failed_observation.rs"]
pub(crate) mod health_failed_observation;

#[path = "export_health_compile.rs"]
pub(crate) mod export_health_compile;

#[path = "export_health_derived.rs"]
pub(crate) mod export_health_derived;

#[path = "export_health_entries.rs"]
pub(crate) mod export_health_entries;

#[path = "export_health_fields.rs"]
pub(crate) mod export_health_fields;

#[path = "export_health_records.rs"]
pub(crate) mod export_health_records;

#[path = "export_health_warning.rs"]
pub(crate) mod export_health_warning;

#[path = "export_ledger.rs"]
pub(crate) mod export_ledger;

#[path = "export_reference_issue.rs"]
pub(crate) mod export_reference_issue;

#[path = "catalog_failures.rs"]
pub(crate) mod catalog_failures;

#[path = "catalog_read_failure.rs"]
pub(crate) mod catalog_read_failure;

#[path = "export_health_prepare.rs"]
pub(crate) mod export_health_prepare;

#[path = "failure_text.rs"]
pub(crate) mod failure_text;

#[path = "health_metadata.rs"]
pub(crate) mod health_metadata;

#[path = "health_text_view.rs"]
pub(crate) mod health_text_view;

pub(crate) use catalog_read_failure::CatalogReadFailure;

#[path = "health_text.rs"]
mod health_text;
