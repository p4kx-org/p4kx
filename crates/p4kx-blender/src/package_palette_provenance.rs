// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_extraction::{FieldOrigins, PhysicalSourceRef, SourceInventory, SourceUses};
use schemars::JsonSchema;
use serde::Serialize;

use crate::collect::Collected;
use crate::items::Item;
use crate::palette_fields;
use crate::palette_selection::{self, Record, Selection};
use crate::plan::MeshPlan;
use crate::{PackageError, PackageResult, PackageWriteContext};

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PaletteProvenance {
    payloads: Vec<Payload>,
    selections: Vec<Selection>,
    bindings: Vec<Binding>,
    source_inventory: SourceInventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Payload {
    palette: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    record: Option<Record>,
    field_origins: FieldOrigins,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Binding {
    node: usize,
    payload: usize,
    selection: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    inherited_from: Option<usize>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum PayloadInput {
    Observed {
        source: PhysicalSourceRef,
        record: u32,
    },
    Unproved {
        value_identity: usize,
    },
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct PayloadKey {
    palette: usize,
    input: PayloadInput,
}

#[derive(Default)]
struct Writer {
    sources: SourceUses,
    payloads: Vec<Payload>,
    selections: Vec<Selection>,
    bindings: Vec<Binding>,
    indexed_payloads: BTreeMap<PayloadKey, usize>,
    indexed_selections: HashMap<palette_selection::SelectionKey, usize>,
}

impl Writer {
    fn bind(&mut self, node: usize, palette: usize, item: &Item<'_>) -> PackageResult<()> {
        let Some((drawn, colours, source)) = item
            .drawn
            .and_then(|drawn| Some((drawn, drawn.palette?, drawn.palette_source?)))
        else {
            return Ok(());
        };
        let records = source.records();
        let scope = if records.palette.is_some() || records.selector.is_some() {
            palette_selection::scope(&self.sources, source.forge_origin())?
        } else {
            None
        };
        let record = records.palette;
        let input = match (scope.as_ref(), record) {
            (Some(scope), Some(record)) => PayloadInput::Observed {
                source: scope.source_ref().clone(),
                record: record.index,
            },
            _ => PayloadInput::Unproved {
                value_identity: std::ptr::from_ref(colours).addr(),
            },
        };
        let key = PayloadKey { palette, input };
        let payload = match self.indexed_payloads.entry(key) {
            std::collections::btree_map::Entry::Occupied(row) => *row.get(),
            std::collections::btree_map::Entry::Vacant(row) => {
                let position = self.payloads.len();
                self.payloads.push(Payload {
                    palette,
                    record: record
                        .map(|record| Record::capture(&self.sources, record, scope.as_ref()))
                        .transpose()?,
                    field_origins: palette_fields::capture(
                        &self.sources,
                        scope.as_ref(),
                        record.map(|record| record.index),
                        colours,
                    )?,
                });
                row.insert(position);
                position
            }
        };
        let selection = Selection::capture(&self.sources, source, scope.as_ref())?;
        let selection = *self
            .indexed_selections
            .entry(selection.key())
            .or_insert_with(|| {
                let position = self.selections.len();
                self.selections.push(selection);
                position
            });
        self.bindings.push(Binding {
            node,
            payload,
            selection,
            inherited_from: drawn.palette_parent,
        });
        Ok(())
    }
}

pub(crate) fn capture(
    items: &[Item<'_>],
    plan: &MeshPlan,
    collected: &Collected,
    context: &PackageWriteContext<'_>,
) -> PackageResult<Option<serde_json::Value>> {
    let mut writer = Writer {
        sources: SourceUses::with_compact_use_refs(),
        ..Writer::default()
    };
    for (node, item) in items.iter().enumerate() {
        context.check()?;
        let palette = plan
            .drawn_by(node)
            .and_then(|mesh| collected.palette_of.get(mesh))
            .copied()
            .flatten();
        if let Some(palette) = palette {
            writer.bind(node, palette, item)?;
        }
    }
    if writer.bindings.is_empty() {
        return Ok(None);
    }
    serde_json::to_value(PaletteProvenance {
        payloads: writer.payloads,
        selections: writer.selections,
        bindings: writer.bindings,
        source_inventory: writer.sources.freeze().map_err(error)?,
    })
    .map(Some)
    .map_err(error)
}

pub(super) fn error(error: impl std::error::Error + Send + Sync + 'static) -> PackageError {
    PackageError::PaletteEvidence(Box::new(error))
}
