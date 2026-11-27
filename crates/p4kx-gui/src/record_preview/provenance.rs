// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForge, DataForgeOrigin, Field, FieldOrigin, IndexedRecordLocalization, InputScope,
    ObservedTagTree, PositionedFields, PositionedRecord, PositionedSources, PositionedValue,
    RecordJsonLocation, RecordJsonUse, SourceEvidenceError, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, TagPathOrigins, Value,
};

use super::FieldLine;
use super::xml_provenance::{PendingXml, RecordXmlFieldProvenance, RowPositions};
use crate::XmlPreviewOriginError;

#[path = "line_provenance.rs"]
mod line_provenance;

pub use line_provenance::{RecordFieldProvenance, RecordReferenceProvenance};

/// Exact original contributions to each piece of the Record preview heading.
#[derive(Debug)]
#[non_exhaustive]
pub struct RecordHeaderProvenance {
    /// Physical Record name.
    pub name: FieldOrigin,
    /// Effective root struct name.
    pub type_name: FieldOrigin,
    /// Stored Record GUID.
    pub guid: FieldOrigin,
    /// Displayed Record path.
    pub record_path: FieldOrigin,
    /// Stored Record tag when present.
    pub tag: Option<FieldOrigin>,
}

/// Original source evidence for the structured Record preview.
#[derive(Debug)]
pub struct RecordPreviewProvenance {
    header: RecordHeaderProvenance,
    lines: Vec<RecordFieldProvenance>,
    inventory: SourceInventory,
    xml: Option<Result<Vec<RecordXmlFieldProvenance>, XmlPreviewOriginError>>,
}

impl RecordPreviewProvenance {
    /// Borrows the field origins of the Record header.
    #[must_use]
    pub const fn header(&self) -> &RecordHeaderProvenance {
        &self.header
    }

    /// Returns origins in the same depth-first order as the preview lines.
    #[must_use]
    pub fn lines(&self) -> &[RecordFieldProvenance] {
        &self.lines
    }

    /// Returns the physical sources and original use coordinates.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    /// Returns the exact emitted fields in the retained Record XML.
    ///
    /// Returns an evidence error separately from a Record XML serialization failure.
    #[must_use]
    pub fn xml_fields(
        &self,
    ) -> Option<Result<&[RecordXmlFieldProvenance], &XmlPreviewOriginError>> {
        self.xml.as_ref().map(Result::as_deref)
    }

    pub(super) fn capture(
        forge: &DataForge<'_>,
        record: &PositionedRecord<'_>,
        input: Option<&DataForgeOrigin>,
        locale: Option<&IndexedRecordLocalization>,
        tags: Option<&ObservedTagTree>,
        shown: &[FieldLine],
        xml: Option<(PendingXml, u64)>,
    ) -> Result<Self, SourceEvidenceError> {
        let binding = Binding::new(input)?;
        let header = binding.header(forge, record)?;
        let route = binding.root_route(record.tree().record_index())?;
        let tags = tags
            .map(|tags| tags.bind_paths(forge, &binding.uses))
            .transpose()?;
        let mut capture = Capture {
            binding,
            locale,
            tags,
            lines: Vec::with_capacity(shown.len()),
            shown: shown.iter(),
            positions: xml.as_ref().map(|_| RowPositions::reserve(shown.len())),
        };
        capture.fields(
            record.tree().fields(),
            record.root_fields().ok().as_ref(),
            &route,
        )?;
        if capture.shown.next().is_some() {
            return Err(SourceEvidenceError::ForeignEvidence);
        }
        let xml = xml
            .zip(capture.positions)
            .map(|((pending, retained_end), positions)| {
                let unknown = capture.binding.uses.origin(&[], false);
                pending.bind(retained_end, positions, &header, &capture.lines, &unknown)
            });
        Ok(Self {
            header,
            lines: capture.lines,
            inventory: capture.binding.uses.freeze()?,
            xml,
        })
    }
}

struct Binding {
    uses: SourceUses,
    scope: SourceScope,
    complete: bool,
}

impl Binding {
    fn new(input: Option<&DataForgeOrigin>) -> Result<Self, SourceEvidenceError> {
        let (source, complete) = match input {
            Some(DataForgeOrigin::Archive(entry)) => (
                SourceInput::Archive(entry),
                entry.ancestors().all(|step| step.is_some()),
            ),
            Some(DataForgeOrigin::File(file)) => (SourceInput::File(file), true),
            _ => (SourceInput::Unknown, false),
        };
        let uses = SourceUses::with_compact_use_refs();
        let scope = uses.observe(source, InputScope::Payload)?;
        Ok(Self {
            uses,
            scope,
            complete,
        })
    }

