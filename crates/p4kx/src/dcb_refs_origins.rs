// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use anyhow::{Context as _, Result, ensure};
use p4kx_extraction::{
    FieldOrigins, FieldPath, IndexedReference, IndexedReferenceOrigin, InputScope,
    ObservedFileGraph, ObservedReferenceIndex, SourceInventory, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

use crate::dcb_refs::{self, Info, Link};
use crate::forge_input::ForgeInput;

#[path = "dcb_refs_count_origins.rs"]
mod counts;

pub(super) use counts::CountEvidence;

/// Per-command original Reference and header provenance.
pub(super) struct Origins<'index, 'source> {
    index: &'index ObservedReferenceIndex<'source>,
    uses: SourceUses,
    scope: SourceScope,
}

#[derive(Clone, Copy)]
pub(super) enum HeaderField {
    Guid,
    FileName,
    Name,
    SourceType,
}

impl HeaderField {
    const fn label(self) -> &'static str {
        match self {
            Self::Guid => "guid",
            Self::FileName => "fileName",
            Self::Name => "recordName",
            Self::SourceType => "sourceType",
        }
    }
}

pub(super) fn read(
    input: &ForgeInput,
    target: Option<&str>,
    write: impl FnOnce(&Origins<'_, '_>, &[u32]) -> Result<()>,
) -> Result<()> {
    let entry = input.entry();
    let forge = entry.open()?;
    let targets = dcb_refs::select(&forge, target)?;
    let index = ObservedReferenceIndex::build(&forge)?;
    dcb_refs::warn(index.index());
    let uses = SourceUses::new();
    let scope = uses.observe(input.source_input()?, InputScope::Payload)?;
    write(
        &Origins {
            index: &index,
            uses,
            scope,
        },
        &targets,
    )
}

impl<'source> Origins<'_, 'source> {
    pub(super) const fn index(&self) -> &ObservedReferenceIndex<'source> {
        self.index
    }

    pub(super) fn file_graph(&self) -> Result<ObservedFileGraph<'_, 'source>> {
        Ok(self.index.file_graph()?)
    }

    pub(super) fn inventory(&self) -> Result<SourceInventory> {
        Ok(self.uses.freeze()?)
    }

    pub(super) fn record(
        &self,
        fields: &mut FieldOrigins,
        record: &Info<'_>,
        relation: Option<&SourceUse>,
    ) -> Result<()> {
        let original = record.original();
        for (path, field, role) in [
            ("/record/guid", HeaderField::Guid, SourceUseRole::Payload),
            ("/record/record_path", HeaderField::FileName, original.path),
            ("/record/name", HeaderField::Name, original.name),
            (
                "/record/type",
                HeaderField::SourceType,
                original.source_type,
            ),
        ] {
            let header = self.header(original.index, field, role)?;
            let complete = role != SourceUseRole::Attempted;
            match relation {
                Some(relation) => {
                    self.insert(fields, path, &[header, relation.clone()], complete)?;
                }
                None => self.insert(fields, path, slice::from_ref(&header), complete)?,
            }
        }
        Ok(())
    }

    pub(super) fn link(&self, link: &Link<'_, '_>) -> Result<FieldOrigins> {
        let site = self.site(link.original())?;
        let mut fields = FieldOrigins::new();
        self.route(&mut fields, site)?;
        let relation = self.relation(site.reference())?;
        self.insert(&mut fields, "/record", slice::from_ref(&relation), true)?;
        if let Some(record) = &link.record {
            self.record(&mut fields, record, Some(&relation))?;
        }
        if site.reference().unresolved_guid().is_some() {
            let guid = self.value(site, "$error.guid", SourceUseRole::Payload)?;
            self.insert(&mut fields, "/guid", slice::from_ref(&guid), true)?;
        }
        Ok(fields)
    }

    fn route(&self, fields: &mut FieldOrigins, site: IndexedReferenceOrigin<'_>) -> Result<()> {
        let record = u64::from(site.reference().source());
        let ordinal = site.value_ordinal();
        let declarations = self.observe(
            "dataforgeReferenceRoute",
            vec![record, ordinal],
            "fieldDeclarations",
            SourceUseRole::Conversion,
        )?;
        let positions = self.observe(
            "dataforgeReferenceRoute",
            vec![record, ordinal],
            "arrayPositions",
            SourceUseRole::Conversion,
        )?;
        self.insert(fields, "/field", slice::from_ref(&declarations), true)?;
        self.insert(fields, "/indices", slice::from_ref(&positions), true)?;
        self.insert(fields, "/location", &[declarations, positions], true)?;
        for (step, _) in site.reference().indices().iter().enumerate() {
            let position = self.observe(
                "dataforgeReferenceRoute",
                vec![record, ordinal, u64::try_from(step)?],
                "arrayPosition",
                SourceUseRole::Conversion,
            )?;
            self.insert(
                fields,
                &format!("/indices/{step}"),
                slice::from_ref(&position),
                true,
            )?;
        }
        Ok(())
    }

    pub(super) fn relation(&self, reference: IndexedReference<'_>) -> Result<SourceUse> {
        let site = self.site(reference)?;
        let part = if reference.target().is_some() {
            "$value"
        } else {
            ensure!(
                reference.unresolved_guid().is_some(),
                "missing original unresolved Reference"
            );
            "$error.guid"
        };
        self.value(site, part, SourceUseRole::Selector)
    }

    pub(super) fn header(
        &self,
        index: u32,
        field: HeaderField,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        self.observe(
            "dataforgeRecordHeader",
            vec![u64::from(index)],
            field.label(),
            role,
        )
    }

    fn site(&self, reference: IndexedReference<'_>) -> Result<IndexedReferenceOrigin<'_>> {
        self.index
            .origin(reference)
            .context("Reference occurrence belongs to a different index")
    }

    fn value(
        &self,
        site: IndexedReferenceOrigin<'_>,
        part: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        self.observe(
            "dataforgeValue",
            vec![u64::from(site.reference().source()), site.value_ordinal()],
            part,
            role,
        )
    }

    fn observe(
        &self,
        namespace: &str,
        indices: Vec<u64>,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        Ok(self.scope.at(
            SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices,
                field: field.to_owned(),
            },
            role,
        )?)
    }

    pub(super) fn insert(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
        uses: &[SourceUse],
        complete: bool,
    ) -> Result<()> {
        fields.insert(FieldPath::parse(path)?, self.uses.origin(uses, complete))?;
        Ok(())
    }
}
