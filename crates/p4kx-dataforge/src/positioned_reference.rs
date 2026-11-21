// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::build;
use crate::positioned_record::ValueSlot;
use crate::{
    DataType, PositionedRecord, PositionedSources, PositionedValue, Record, RecordJsonBytes,
    RecordJsonRecordField, RecordJsonSource, Reference, ReferenceOrigin, Step, Value,
};

/// Original caller and target observations from a positioned Reference or weak pointer.
#[derive(Debug)]
pub struct PositionedReferenceSources {
    caller: PositionedSources,
    slot: Option<RecordJsonSource<'static>>,
    stored_guid: bool,
    target_guid: Option<RecordJsonSource<'static>>,
    record_path: PositionedSources,
    target_type: PositionedSources,
    target_route: PositionedSources,
    instance_path: Option<PositionedSources>,
}

impl PositionedReferenceSources {
    pub(crate) fn capture(
        input: &PositionedRecord<'_>,
        slot: ValueSlot<'_>,
        reference: &Reference<'_>,
    ) -> Option<Self> {
        let stored_guid = match slot.data_type {
            DataType::Reference => true,
            DataType::WeakPointer => false,
            _ => return None,
        };
        let origin = reference.original_target();
        let admitted = origin
            .record()
            .ok()
            .filter(|record| record.is_from(input.forge));
        let target_guid = admitted
            .filter(|_| !stored_guid)
            .map(|record| header(record.index(), RecordJsonRecordField::Guid));
        let record_path = record_path(input, admitted);
        let target = if origin.instance_location().is_some() {
            Target::owned(input, origin, admitted)
        } else {
            Target::record(admitted)
        };
        Some(Self {
            caller: input.describe_selection_header(slot),
            slot: locator(input, slot),
            stored_guid,
            target_guid,
            record_path,
            target_type: target.definition,
            target_route: target.route,
            instance_path: target.path,
        })
    }

    pub(crate) fn caller_storage(
        input: &PositionedRecord<'_>,
        slot: ValueSlot<'_>,
    ) -> PositionedSources {
        let mut sources = input.describe_selection_header(slot);
        sources.require(locator(input, slot));
        sources
    }

    /// Returns the caller property declaration and any selected array header.
    #[must_use]
    pub const fn caller(&self) -> &PositionedSources {
        &self.caller
    }

    /// Returns the exact decoded GUID suffix or weak pointer words.
    #[must_use]
    pub const fn slot(&self) -> Option<RecordJsonSource<'static>> {
        self.slot
    }

    /// Returns whether the GUID is stored in the caller's slot.
    #[must_use]
    pub const fn has_stored_guid(&self) -> bool {
        self.stored_guid
    }

    /// Returns the GUID declaration of an admitted weak-pointer target's owning Record.
    #[must_use]
    pub const fn target_guid(&self) -> Option<RecordJsonSource<'static>> {
        self.target_guid
    }

    /// Returns the admitted original Record filename declaration and its stored text.
    #[must_use]
    pub const fn record_path(&self) -> &PositionedSources {
        &self.record_path
    }

    /// Returns the exact admitted target type definition.
    #[must_use]
    pub const fn target_type(&self) -> &PositionedSources {
        &self.target_type
    }

    /// Returns the selectors of the owned target route, empty for a Record target.
    #[must_use]
    pub const fn target_route(&self) -> &PositionedSources {
        &self.target_route
    }

    /// Returns original names and array positions used to spell an owned target location.
    #[must_use]
    pub const fn instance_path(&self) -> Option<&PositionedSources> {
        self.instance_path.as_ref()
    }
}

