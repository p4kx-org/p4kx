// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;

use p4kx_assembly::{EntityRig, RigPlacement};
use p4kx_extraction::query::{CompiledQuery, Query, QueryCellSources, QueryErrorKind, QuerySource};
use p4kx_extraction::{DataForge, Value, ValueError};

use crate::{DirectoryLookup, MediaError, MediaWriter};

const FIELDS: [ComponentField; 2] = [
    ComponentField {
        component: "SAnimatedCharacterComponentParams",
        name: "animDatabase3P",
    },
    ComponentField {
        component: "SAnimationControllerParams",
        name: "AnimationDatabase",
    },
];

pub(super) struct Selector<'a> {
    pub(super) rig: &'a EntityRig,
    pub(super) authored: &'a str,
    pub(super) component: usize,
    pub(super) field: &'static str,
    pub(super) record_index: u32,
}

struct ComponentField {
    component: &'static str,
    name: &'static str,
}

struct FieldQuery<'source> {
    query: CompiledQuery<'source>,
    name: &'static str,
}

struct Observed<'source> {
    authored: &'source str,
    component: usize,
    field: &'static str,
}

pub(super) fn visit<'source>(
    forge: &'source DataForge<'source>,
    rigs: &[EntityRig],
    lookup: &DirectoryLookup<'_>,
    writer: &mut MediaWriter<'_>,
    mut consume: impl FnMut(Selector<'_>, &mut MediaWriter<'_>) -> Result<(), MediaError>,
) -> Result<(), MediaError> {
    let mut queries = Vec::new();
    for field in FIELDS {
        writer.checkpoint()?;
        if let Some(query) = field.compile(forge)? {
            queries.push(query);
        }
    }
    let mut owners = HashMap::<u32, Vec<Observed<'source>>>::new();
    for rig in rigs {
        writer.checkpoint()?;
        if !matches!(
            rig.placement,
            RigPlacement::Root | RigPlacement::Attachment { .. }
        ) {
            continue;
        }
        let Some(record_index) = rig.record_index else {
            continue;
        };
        let values: &[Observed<'source>] = match owners.entry(record_index) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(observe(&queries, record_index, lookup, writer)?),
        };
        for value in values {
            writer.checkpoint()?;
            consume(
                Selector {
                    rig,
                    authored: value.authored,
                    component: value.component,
                    field: value.field,
                    record_index,
                },
                writer,
            )?;
        }
    }
    Ok(())
}

impl ComponentField {
    fn compile<'source>(
        &self,
        forge: &'source DataForge<'source>,
    ) -> Result<Option<FieldQuery<'source>>, MediaError> {
        let query = Query::parse(&format!(
            "from EntityClassDefinition select Components[{}].{}",
            self.component, self.name,
        ))
        .map_err(invalid)?;
        match query.compile_with(forge, None) {
            Ok(query) => Ok(Some(FieldQuery {
                query,
                name: self.name,
            })),
            Err(error) if self.absent(error.kind()) => Ok(None),
            Err(error) => Err(invalid(error)),
        }
    }

    fn absent(&self, kind: &QueryErrorKind) -> bool {
        match kind {
            QueryErrorKind::UnknownType { name, .. } => name == self.component,
            QueryErrorKind::UnknownField { name, .. } => name == self.name || name == "Components",
            _ => false,
        }
    }
}

fn observe<'source>(
    queries: &[FieldQuery<'source>],
    record: u32,
    lookup: &DirectoryLookup<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Vec<Observed<'source>>, MediaError> {
    let mut values = Vec::new();
    for field in queries {
        field.observe(record, lookup, writer, &mut values)?;
    }
    values.sort_by_key(|value| value.component);
    Ok(values)
}

impl<'source> FieldQuery<'source> {
    fn observe(
        &self,
        record: u32,
        lookup: &DirectoryLookup<'_>,
        writer: &MediaWriter<'_>,
        output: &mut Vec<Observed<'source>>,
    ) -> Result<(), MediaError> {
        let budget = &lookup.input_state()?.budget;
        let limits = writer.limits();
        budget.metadata(0, 1, limits, writer.checkpoint)?;
        let allowance =
            usize::try_from(budget.remaining_metadata_work(limits)?).unwrap_or(usize::MAX);
        let row = self
            .query
            .physical_row_observed(record, allowance)
            .ok_or_else(|| invalid("missing physical owner"))?;
        budget.metadata(0, row.visited_elements() as u64, limits, writer.checkpoint)?;
        let cells = row.cells().map_err(invalid)?;
        let values = match cells {
            [None] => return Ok(()),
            [Some(Value::Array(values))] => values,
            [Some(Value::Error(ValueError::PathTooWide { .. }))] => {
                return Err(work_limit(limits.metadata_work));
            }
            _ => return Err(invalid("expected Components array locator column")),
        };
        let originals = row
            .cell_sources()
            .and_then(|columns| columns.first())
            .map(QueryCellSources::values)
            .filter(|originals| originals.len() == values.len())
            .ok_or_else(|| invalid("missing original locator positions"))?;
        for (value, sites) in values.iter().zip(originals) {
            writer.checkpoint()?;
            match value {
                Value::String(authored) => output.push(Observed {
                    authored,
                    component: component(sites)?,
                    field: self.name,
                }),
                Value::Null => {}
                Value::Error(ValueError::PathTooWide { .. }) => {
                    return Err(work_limit(limits.metadata_work));
                }
                Value::Error(cause) => return Err(invalid(cause)),
                _ => return Err(invalid("expected text locator")),
            }
        }
        Ok(())
    }
}

fn component(sites: &[QuerySource]) -> Result<usize, MediaError> {
    sites
        .iter()
        .find_map(|site| match *site {
            QuerySource::ArrayElement { index, .. } => Some(index as usize),
            _ => None,
        })
        .ok_or_else(|| invalid("missing original Components slot"))
}

fn invalid(cause: impl fmt::Display) -> MediaError {
    MediaError::Contract(format!(
        "cannot read authored Mannequin database locator: {cause}"
    ))
}

const fn work_limit(limit: u64) -> MediaError {
    MediaError::Limit {
        budget: "metadata_work",
        limit,
    }
}
