// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{
    DataForge, DataForgeOrigin, RecordJsonBytes, RecordJsonObserver, RecordJsonRecordField,
    RecordJsonSource, SourceLocation, SourceUse, SourceUseRole,
};
use serde_json::value::RawValue;

use crate::budget::json_len;
use crate::forge_origins::{EvidenceError, ForgeOrigins, site};
use crate::forge_shapes::ForgeFieldOrigins;
use crate::records::Selector;

pub(crate) struct CapturedRecord {
    pub(crate) index: u32,
    pub(crate) value: Box<RawValue>,
    pub(crate) capture: Capture,
}

pub(crate) struct Capture {
    limit: usize,
    minimum_wire_len: usize,
    fields: Vec<CapturedField>,
}

struct CapturedField {
    pointer: String,
    sources: Vec<Locator>,
    complete: bool,
}

struct Locator {
    location: SourceLocation,
    role: SourceUseRole,
}

impl Capture {
    pub(crate) const fn new(limit: usize) -> Self {
        Self {
            limit,
            minimum_wire_len: 0,
            fields: Vec::new(),
        }
    }

    pub(crate) const fn minimum_wire_len(&self) -> usize {
        self.minimum_wire_len
    }
}

impl RecordJsonObserver for Capture {
    fn value(
        &mut self,
        pointer: &str,
        sources: &[RecordJsonSource<'_>],
        complete: bool,
    ) -> io::Result<()> {
        // Each address owns a distinct JSON member, even when its source sites deduplicate.
        let minimum = json_len(&pointer).saturating_add(3);
        if self.minimum_wire_len.saturating_add(minimum) > self.limit {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "the response cap is reached",
            ));
        }
        self.minimum_wire_len += minimum;
        let locations: Vec<_> = sources
            .iter()
            .filter_map(|source| Locator::of(*source))
            .collect();
        self.fields.push(CapturedField {
            pointer: pointer.to_owned(),
            complete: complete && locations.len() == sources.len(),
            sources: locations,
        });
        Ok(())
    }
}

pub(crate) struct Navigation {
    record_count: usize,
    selector: &'static str,
    complete: bool,
}

impl Navigation {
    pub(crate) fn new(forge: &DataForge<'_>, selector: &Selector) -> Self {
        let record_count = forge.record_count();
        let (selector, complete) = match selector {
            Selector::Guid(_) => ("guidWords", true),
            Selector::Path(_) => ("fileNameOffsets", forge.path_order().len() == record_count),
        };
        Self {
            record_count,
            selector,
            complete,
        }
    }

    fn write(
        &self,
        origins: &mut ForgeOrigins,
        cursor: Option<&[u32]>,
    ) -> Result<(), EvidenceError> {
        let scope = &origins.scope;
        let count = site(
            scope,
            "dataforgeRecordTable",
            "definitionCount",
            Vec::new(),
            SourceUseRole::Selector,
        )?;
        let selector = site(
            scope,
            "dataforgeRecordSurvey",
            self.selector,
            vec![0, self.record_count as u64],
            SourceUseRole::Selector,
        )?;
        let mut sources = vec![count, selector];
        origins.put(
            "total",
            origins
                .uses
                .origin(&sources, origins.complete && self.complete),
        )?;
        if let Some(examined) = cursor {
            if !examined.is_empty() {
                sources.push(site(
                    &origins.scope,
                    "dataforgeRecordSurvey",
                    "decodedRecordSlots",
                    examined.iter().copied().map(u64::from).collect(),
                    SourceUseRole::Selector,
                )?);
            }
            origins.put(
                "next_cursor",
                origins
                    .uses
                    .origin(&sources, origins.complete && self.complete),
            )?;
        }
        Ok(())
    }
}

pub(crate) fn project(
    origin: &DataForgeOrigin,
    navigation: &Navigation,
    records: &[CapturedRecord],
    cursor: Option<&[u32]>,
) -> Result<ForgeFieldOrigins, EvidenceError> {
    let mut origins = ForgeOrigins::new(origin)?;
    origins.put("archive", origins.uses.origin(&[], true))?;
    navigation.write(&mut origins, cursor)?;
    let mut page = Vec::new();
    let mut complete = true;
    for (position, record) in records.iter().enumerate() {
        let (row, row_complete) = project_record(&mut origins, record, position)?;
        page.push(row);
        complete &= row_complete;
    }
    origins.put("records", origins.uses.origin(&page, complete))?;
    origins.finish()
}