    fn header(
        &self,
        forge: &DataForge<'_>,
        positioned: &PositionedRecord<'_>,
    ) -> Result<RecordHeaderProvenance, SourceEvidenceError> {
        let tree = positioned.tree();
        let record = forge.record(tree.record_index());
        if !record.as_ref().is_some_and(|record| tree.is_from(record)) {
            return Err(SourceEvidenceError::ForeignEvidence);
        }
        let [(_, guid), (_, record_path), (_, name), _] = RecordJsonLocation::identity(
            forge,
            record.as_ref(),
            &self.uses,
            &self.scope,
            self.complete,
        )?;
        let mut kind = self.root_route(tree.record_index())?;
        kind.push(self.site(
            SourceLocation::Subresource {
                namespace: "dataforgeStructDefinition".into(),
                indices: vec![u64::from(tree.root().struct_index())],
                field: "name".into(),
            },
            SourceUseRole::Payload,
        )?);
        match RecordJsonLocation::borrowed_text(forge, tree.type_name()) {
            Some(text) => kind.push(self.site(text, SourceUseRole::Payload)?),
            None => kind.complete = false,
        }
        Ok(RecordHeaderProvenance {
            name,
            type_name: kind.origin(&self.uses),
            guid,
            record_path,
            tag: tree
                .tag()
                .map(|tag| self.header_tag(forge, tree.record_index(), tag))
                .transpose()?,
        })
    }

    fn header_tag(
        &self,
        forge: &DataForge<'_>,
        index: u32,
        tag: &str,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut route = Route {
            inputs: vec![self.record_field(index, "tag", SourceUseRole::Conversion)?],
            complete: self.complete,
        };
        if let Some(text) = RecordJsonLocation::borrowed_text(forge, tag) {
            route.push(self.site(text, SourceUseRole::Payload)?);
        } else {
            route.complete = false;
        }
        Ok(route.origin(&self.uses))
    }

    fn root_route(&self, record: u32) -> Result<Route, SourceEvidenceError> {
        let mut route = Route {
            inputs: Vec::new(),
            complete: self.complete,
        };
        for field in ["structIndex", "instanceIndex"] {
            route.push(self.record_field(record, field, SourceUseRole::Selector)?);
        }
        Ok(route)
    }

    fn record_field(
        &self,
        record: u32,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.site(
            SourceLocation::DataForge {
                record_index: record,
                field: field.into(),
            },
            role,
        )
    }

    fn site(
        &self,
        location: SourceLocation,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.scope.at(location, role)
    }

    fn positioned(
        &self,
        route: &mut Route,
        sources: Option<&PositionedSources>,
    ) -> Result<(), SourceEvidenceError> {
        let Some(sources) = sources else {
            route.complete = false;
            return Ok(());
        };
        route.complete &= sources.complete();
        for source in sources.sources() {
            self.append(route, RecordJsonLocation::of(*source))?;
        }
        Ok(())
    }

    fn referenced(
        &self,
        route: &mut Route,
        sources: &[RecordJsonUse],
        complete: bool,
    ) -> Result<(), SourceEvidenceError> {
        route.complete &= complete;
        for source in sources {
            self.append(route, RecordJsonLocation::of_use(source))?;
        }
        Ok(())
    }

    fn append(
        &self,
        route: &mut Route,
        site: Option<RecordJsonLocation>,
    ) -> Result<(), SourceEvidenceError> {
        match site {
            Some(site) => route.push(self.site(site.location, site.role)?),
            None => route.complete = false,
        }
        Ok(())
    }
}

#[derive(Clone)]
struct Route {
    inputs: Vec<SourceUse>,
    complete: bool,
}

impl Route {
    fn push(&mut self, used: SourceUse) {
        self.inputs.push(used);
    }

    fn origin(&self, uses: &SourceUses) -> FieldOrigin {
        uses.origin(&self.inputs, self.complete)
    }
}

struct Capture<'a> {
    binding: Binding,
    locale: Option<&'a IndexedRecordLocalization>,
    tags: Option<TagPathOrigins<'a>>,
    lines: Vec<RecordFieldProvenance>,
    shown: std::slice::Iter<'a, FieldLine>,
    positions: Option<RowPositions>,
}

impl Capture<'_> {
    fn fields(
        &mut self,
        fields: &[Field<'_>],
        positioned: Option<&PositionedFields<'_, '_>>,
        parent: &Route,
    ) -> Result<(), SourceEvidenceError> {
        for (ordinal, field) in fields.iter().enumerate() {
            let at = positioned.and_then(|fields| fields.get(ordinal));
            self.field(field.value(), at, parent)?;
        }
        Ok(())
    }

    fn field(
        &mut self,
        value: &Value<'_>,
        at: Option<PositionedValue<'_, '_>>,
        parent: &Route,
    ) -> Result<(), SourceEvidenceError> {
        let shown = self
            .shown
            .next()
            .ok_or(SourceEvidenceError::ForeignEvidence)?;
        if let Some(positions) = &mut self.positions {
            positions.record(value, self.lines.len());
        }
        let line = self.line(parent, at, shown.value())?;
        self.lines.push(line);
        match value {
            Value::Array(items) => {
                let route = self.child_route(parent, at)?;
                for (item, ordinal) in items.iter().zip(0_u32..) {
                    self.field(item, at.and_then(|array| array.element(ordinal)), &route)?;
                }
            }
            Value::Struct(value) => {
                let route = self.child_route(parent, at)?;
                let fields = at.and_then(|value| value.fields().ok().flatten());
                self.fields(value.fields(), fields.as_ref(), &route)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn child_route(
        &self,
        parent: &Route,
        slot: Option<PositionedValue<'_, '_>>,
    ) -> Result<Route, SourceEvidenceError> {
        let mut route = parent.clone();
        self.binding.positioned(
            &mut route,
            slot.and_then(|slot| slot.selection_sources()).as_ref(),
        )?;
        Ok(route)
    }
}