fn locator(input: &PositionedRecord<'_>, slot: ValueSlot<'_>) -> Option<RecordJsonSource<'static>> {
    let bytes = slot.bytes?;
    let bytes = match slot.data_type {
        DataType::Reference => build::reference_guid_bytes(bytes)?.as_slice(),
        DataType::WeakPointer => bytes.first_chunk::<{ size_of::<[u32; 2]>() }>()?.as_slice(),
        _ => return None,
    };
    let kind = if slot.array_header.is_some() {
        RecordJsonBytes::ArrayElement
    } else {
        RecordJsonBytes::Slot
    };
    input.describe_bytes(Some(bytes), kind)
}

const fn header(index: u32, field: RecordJsonRecordField) -> RecordJsonSource<'static> {
    RecordJsonSource::Record { index, field }
}

fn record_path(input: &PositionedRecord<'_>, record: Option<Record<'_>>) -> PositionedSources {
    let mut sources = PositionedSources::new(Vec::new(), true);
    if let Some(record) = record {
        sources.push(header(record.index(), RecordJsonRecordField::FileName));
        sources.require(
            record
                .stored_file_name()
                .ok()
                .and_then(|name| input.describe_text(name)),
        );
    } else {
        sources.incomplete();
    }
    sources
}

struct Target {
    definition: PositionedSources,
    route: PositionedSources,
    path: Option<PositionedSources>,
}

impl Target {
    fn record(record: Option<Record<'_>>) -> Self {
        let mut definition = PositionedSources::new(Vec::new(), true);
        if let Some(record) = record {
            definition.push(header(record.index(), RecordJsonRecordField::Struct));
            definition.require(record.struct_type().ok().map(|structure| {
                RecordJsonSource::StructName {
                    index: structure.index(),
                }
            }));
        } else {
            definition.incomplete();
        }
        Self {
            definition,
            route: PositionedSources::new(Vec::new(), true),
            path: None,
        }
    }

    fn owned(
        input: &PositionedRecord<'_>,
        original: ReferenceOrigin<'_, '_>,
        record: Option<Record<'_>>,
    ) -> Self {
        let valid = record.is_some_and(|record| record.index() == input.tree().record_index());
        if valid && let Some(target) = owned_route(input, original) {
            return target;
        }
        Self {
            definition: PositionedSources::new(Vec::new(), false),
            route: PositionedSources::new(Vec::new(), false),
            path: Some(PositionedSources::new(Vec::new(), false)),
        }
    }
}

fn owned_route(input: &PositionedRecord<'_>, original: ReferenceOrigin<'_, '_>) -> Option<Target> {
    let steps = original.instance_location()?.steps();
    let positions = original.instance_positions()?;
    if steps.len() != positions.len() {
        return None;
    }
    let mut route = PositionedSources::new(Vec::new(), true);
    let mut path = PositionedSources::new(Vec::new(), true);
    let mut current = None;
    for (&step, &position) in steps.iter().zip(positions) {
        let site = target_step(input, current.as_ref(), step, position)?;
        match step {
            Step::Field(_) => path.append(site.field_name_sources()),
            Step::Element(_) => path.require(site.array_header_source()),
        }
        route.append(site.selection_sources());
        current = Some(site);
    }
    let Value::Struct(target) = current?.value() else {
        return None;
    };
    Some(Target {
        definition: PositionedSources::new(
            vec![RecordJsonSource::StructName {
                index: target.struct_index(),
            }],
            true,
        ),
        route,
        path: Some(path),
    })
}

fn target_step<'record, 'source>(
    input: &'record PositionedRecord<'source>,
    current: Option<&PositionedValue<'record, 'source>>,
    step: Step<'_>,
    position: u64,
) -> Option<PositionedValue<'record, 'source>> {
    match step {
        Step::Element(ordinal) if u64::from(ordinal) == position => current?.element(ordinal),
        Step::Field(name) => {
            let fields = if let Some(parent) = current {
                parent.fields().ok()??
            } else {
                input.root_fields().ok()?
            };
            let site = fields.get(usize::try_from(position).ok()?)?;
            (site.field_name() == Some(name)).then_some(site)
        }
        Step::Element(_) => None,
    }
}