fn project_record(
    origins: &mut ForgeOrigins,
    record: &CapturedRecord,
    position: usize,
) -> Result<(SourceUse, bool), EvidenceError> {
    let row = format!("records/{position}");
    for field in &record.capture.fields {
        let uses = field
            .sources
            .iter()
            .map(|site| origins.scope.at(site.location.clone(), site.role))
            .collect::<Result<Vec<_>, _>>()?;
        origins.put(
            &format!("{row}{}", field.pointer),
            origins
                .uses
                .origin(&uses, origins.complete && field.complete),
        )?;
    }
    let record_input = site(
        &origins.scope,
        "dataforgeRecordTable",
        "canonicalRecord",
        vec![u64::from(record.index)],
        SourceUseRole::Selector,
    )?;
    let complete = origins.complete && record.capture.fields.iter().all(|field| field.complete);
    origins.put(
        &row,
        origins
            .uses
            .origin(std::slice::from_ref(&record_input), complete),
    )?;
    Ok((record_input, complete))
}

impl Locator {
    fn of(source: RecordJsonSource<'_>) -> Option<Self> {
        match source {
            RecordJsonSource::Record { index, field } => Self::record(index, field),
            RecordJsonSource::StructName { index } => Some(Self::subresource(
                "dataforgeStructDefinition",
                "name",
                vec![u64::from(index)],
                SourceUseRole::Payload,
            )),
            RecordJsonSource::PropertyName { index } => Some(Self::subresource(
                "dataforgePropertyDefinition",
                "name",
                vec![index as u64],
                SourceUseRole::Selector,
            )),
            RecordJsonSource::PropertyStorage { index, target_used } => Some(Self::subresource(
                "dataforgePropertyDefinition",
                if target_used {
                    "targetedInstanceLayout"
                } else {
                    "instanceLayout"
                },
                vec![index as u64],
                SourceUseRole::Conversion,
            )),
            RecordJsonSource::InstancePath { record, ordinals } => {
                let mut indices = vec![u64::from(record)];
                indices.extend_from_slice(ordinals);
                Some(Self::subresource(
                    "dataforgeOwnedInstance",
                    "path",
                    indices,
                    SourceUseRole::Selector,
                ))
            }
            RecordJsonSource::Bytes { start, end, kind } => Self::bytes(start, end, kind),
            _ => None,
        }
    }

    fn record(index: u32, field: RecordJsonRecordField) -> Option<Self> {
        let (field, role) = match field {
            RecordJsonRecordField::Guid => ("guid", SourceUseRole::Payload),
            RecordJsonRecordField::FileName => ("fileName", SourceUseRole::Conversion),
            RecordJsonRecordField::Name => ("recordName", SourceUseRole::Conversion),
            RecordJsonRecordField::Tag => ("tag", SourceUseRole::Conversion),
            RecordJsonRecordField::Struct => ("sourceType", SourceUseRole::Selector),
            _ => return None,
        };
        Some(Self {
            location: SourceLocation::DataForge {
                record_index: index,
                field: field.into(),
            },
            role,
        })
    }

    fn bytes(start: u64, end: u64, kind: RecordJsonBytes) -> Option<Self> {
        let (label, role) = match kind {
            RecordJsonBytes::Slot => ("slot", SourceUseRole::Payload),
            RecordJsonBytes::ArrayHeader => ("arrayHeader", SourceUseRole::Selector),
            RecordJsonBytes::ArrayElement => ("arrayElement", SourceUseRole::Payload),
            RecordJsonBytes::Text => ("text", SourceUseRole::Payload),
            _ => return None,
        };
        Some(Self::subresource(
            "dataforgeDecodedBytes",
            label,
            vec![start, end],
            role,
        ))
    }

    fn subresource(namespace: &str, field: &str, indices: Vec<u64>, role: SourceUseRole) -> Self {
        Self {
            location: SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices,
                field: field.to_owned(),
            },
            role,
        }
    }
}
