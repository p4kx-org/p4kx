// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Integration tests of `p4kx-comparison`, built as one test binary.

mod common;
mod container_entries;
mod duplicate_guids;
mod entries;
mod entry_aggregate_origins;
mod entry_collisions;
mod entry_decision_origins;
mod entry_incomparables;
mod forge_pair;
mod line_diff;
mod line_diff_content;
mod line_diff_origins;
mod line_diff_viewer;
mod nested_diff;
mod owned;
mod owned_records;
mod public_patch;
mod record_body_origins;
mod record_fields;
mod record_header_origins;
mod record_name_tag;
mod record_nested;
mod record_pointer_route_origins;
mod record_reference_origins;
mod record_references;
mod records;
mod schema_counts;
mod schema_diff;
mod unreadable_records;
