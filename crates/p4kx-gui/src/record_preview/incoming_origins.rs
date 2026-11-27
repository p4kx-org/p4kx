// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForge, DataForgeOrigin, FieldOrigin, IndexedReferenceOrigin, InputScope, Record,
    RecordJsonLocation, ReferenceIndex, SourceEvidenceError, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};

use crate::{RecordFieldProvenance, RecordReferenceProvenance};

#[derive(Debug)]
pub(super) struct Evidence {
    pub(super) inventory: SourceInventory,
    pub(super) rows: Vec<RecordFieldProvenance>,
    pub(super) count: FieldOrigin,
}

pub(super) struct Capture {
    uses: SourceUses,
    scope: SourceScope,
    rows: Vec<RecordFieldProvenance>,
    complete: bool,
}

impl Capture {
    pub(super) fn new(input: Option<&DataForgeOrigin>) -> Result<Self, SourceEvidenceError> {
        let (input, complete) = match input {
            Some(DataForgeOrigin::Archive(entry)) => (
                SourceInput::Archive(entry),
                entry.ancestors().all(|step| step.is_some()),
            ),
            Some(DataForgeOrigin::File(file)) => (SourceInput::File(file), true),
            _ => (SourceInput::Unknown, false),
        };
        let uses = SourceUses::with_compact_use_refs();
        let scope = uses.observe(input, InputScope::Payload)?;
        Ok(Self {
            uses,
            scope,
            rows: Vec::new(),
            complete,
        })
    }

    pub(super) fn row(
        &mut self,
        forge: &DataForge<'_>,
        reference: Option<IndexedReferenceOrigin<'_>>,
        record: &Record<'_>,
        path: Option<&str>,
        type_name: Option<&str>,
    ) -> Result<(), SourceEvidenceError> {
        let index = record.index();
        let name = self.route(reference)?;
        let relation = reference.map(|site| self.relation(site)).transpose()?;
        let path = self.header(forge, index, Header::Path(path), relation.as_ref())?;
        let type_name = self.header(forge, index, Header::Type(type_name), relation.as_ref())?;
        let guid = self.header(forge, index, Header::Guid, relation.as_ref())?;
        self.rows.push(RecordFieldProvenance {
            name,
            value: path.clone(),
            annotation: Some(type_name.clone()),
            reference: Some(RecordReferenceProvenance {
                guid,
                path,
                type_name,
                instance_path: None,
            }),
            dangling_guid: None,
        });
        Ok(())
    }

    pub(super) fn finish(
        self,
        forge: &DataForge<'_>,
        references: &ReferenceIndex,
        target: u32,
    ) -> Result<Evidence, SourceEvidenceError> {
        let count = self.count(forge, references, target)?;
        Ok(Evidence {
            inventory: self.uses.freeze()?,
            rows: self.rows,
            count,
        })
    }

    fn route(
        &self,
        origin: Option<IndexedReferenceOrigin<'_>>,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let Some(origin) = origin else {
            return Ok(self.uses.origin(&[], false));
        };
        let route = ordinal(origin);
        let mut inputs = vec![self.observe(
            "dataforgeReferenceRoute",
            route.clone(),
            "fieldDeclarations",
            SourceUseRole::Conversion,
        )?];
        if !origin.reference().indices().is_empty() {
            inputs.push(self.observe(
                "dataforgeReferenceRoute",
                route,
                "arrayPositions",
                SourceUseRole::Conversion,
            )?);
        }
        Ok(self.uses.origin(&inputs, self.complete))
    }

    fn relation(
        &self,
        origin: IndexedReferenceOrigin<'_>,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.observe(
            "dataforgeValue",
            ordinal(origin),
            "$value",
            SourceUseRole::Selector,
        )
    }

    fn header(
        &self,
        forge: &DataForge<'_>,
        record: u32,
        header: Header<'_>,
        relation: Option<&SourceUse>,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut inputs: Vec<_> = relation.into_iter().cloned().collect();
        let (field, role, text) = match header {
            Header::Guid => ("guid", SourceUseRole::Payload, None),
            Header::Path(text) => ("fileName", SourceUseRole::Conversion, text),
            Header::Type(text) => ("sourceType", SourceUseRole::Selector, text),
        };
        let located = text.and_then(|text| RecordJsonLocation::borrowed_text(forge, text));
        let complete = self.complete
            && relation.is_some()
            && (matches!(header, Header::Guid) || located.is_some());
        let role = if matches!(header, Header::Guid) || text.is_some() {
            role
        } else {
            SourceUseRole::Attempted
        };
        let physical = self.scope.at(
            SourceLocation::DataForge {
                record_index: record,
                field: field.into(),
            },
            role,
        )?;
        if role != SourceUseRole::Attempted {
            inputs.push(physical);
        }
        if let Some(location) = located {
            inputs.push(self.scope.at(location, SourceUseRole::Payload)?);
        }
        Ok(self.uses.origin(&inputs, complete))
    }

    fn count(
        &self,
        forge: &DataForge<'_>,
        references: &ReferenceIndex,
        target: u32,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let Some(gaps) = references.scan_gaps() else {
            return Ok(self.uses.origin(&[], false));
        };
        let mut inputs = vec![self.observe(
            "dataforgeHeader",
            Vec::new(),
            "recordCount",
            SourceUseRole::Selector,
        )?];
        let mut next = 0;
        for gap in gaps {
            let record = u64::from(gap.record_index());
            self.scan_range(&mut inputs, next, record)?;
            next = record + 1;
        }
        self.scan_range(&mut inputs, next, forge.record_count() as u64)?;
        if !gaps.is_empty() {
            for occurrence in references.incoming(target) {
                if gaps
                    .binary_search_by_key(&occurrence.source(), |gap| gap.record_index())
                    .is_ok()
                    && let Some(origin) = references.origin(occurrence)
                {
                    inputs.push(self.relation(origin)?);
                }
            }
        }
        let complete = self.complete && gaps.is_empty() && !references.is_truncated();
        Ok(self.uses.origin(&inputs, complete))
    }

    fn scan_range(
        &self,
        uses: &mut Vec<SourceUse>,
        start: u64,
        end: u64,
    ) -> Result<(), SourceEvidenceError> {
        if start < end {
            uses.push(self.observe(
                "dataforgeRecordBodyRange",
                vec![start, end],
                "referenceMembership",
                SourceUseRole::Selector,
            )?);
        }
        Ok(())
    }

    fn observe(
        &self,
        namespace: &str,
        indices: Vec<u64>,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.scope.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices,
                field: field.into(),
            },
            role,
        )
    }
}

#[derive(Clone, Copy)]
enum Header<'a> {
    Guid,
    Path(Option<&'a str>),
    Type(Option<&'a str>),
}

fn ordinal(origin: IndexedReferenceOrigin<'_>) -> Vec<u64> {
    vec![
        u64::from(origin.reference().source()),
        origin.value_ordinal(),
    ]
}
