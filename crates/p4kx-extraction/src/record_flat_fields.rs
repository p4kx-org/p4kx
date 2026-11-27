// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_dataforge::{
    Annotations, DataForge, DataForgeHeaderField, FlatColumn, FlatColumnSpan, FlatFormat,
    FlatRowObservation, FlatValues, PositionedRecord, PositionedReferenceSources,
    PositionedSources, PositionedValue, RecordJsonBytes, RecordJsonRecordField,
    RecordJsonReferenceMember, RecordJsonReferenceSources, RecordJsonSource, RecordText,
    Resolution, TextResolver, Value, ValueError,
};
use p4kx_localization::is_placeholder_key;

use super::stream_fields::{StreamAddress, StreamField, StreamLookup};
use crate::{FieldPath, RecordJsonLocation, RecordLocalization, SourceUseRole};

pub(super) fn write_flat<'locale>(
    forge: &DataForge<'_>,
    writer: &mut FlatValues,
    record: &PositionedRecord<'_>,
    out: &mut Vec<u8>,
    locale: Option<&RecordLocalization<'locale>>,
) -> io::Result<(u64, Vec<StreamField<'locale>>)> {
    let index = record.tree().record_index();
    let header = Headers::new(forge, record);
    let offset = u64::try_from(out.len()).map_err(io::Error::other)?;
    let format = writer.format();
    let annotations =
        Annotations::none().with_texts(locale.map(|locale| locale as &dyn TextResolver));
    let mut output = Vec::new();
    let mut rows = 0;
    writer.write_positioned_observed(record, annotations, out, |row| {
        let positioned = RowSources::new(index, row.route())?;
        output
            .try_reserve(row.columns().len())
            .map_err(io::Error::other)?;
        for span in row.columns() {
            let column = span.column();
            let (proof, lookup) =
                column_sources(column, &header, &positioned, &row, forge, locale)?;
            output.push(StreamField {
                address: address(format, row.row_index(), span, offset)?,
                sites: proof.sites,
                complete: proof.complete,
                lookup,
            });
        }
        rows = row
            .row_index()
            .checked_add(1)
            .ok_or_else(|| io::Error::other("flat Record row count exceeds u64"))?;
        Ok(())
    })?;
    Ok((rows, output))
}

fn address(
    format: FlatFormat,
    row: u64,
    span: &FlatColumnSpan,
    offset: u64,
) -> io::Result<StreamAddress> {
    let column = span.column().name();
    if format == FlatFormat::Ndjson {
        let mut path = FieldPath::root();
        path.push_field(column).map_err(io::Error::other)?;
        return Ok(StreamAddress::Json { row, path });
    }
    let bytes = span.bytes();
    let absolute = |within| {
        offset
            .checked_add(within)
            .ok_or_else(|| io::Error::other("flat Part byte offset exceeds u64"))
    };
    Ok(StreamAddress::Text {
        row,
        column,
        start: absolute(bytes.start)?,
        end: absolute(bytes.end)?,
    })
}

fn column_sources<'locale>(
    column: FlatColumn,
    header: &Headers,
    positioned: &RowSources<'_, '_>,
    row: &FlatRowObservation<'_, '_, '_, '_>,
    forge: &DataForge<'_>,
    locale: Option<&RecordLocalization<'locale>>,
) -> io::Result<(Proof, Option<StreamLookup<'locale>>)> {
    let proof = match column {
        FlatColumn::RecordGuid => header.guid.clone(),
        FlatColumn::RecordName => header.name.clone(),
        FlatColumn::RecordPath => header.path.clone(),
        FlatColumn::RecordTag => header.tag.clone(),
        FlatColumn::FieldPath => positioned.field_path(row)?,
        FlatColumn::ArrayIndices => positioned.array_indices(row)?,
        FlatColumn::Kind => positioned.kind()?,
        FlatColumn::Value => positioned.value()?,
        FlatColumn::TargetPath => positioned.target_path()?,
        FlatColumn::Text => return text_sources(positioned, row, forge, locale),
        _ => Proof::single(None),
    };
    Ok((proof, None))
}

