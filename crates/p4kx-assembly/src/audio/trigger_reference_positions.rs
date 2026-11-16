// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{
    DataForgeError, PositionedFields, PositionedRecord, PositionedSources, PositionedValue,
    RecordJsonBytes, RecordJsonRecordField, RecordJsonSource, RecordJsonUse, RecordJsonUseRole,
    Value,
};

use super::{Position, TriggerReference, TriggerScope, Visit, Walk};

/// A trigger paired with each original field that names it.
#[derive(Debug)]
pub struct PositionedTriggerReference {
    reference: TriggerReference,
    occurrences: Vec<TriggerOccurrence>,
}

impl PositionedTriggerReference {
    /// Returns the trigger's first spelling, first location and count.
    #[must_use]
    pub const fn reference(&self) -> &TriggerReference {
        &self.reference
    }

    /// Returns the contributing fields in discovery order.
    #[must_use]
    pub fn occurrences(&self) -> &[TriggerOccurrence] {
        &self.occurrences
    }

    /// Splits the reference from its ordered original occurrences.
    #[must_use]
    pub fn into_parts(self) -> (TriggerReference, Vec<TriggerOccurrence>) {
        (self.reference, self.occurrences)
    }
}

/// A trigger name's original field and the selectors on its actual route.
#[derive(Debug)]
pub struct TriggerOccurrence {
    sources: Vec<RecordJsonUse>,
    complete: bool,
}

impl TriggerOccurrence {
    /// Returns the original name bytes and all observed local and route contributions.
    #[must_use]
    pub fn sources(&self) -> &[RecordJsonUse] {
        &self.sources
    }

    /// Returns whether each required original contributor is present.
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.complete
    }
}

/// Discovers triggers and captures their actual fields from the same positioned Record tree.
///
/// # Errors
/// Returns a source-layout or slot-allocation error.
pub fn positioned_trigger_references_in(
    record: &PositionedRecord<'_>,
    scope: TriggerScope,
) -> Result<Vec<PositionedTriggerReference>, DataForgeError> {
    let mut walk = Walk {
        scope,
        ..Walk::default()
    };
    let mut capture = Capture {
        root: record.root_fields()?,
        record: record.tree().record_index(),
        route: Vec::new(),
        occurrences: Vec::new(),
    };
    walk.structure(record.tree().root(), &mut |visit| capture.visit(visit))?;
    if walk.found.len() != capture.occurrences.len() || !capture.route.is_empty() {
        return Err(DataForgeError::CanonicalTreeInvariant);
    }
    Ok(walk
        .found
        .into_iter()
        .zip(capture.occurrences)
        .map(|(reference, occurrences)| PositionedTriggerReference {
            reference,
            occurrences,
        })
        .collect())
}

struct Capture<'record, 'source> {
    root: PositionedFields<'record, 'source>,
    record: u32,
    route: Vec<Route<'record, 'source>>,
    occurrences: Vec<Vec<TriggerOccurrence>>,
}

struct Route<'record, 'source> {
    site: PositionedValue<'record, 'source>,
    fields: Option<PositionedFields<'record, 'source>>,
}

impl<'record, 'source> Capture<'record, 'source> {
    fn visit(&mut self, visit: Visit) -> Result<(), DataForgeError> {
        match visit {
            Visit::Enter(position) => {
                let site = self.enter(position)?;
                self.route.push(Route { site, fields: None });
            }
            Visit::Leave => {
                self.route
                    .pop()
                    .ok_or(DataForgeError::CanonicalTreeInvariant)?;
            }
            Visit::Trigger { index, structure } => {
                let occurrence = self.occurrence(structure)?;
                if index == self.occurrences.len() {
                    self.occurrences.push(Vec::new());
                }
                self.occurrences
                    .get_mut(index)
                    .ok_or(DataForgeError::CanonicalTreeInvariant)?
                    .push(occurrence);
            }
        }
        Ok(())
    }

    fn enter(
        &mut self,
        position: Position,
    ) -> Result<PositionedValue<'record, 'source>, DataForgeError> {
        let found = match (self.route.last_mut(), position) {
            (None, Position::Field(ordinal)) => self.root.get(ordinal),
            (Some(parent), Position::Field(ordinal)) => {
                if parent.fields.is_none() {
                    parent.fields = parent.site.fields()?;
                }
                parent
                    .fields
                    .as_ref()
                    .and_then(|fields| fields.get(ordinal))
            }
            (Some(parent), Position::Element(ordinal)) => parent.site.element(ordinal),
            (None, Position::Element(_)) => None,
        };
        found.ok_or(DataForgeError::CanonicalTreeInvariant)
    }

    fn occurrence(&self, structure: u32) -> Result<TriggerOccurrence, DataForgeError> {
        let (field, parents) = self
            .route
            .split_last()
            .ok_or(DataForgeError::CanonicalTreeInvariant)?;
        let mut occurrence = TriggerOccurrence {
            sources: vec![
                RecordJsonUse::new(
                    RecordJsonSource::Record {
                        index: self.record,
                        field: RecordJsonRecordField::Struct,
                    },
                    RecordJsonUseRole::Selector,
                ),
                RecordJsonUse::new(
                    RecordJsonSource::StructName { index: structure },
                    RecordJsonUseRole::Selector,
                ),
            ],
            complete: true,
        };
        for parent in parents {
            if parent.site.field_name().is_some() {
                occurrence.add(parent.site.field_name_sources().as_ref(), name_role);
            }
            occurrence.add(parent.site.selection_sources().as_ref(), selection_role);
        }
        occurrence.add(field.site.field_name_sources().as_ref(), name_role);
        if !matches!(field.site.value(), Value::String(_)) {
            return Err(DataForgeError::CanonicalTreeInvariant);
        }
        occurrence.add(Some(&field.site.value_sources()), value_role);
        Ok(occurrence)
    }
}

impl TriggerOccurrence {
    fn add(
        &mut self,
        sources: Option<&PositionedSources>,
        role: fn(RecordJsonSource<'static>) -> RecordJsonUseRole,
    ) {
        if let Some(sources) = sources {
            self.complete &= sources.complete();
            self.sources.extend(
                sources
                    .sources()
                    .iter()
                    .copied()
                    .map(|source| RecordJsonUse::new(source, role(source))),
            );
        } else {
            self.complete = false;
        }
    }
}

const fn name_role(_: RecordJsonSource<'_>) -> RecordJsonUseRole {
    RecordJsonUseRole::Selector
}

const fn selection_role(source: RecordJsonSource<'_>) -> RecordJsonUseRole {
    match source {
        RecordJsonSource::PropertyStorage { .. } => RecordJsonUseRole::Conversion,
        _ => RecordJsonUseRole::Selector,
    }
}

const fn value_role(source: RecordJsonSource<'_>) -> RecordJsonUseRole {
    match source {
        RecordJsonSource::Bytes {
            kind: RecordJsonBytes::Text,
            ..
        } => RecordJsonUseRole::Payload,
        _ => selection_role(source),
    }
}
