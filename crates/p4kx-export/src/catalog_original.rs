// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Facts captured during one admitted canonical Record read.

#[path = "catalog_failure_details.rs"]
mod catalog_failure_details;
#[path = "health_failure_finish.rs"]
mod health_failure_finish;
#[path = "health_failure_human.rs"]
mod health_failure_human;
#[path = "health_failure_reason.rs"]
mod health_failure_reason;
#[path = "health_record_capture.rs"]
mod health_record_capture;
#[path = "health_text_capture.rs"]
mod health_text_capture;

#[path = "health_cell_order.rs"]
mod health_cell_order;
#[path = "health_diagnostic_cells.rs"]
mod health_diagnostic_cells;
#[path = "health_diagnostic_subject.rs"]
mod health_diagnostic_subject;
#[path = "health_failed_record.rs"]
mod health_failed_record;
#[path = "health_header_receipt.rs"]
mod health_header_receipt;
#[path = "health_record_cells.rs"]
mod health_record_cells;
#[path = "health_record_key.rs"]
mod health_record_key;
#[path = "health_record_subject.rs"]
mod health_record_subject;
#[path = "health_report_retention.rs"]
mod health_report_retention;

pub use catalog_failure_details::CatalogFailureDetails;
pub use health_diagnostic_cells::{ExportDiagnostic, ExportDiagnosticExample};
pub use health_failed_record::FailedExportRecord;
pub(crate) use health_failure_finish::finish_report;
pub(crate) use health_record_key::RecordSubjectKey;
pub(crate) use health_record_subject::RecordSubject;
pub(crate) use health_text_capture::HealthTextCapture;

#[path = "original_datum.rs"]
mod original_datum;
#[path = "original_flow_fields.rs"]
mod original_flow_fields;
#[path = "original_instance_capture.rs"]
mod original_instance_capture;
#[path = "original_instance_location.rs"]
mod original_instance_location;
#[path = "original_reference_admission.rs"]
mod original_reference_admission;
#[path = "original_reference_capture.rs"]
mod original_reference_capture;
#[path = "original_struct_ancestry.rs"]
mod original_struct_ancestry;
#[path = "original_target_shape.rs"]
mod original_target_shape;
#[path = "original_value_capture.rs"]
mod original_value_capture;
#[path = "original_owned_values.rs"]
mod owned;

pub(crate) use crate::build::original_input::OriginalRecordScope;
pub(crate) use original_datum::OriginalDatum;
#[path = "original_reward_read.rs"]
mod original_reward_read;
pub(crate) use original_flow_fields::{OriginalFlowFields, OriginalMissingField};
pub(crate) use original_instance_location::OriginalInstanceLocation;
pub(crate) use original_reference_capture::OriginalReferenceObservation;
pub(crate) use original_reward_read::RewardRead;
pub(crate) use original_target_shape::{OriginalReferenceCapture, OriginalTargetShape};
pub(crate) use owned::{
    OriginalContainerKind, OriginalErrorParameter, OriginalValueEdge, OriginalValueEvent,
    OriginalValueFact, OriginalValuePart, OriginalValueSite, OwnedValueEvent, OwnedValueVisit,
    OwnedValueWalk,
};

#[path = "health_reference_capture.rs"]
mod health_reference_capture;
#[path = "health_reference_cells.rs"]
mod health_reference_cells;
#[path = "health_reference_data.rs"]
mod health_reference_data;
#[path = "health_reference_error.rs"]
mod health_reference_error;
#[path = "health_reference_finish.rs"]
mod health_reference_finish;
#[path = "health_reference_link.rs"]
mod health_reference_link;
#[path = "health_reference_observation.rs"]
mod health_reference_observation;
#[path = "health_reference_path.rs"]
mod health_reference_path;
#[path = "health_reference_receipt.rs"]
mod health_reference_receipt;
#[path = "health_reference_target.rs"]
mod health_reference_target;
#[path = "original_error_input.rs"]
mod original_error_input;

pub(crate) use health_reference_capture::ReferenceCapture;
pub use health_reference_cells::{ExportSourcePointer, ExportUnresolvedReference};
pub(crate) use health_reference_data::ReferenceData;
pub(crate) use health_reference_link::ReferenceLink;
pub(crate) use health_reference_observation::ReferenceObservation;