fn text_sources<'locale>(
    positioned: &RowSources<'_, '_>,
    row: &FlatRowObservation<'_, '_, '_, '_>,
    forge: &DataForge<'_>,
    locale: Option<&RecordLocalization<'locale>>,
) -> io::Result<(Proof, Option<StreamLookup<'locale>>)> {
    let Value::Locale(key) = positioned.leaf.value() else {
        return Ok((positioned.kind()?, None));
    };
    let mut proof = positioned.parents.clone();
    proof.positions(Some(positioned.leaf.value_sources()), selector)?;
    if is_placeholder_key(key)
        && row
            .localization()
            .is_some_and(|observed| matches!(observed.resolution(), Resolution::Keep))
    {
        return Ok((proof, None));
    }
    let authored_key = borrowed_text(forge, key, SourceUseRole::Selector);
    let evidence = locale
        .zip(row.localization())
        .and_then(|(locale, observed)| locale.capture_lookup(observed));
    proof.complete &= authored_key.is_some() && evidence.is_some();
    Ok((
        proof,
        Some(StreamLookup {
            authored_key,
            evidence,
        }),
    ))
}

struct Headers {
    guid: Proof,
    name: Proof,
    path: Proof,
    tag: Proof,
}

impl Headers {
    fn new(forge: &DataForge<'_>, record: &PositionedRecord<'_>) -> Self {
        let tree = record.tree();
        let index = tree.record_index();
        let metadata = tree.metadata();
        let mut path = Proof::single(header(index, RecordJsonRecordField::FileName));
        path.push(borrowed_text(
            forge,
            metadata.file_name(),
            SourceUseRole::Payload,
        ));
        let name = Self::declared(forge, index, RecordJsonRecordField::Name, metadata.name());
        let tag = match metadata.metadata_tag() {
            RecordText::Absent => Proof::single(version()),
            stored => Self::declared(forge, index, RecordJsonRecordField::Tag, stored),
        };
        Self {
            guid: Proof::single(header(index, RecordJsonRecordField::Guid)),
            name,
            path,
            tag,
        }
    }

    fn declared(
        forge: &DataForge<'_>,
        index: u32,
        field: RecordJsonRecordField,
        stored: RecordText<'_>,
    ) -> Proof {
        let mut proof = Proof::single(header(index, field));
        match stored {
            RecordText::Present(value) => {
                proof.push(borrowed_text(forge, value, SourceUseRole::Payload));
            }
            RecordText::Null => {}
            _ => proof.complete = false,
        }
        proof
    }
}

fn header(index: u32, field: RecordJsonRecordField) -> Option<RecordJsonLocation> {
    RecordJsonLocation::of(RecordJsonSource::Record { index, field })
}

fn version() -> Option<RecordJsonLocation> {
    let bytes = DataForgeHeaderField::Version.byte_span();
    selector(RecordJsonSource::Bytes {
        start: bytes.start,
        end: bytes.end,
        kind: RecordJsonBytes::Slot,
    })
}

fn borrowed_text(
    forge: &DataForge<'_>,
    text: &str,
    role: SourceUseRole,
) -> Option<RecordJsonLocation> {
    Some(RecordJsonLocation {
        location: RecordJsonLocation::borrowed_text(forge, text)?,
        role,
    })
}

struct RowSources<'record, 'source> {
    leaf: PositionedValue<'record, 'source>,
    parents: Proof,
    reference: Option<PositionedReferenceSources>,
}

impl<'record, 'source> RowSources<'record, 'source> {
    fn new(index: u32, route: &[PositionedValue<'record, 'source>]) -> io::Result<Self> {
        let (&leaf, parents) = route
            .split_last()
            .ok_or_else(|| io::Error::other("flat value has no original site"))?;
        let mut selected = Proof::single(header(index, RecordJsonRecordField::Struct));
        for parent in parents {
            selected.positions(parent.selection_sources(), selector)?;
        }
        Ok(Self {
            leaf,
            parents: selected,
            reference: leaf.reference_sources(),
        })
    }

    fn field_path(&self, row: &FlatRowObservation<'_, '_, '_, '_>) -> io::Result<Proof> {
        let mut proof = self.parents.clone();
        for field in row.route().iter().chain(row.field_path_aliases()) {
            if field.field_name().is_some() {
                proof.positions(field.field_name_sources(), selector)?;
            }
        }
        Ok(proof)
    }

    fn array_indices(&self, row: &FlatRowObservation<'_, '_, '_, '_>) -> io::Result<Proof> {
        let mut proof = self.parents.clone();
        if !row.array_indices().is_empty() {
            proof.positions(Some(self.leaf.storage_sources()), selector)?;
        }
        Ok(proof)
    }

