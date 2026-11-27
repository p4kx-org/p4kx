// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{Loadout, LoadoutPort};
use p4kx_export::catalog_queries::tags::TagPathRow;

use super::rows::Rows;

pub(super) fn loadout(rows: &mut Rows, row: &Loadout) {
    rows.observe_completeness(row.source_record_index().origin());
    rows.observe_completeness(row.source_entity_guid().origin());
    rows.observe_completeness(row.source_entity_kind().origin());
    for port in row.ports() {
        loadout_port(rows, port);
    }
}

fn loadout_port(rows: &mut Rows, port: &LoadoutPort) {
    for origin in [
        port.name().origin(),
        port.source_min_size().origin(),
        port.source_max_size().origin(),
        port.source_flags().origin(),
        port.source_port_tags().origin(),
        port.source_required_tags().origin(),
        port.source_display_name_key().origin(),
    ] {
        rows.observe_completeness(origin);
    }
    if let Some(attachment) = port.source_constraints().attachment() {
        rows.observe_completeness(attachment.target_guid().origin());
        rows.observe_completeness(attachment.target_record_index().origin());
    }
    for child in port.ports() {
        loadout_port(rows, child);
    }
}

pub(super) fn tag_path(rows: &mut Rows, row: &TagPathRow<'_>) {
    for origin in [
        row.guid().origin(),
        row.record_index().origin(),
        row.path().origin(),
        row.depth().origin(),
        row.listed().origin(),
        row.parent_origin(),
    ] {
        rows.observe_completeness(origin);
    }
    if let Some(parent) = row.parent_header() {
        rows.observe_completeness(parent.guid.origin());
        rows.observe_completeness(parent.record_index.origin());
    }
}