#[path = "health_example_body.rs"]
mod health_example_body;
#[path = "health_example_capture.rs"]
mod health_example_capture;
#[path = "health_example_cells.rs"]
mod health_example_cells;
#[path = "health_example_retention.rs"]
mod health_example_retention;
#[path = "health_value_chain.rs"]
mod health_value_chain;

pub(crate) use health_example_body::ExampleBodyReceipt;
pub(crate) use health_example_capture::{ExampleBodyCapture, FieldFailureKind};

#[path = "original_dangling_reference.rs"]
mod original_dangling_reference;
#[path = "original_record_name.rs"]
mod original_record_name;
#[path = "original_tag_capture.rs"]
mod original_tag_capture;

#[path = "health_entry_frame.rs"]
mod health_entry_frame;
#[path = "health_entry_loadout_xml.rs"]
mod health_entry_loadout_xml;
#[path = "health_entry_path_capture.rs"]
mod health_entry_path_capture;
#[path = "health_entry_selection.rs"]
mod health_entry_selection;
#[path = "health_loadout_body.rs"]
mod health_loadout_body;

pub(crate) use health_entry_frame::{EntryCapture, EntryFrame};
pub(crate) use health_entry_path_capture::EntryPathCapture;
pub(crate) use health_entry_selection::AuthoredEntryPath;
pub(crate) use health_loadout_body::LoadoutBodyReceipt;

#[path = "health_loadout_graph.rs"]
mod health_loadout_graph;

pub(crate) use health_loadout_graph::{FileGraph, FileSelection};

#[cfg(test)]
#[path = "health_number_body_tests.rs"]
mod health_number_body_tests;

#[path = "health_entry_cells.rs"]
mod health_entry_cells;
#[path = "health_entry_diagnosis.rs"]
mod health_entry_diagnosis;
#[path = "health_entry_json.rs"]
mod health_entry_json;
#[path = "health_entry_metadata.rs"]
mod health_entry_metadata;
#[path = "health_entry_observation.rs"]
mod health_entry_observation;
#[path = "health_entry_read.rs"]
mod health_entry_read;
#[path = "health_entry_receipt.rs"]
mod health_entry_receipt;
#[path = "health_entry_retention.rs"]
mod health_entry_retention;
#[path = "health_entry_xml.rs"]
mod health_entry_xml;
#[path = "health_failed_entry.rs"]
mod health_failed_entry;

pub(crate) use health_entry_diagnosis::EntryDiagnosis;
#[cfg(test)]
pub(crate) use health_entry_frame::with_entry_capture;
pub(crate) use health_entry_json::{EntryJson, JsonEvent};
pub(crate) use health_entry_observation::FailedEntryObservation;
pub(crate) use health_entry_read::{EntryRead, EntryReadError};
pub(crate) use health_entry_receipt::{EntryReceipt, PhysicalEntryIdentity, PhysicalEntryRef};
pub(crate) use health_entry_xml::{EntryXml, XmlAttributeSource, XmlNodeSource};
pub use health_failed_entry::FailedExportEntry;

#[path = "health_example_description.rs"]
mod health_example_description;
#[path = "health_example_header.rs"]
mod health_example_header;
#[path = "health_loadout_example.rs"]
mod health_loadout_example;

#[path = "health_loadout_target.rs"]
mod health_loadout_target;
#[path = "health_loadout_target_example.rs"]
mod health_loadout_target_example;
pub(crate) use health_loadout_target::{LoadoutTargetCapture, LoadoutTargetReceipt};

#[path = "health_tag_relation.rs"]
mod health_tag_relation;
pub(crate) use health_tag_relation::TagRelationReceipts;
#[cfg(test)]
#[path = "health_loadout_class_tests.rs"]
mod health_loadout_class_tests;

#[path = "authored_entry_text.rs"]
mod authored_entry_text;
#[path = "entry_selection_input.rs"]
mod entry_selection_input;
#[path = "health_loadout_class_name.rs"]
mod health_loadout_class_name;
#[path = "health_loadout_class_target.rs"]
mod health_loadout_class_target;
pub(crate) use health_loadout_class_name::LoadoutClassNameReceipt;
