// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::query::{CompiledQuery, Query, QueryErrorKind, QuerySource, Row};
use p4kx_extraction::{DATAFORGE_ENTRIES, DataForge, DataForgeEntry, Record, Value, ValueError};

use super::origins::ModelOrigins;
use super::vehicle_grid_record::{self as records, GridDatabaseState};
use crate::{MediaError, MediaWriter, Projection, SourceContext, required_reader};

const FIELD: &str = "Components[VehicleComponentParams].physicsGrid.physicsProxy.path";
const QUERY: &str = "from EntityClassDefinition select Components[VehicleComponentParams].physicsGrid.physicsProxy.path";

pub(super) fn is_candidate(context: &SourceContext<'_>) -> bool {
    let path = context.entry.path().replace('/', "\\");
    DATAFORGE_ENTRIES
        .iter()
        .any(|name| path.eq_ignore_ascii_case(name))
}

pub(super) fn emit(
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    let archive = context.directory.index.archive;
    if DataForgeEntry::locate(archive).is_none_or(|entry| entry.index() != context.entry.index()) {
        return Ok(Projection::Skipped);
    }
    let input = context.origin_input(writer)?;
    let origins = ModelOrigins {
        input: &input,
        entry: context.entry.index(),
    };
    let bytes = required_reader::read_all(context, writer)?;
    writer.checkpoint()?;
    let outcome = Dispatch {
        context,
        origins: &origins,
    };
    if let Ok(forge) = DataForge::open(&bytes) {
        outcome.rows(&forge, writer)?;
    } else {
        records::summary(
            GridDatabaseState::InvalidDatabase,
            bytes.len(),
            &origins,
            writer,
        )?;
    }
    Ok(Projection::Written)
}

struct Dispatch<'context, 'input> {
    context: &'context SourceContext<'input>,
    origins: &'context ModelOrigins<'input>,
}

impl Dispatch<'_, '_> {
    fn rows(&self, forge: &DataForge<'_>, writer: &mut MediaWriter<'_>) -> Result<(), MediaError> {
        let query = Query::parse(QUERY).map_err(|cause| MediaError::Contract(cause.to_string()))?;
        match query.compile_with(forge, None) {
            Ok(compiled) => {
                records::summary(
                    GridDatabaseState::Ready,
                    forge.byte_len(),
                    self.origins,
                    writer,
                )?;
                for record in forge.records() {
                    self.record(record, &compiled, writer)?;
                }
            }
            Err(error) => {
                let failure = match error.kind() {
                    QueryErrorKind::UnknownType { .. } | QueryErrorKind::UnknownField { .. } => {
                        GridDatabaseState::AbsentSchema
                    }
                    _ => GridDatabaseState::UnsupportedSchema,
                };
                records::summary(failure, forge.byte_len(), self.origins, writer)?;
            }
        }
        Ok(())
    }

    fn record(
        &self,
        record: Record<'_>,
        compiled: &CompiledQuery<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let budget = &self.context.directory.input_state()?.budget;
        let limits = writer.limits();
        budget.metadata(0, 1, limits, writer.checkpoint)?;
        let limit = usize::try_from(budget.remaining_metadata_work(limits)).unwrap_or(usize::MAX);
        let Some(row) = compiled.physical_row_observed(record.index(), limit) else {
            return Ok(());
        };
        budget.metadata(0, row.visited_elements() as u64, limits, writer.checkpoint)?;
        match row.cells() {
            Ok([Some(Value::Array(values))]) => self.occurrences(&row, values, writer),
            Ok([Some(Value::Error(ValueError::PathTooWide { .. }))]) => Err(MediaError::Limit {
                budget: "metadata_work",
                limit: limits.metadata_work,
            }),
            Ok([None]) => Ok(()),
            Ok(_) | Err(_) => records::row_failure(
                record,
                "vehicle grid Record layout could not be read",
                self.origins,
                writer,
            ),
        }
    }

    fn occurrences(
        &self,
        row: &Row<'_>,
        values: &[Value<'_>],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let sources = row
            .cell_sources()
            .and_then(|sites| sites.first())
            .ok_or_else(|| {
                MediaError::Contract("observed vehicle grid row lost its column source".into())
            })?;
        for (index, value) in values.iter().enumerate() {
            writer.checkpoint()?;
            let sites = sources.values().get(index).ok_or_else(|| {
                MediaError::Contract("vehicle grid occurrence has no original coordinate".into())
            })?;
            let component_index = sites.iter().find_map(|source| match *source {
                QuerySource::ArrayElement { index, .. } => Some(index),
                _ => None,
            });
            let (Some(component_index), Some(state)) =
                (component_index, records::PathState::from_value(value))
            else {
                return records::row_failure(
                    row.record(),
                    "the vehicle Components array could not be read",
                    self.origins,
                    writer,
                );
            };
            records::occurrence(
                records::GridOccurrence {
                    record: row.record(),
                    component_index,
                    field: FIELD,
                    state,
                    sites,
                },
                self.context.directory,
                self.origins,
                writer,
            )?;
        }
        Ok(())
    }
}