    fn value(&self) -> io::Result<Proof> {
        let mut proof = self.parents.clone();
        match self.leaf.value() {
            Value::Reference(_) => {
                proof.reference(self.member(RecordJsonReferenceMember::Guid), keep_role)?;
            }
            Value::Error(ValueError::DanglingReference { .. }) => {
                proof.reference(self.leaf.dangling_reference_guid_sources(), keep_role)?;
            }
            _ => proof.positions(Some(self.leaf.value_sources()), RecordJsonLocation::of)?,
        }
        Ok(proof)
    }

    fn kind(&self) -> io::Result<Proof> {
        let mut proof = self.parents.clone();
        match self.leaf.value() {
            Value::Reference(_) => proof.reference(
                self.reference
                    .as_ref()
                    .map(PositionedReferenceSources::kind),
                keep_role,
            )?,
            Value::Error(ValueError::DanglingReference { .. }) => {
                proof.reference(self.leaf.dangling_reference_guid_sources(), select_payload)?;
            }
            _ => proof.positions(Some(self.leaf.storage_sources()), selector)?,
        }
        Ok(proof)
    }

    fn target_path(&self) -> io::Result<Proof> {
        let mut proof = self.parents.clone();
        match self.leaf.value() {
            Value::Reference(_) => {
                proof.reference(self.member(RecordJsonReferenceMember::Path), keep_role)?;
            }
            Value::Error(ValueError::DanglingReference { .. }) => {
                proof.reference(self.leaf.dangling_reference_error_sources(), keep_role)?;
            }
            _ => proof.positions(Some(self.leaf.storage_sources()), selector)?,
        }
        Ok(proof)
    }

    fn member(&self, member: RecordJsonReferenceMember) -> Option<RecordJsonReferenceSources> {
        self.reference
            .as_ref()
            .map(|reference| reference.member(member))
    }
}

#[derive(Clone)]
struct Proof {
    sites: Vec<Option<RecordJsonLocation>>,
    complete: bool,
}

impl Proof {
    fn single(site: Option<RecordJsonLocation>) -> Self {
        Self {
            complete: site.is_some(),
            sites: vec![site],
        }
    }

    fn push(&mut self, site: Option<RecordJsonLocation>) {
        self.complete &= site.is_some();
        self.sites.push(site);
    }

    fn positions(
        &mut self,
        positions: Option<PositionedSources>,
        locate: impl FnMut(RecordJsonSource<'static>) -> Option<RecordJsonLocation>,
    ) -> io::Result<()> {
        let Some(positions) = positions else {
            self.complete = false;
            return Ok(());
        };
        self.complete &= positions.complete();
        self.append(positions.sources().iter().copied().map(locate))
    }

    fn reference(
        &mut self,
        reference: Option<RecordJsonReferenceSources>,
        roles: impl Fn(RecordJsonLocation) -> RecordJsonLocation,
    ) -> io::Result<()> {
        let Some(reference) = reference else {
            self.complete = false;
            return Ok(());
        };
        self.complete &= reference.complete();
        self.append(
            reference
                .sources()
                .iter()
                .map(|used| RecordJsonLocation::of_use(used).map(&roles)),
        )
    }

    fn append(
        &mut self,
        sites: impl ExactSizeIterator<Item = Option<RecordJsonLocation>>,
    ) -> io::Result<()> {
        self.sites
            .try_reserve(sites.len())
            .map_err(io::Error::other)?;
        for site in sites {
            self.push(site);
        }
        Ok(())
    }
}

fn selector(source: RecordJsonSource<'static>) -> Option<RecordJsonLocation> {
    let mut location = RecordJsonLocation::of(source)?;
    if !matches!(source, RecordJsonSource::PropertyStorage { .. }) {
        location.role = SourceUseRole::Selector;
    }
    Some(location)
}

const fn select_payload(mut location: RecordJsonLocation) -> RecordJsonLocation {
    if matches!(location.role, SourceUseRole::Payload) {
        location.role = SourceUseRole::Selector;
    }
    location
}

const fn keep_role(location: RecordJsonLocation) -> RecordJsonLocation {
    location
}

#[cfg(test)]
#[path = "record_flat_fields_tests.rs"]
mod tests;
