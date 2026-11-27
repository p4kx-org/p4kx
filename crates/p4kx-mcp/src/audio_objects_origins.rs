// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::hirc_view::ObjectView;
use p4kx_extraction::{
    Bank, FieldOrigin, FieldOrigins, HircKind, HircObject, HircValue, InputScope, SectionId,
    SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses,
};

use crate::audio_wem::EvidenceError;
use crate::audio_wem::payload_origins::{metadata, put, range_use};

const ROOT: &str = "/structuredContent";
const WORD_BYTES: u64 = size_of::<u32>() as u64;
const OBJECT: &str = "bnkHircObject";
const EVENT: &str = "bnkHircEvent";

type Result<T> = std::result::Result<T, EvidenceError>;

#[derive(Clone, Copy)]
pub(super) struct Listing {
    pub(super) filter_kind: Option<HircKind>,
    pub(super) filter_id: bool,
    pub(super) cursor: bool,
    pub(super) limited_by_bytes: bool,
}

struct Evidence {
    uses: SourceUses,
    bank: SourceScope,
    version: Range<u64>,
    complete: bool,
}

pub(super) fn capture(
    bank: &Bank,
    rows: &[ObjectView<'_>],
    listing: Listing,
) -> Result<(FieldOrigins, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let origin = bank.archive_origin();
    let scope = uses.observe(
        origin.map_or(SourceInput::Unknown, SourceInput::Archive),
        InputScope::Payload,
    )?;
    let version = bank
        .soundbank()
        .version_byte_range()
        .ok_or(EvidenceError::MissingHeader)?;
    let evidence = Evidence {
        uses,
        bank: scope,
        version: bank_offsets(version),
        complete: origin.is_some_and(|origin| origin.ancestors().all(|step| step.is_some())),
    };
    let mut fields = FieldOrigins::new();
    let directory = evidence.envelope(bank, listing, &mut fields)?;
    let objects = bank.soundbank().objects();
    for (position, row) in rows.iter().enumerate() {
        let original = objects.get(row.index).ok_or(EvidenceError::InvalidPrefix)?;
        evidence.row(position, row, original, &directory, &mut fields)?;
    }
    Ok((fields, evidence.uses.freeze()?))
}

impl Evidence {
    fn envelope(
        &self,
        bank: &Bank,
        listing: Listing,
        fields: &mut FieldOrigins,
    ) -> Result<Vec<SourceUse>> {
        put(fields, ROOT, "archive", self.uses.origin(&[], true))?;
        let path = metadata(&self.bank, "path", SourceUseRole::Payload)?;
        put(fields, ROOT, "soundbank", self.origin(&[path], true))?;
        let version = self.version_use(SourceUseRole::Payload)?;
        put(fields, ROOT, "version", self.origin(&[version], true))?;
        let id = self.word(
            "bnkBkhd",
            "bankId",
            self.version.end..self.version.end + WORD_BYTES,
            SourceUseRole::Payload,
        )?;
        put(fields, ROOT, "bank_id", self.origin(&[id], true))?;
        self.membership(bank, listing, fields)
    }

    fn membership(
        &self,
        bank: &Bank,
        listing: Listing,
        fields: &mut FieldOrigins,
    ) -> Result<Vec<SourceUse>> {
        let (mut directory, scan) = self.directory(bank)?;
        put(fields, ROOT, "total", self.origin(&directory, true))?;

        let mut selected = directory.clone();
        let mut layout = directory.clone();
        if let Some(scan) = scan {
            let frames = self.scan(&scan, "objectFrames")?;
            directory.push(frames.clone());
            layout.push(frames.clone());
            layout.push(self.scan(&scan, "typeCodes")?);
            layout.push(self.version_use(SourceUseRole::Selector)?);
            self.filter(&scan, listing, frames, &mut selected)?;
        }
        put(fields, ROOT, "matched", self.origin(&selected, true))?;
        selected.extend(directory.iter().cloned());
        put(
            fields,
            ROOT,
            "objects",
            self.origin(&selected, !listing.limited_by_bytes),
        )?;
        if listing.cursor {
            put(
                fields,
                ROOT,
                "next_cursor",
                self.origin(&selected, !listing.limited_by_bytes),
            )?;
        }
        let complete_layouts = bank
            .soundbank()
            .objects()
            .iter()
            .all(|object| matches!(object.value, HircValue::Raw));
        let layouts = self.origin(&layout, complete_layouts);
        put(fields, ROOT, "layouts", layouts.clone())?;
        for counter in ["decoded", "undecoded", "unsupported", "malformed"] {
            put(
                fields,
                "/structuredContent/layouts",
                counter,
                layouts.clone(),
            )?;
        }
        Ok(directory)
    }

    fn directory(&self, bank: &Bank) -> Result<(Vec<SourceUse>, Option<ObjectDirectory>)> {
        let soundbank = bank.soundbank();
        let mut decoded_end = 0;
        let mut hirc = None;
        for (tag, body) in soundbank.source_section_bodies() {
            decoded_end = body.end;
            if tag == SectionId::HIRC {
                hirc = Some(body);
            }
        }
        let routing = self.site(
            "bnkSectionDirectory",
            "headerScan",
            vec![decoded_end],
            SourceUseRole::Selector,
        )?;
        let mut uses = vec![routing];
        let Some(hirc) = hirc else {
            return Ok((uses, None));
        };
        uses.push(self.word(
            "bnkHirc",
            "objectCount",
            hirc.start..hirc.start + WORD_BYTES,
            SourceUseRole::Payload,
        )?);
        let directory = ObjectDirectory {
            section_body: hirc.start,
            admitted: soundbank.objects().len() as u64,
        };
        Ok((uses, Some(directory)))
    }

    fn filter(
        &self,
        scan: &ObjectDirectory,
        listing: Listing,
        frames: SourceUse,
        uses: &mut Vec<SourceUse>,
    ) -> Result<()> {
        if listing.filter_kind.is_some() {
            uses.push(self.scan(scan, "typeCodes")?);
            uses.push(self.version_use(SourceUseRole::Selector)?);
        }
        if listing.filter_id {
            // A kind suffix restricts the ID scan to objects that passed the type condition.
            let ids = listing.filter_kind.map_or_else(
                || "objectIds".to_owned(),
                |kind| format!("objectIds/{}", kind.slug()),
            );
            uses.push(self.scan(scan, &ids)?);
        }
        if listing.filter_kind.is_some() || listing.filter_id {
            uses.push(frames);
        }
        Ok(())
    }

    fn row(
        &self,
        position: usize,
        row: &ObjectView<'_>,
        original: &HircObject,
        directory: &[SourceUse],
        fields: &mut FieldOrigins,
    ) -> Result<()> {
        let parent = format!("{ROOT}/objects/{position}");
        let kind = bank_offsets(original.kind_byte_range());
        let id = bank_offsets(original.id_byte_range());
        let mut position_proof = directory.to_vec();
        position_proof.push(self.site(
            OBJECT,
            "ordinal",
            vec![kind.start, row.index as u64],
            SourceUseRole::Conversion,
        )?);
        put(fields, &parent, "index", self.origin(&position_proof, true))?;
        let original_id = self.word(OBJECT, "id", id.clone(), SourceUseRole::Payload)?;
        put(fields, &parent, "id", self.origin(&[original_id], true))?;
        let type_code = self.word(OBJECT, "typeCode", kind.clone(), SourceUseRole::Conversion)?;
        let version = self.version_use(SourceUseRole::Selector)?;
        put(
            fields,
            &parent,
            "type",
            self.origin(&[version.clone(), type_code], true),
        )?;
        if row.type_byte.is_some() {
            let raw = self.word(OBJECT, "typeCode", kind.clone(), SourceUseRole::Payload)?;
            put(fields, &parent, "type_byte", self.origin(&[raw], true))?;
        }
        let selectors = [
            version,
            self.word(OBJECT, "typeCode", kind.clone(), SourceUseRole::Selector)?,
            self.word(
                OBJECT,
                "bodyLength",
                kind.end..id.start,
                SourceUseRole::Selector,
            )?,
        ];
        self.legacy(&parent, original, &selectors, fields)
    }

    fn legacy(
        &self,
        parent: &str,
        object: &HircObject,
        selectors: &[SourceUse],
        fields: &mut FieldOrigins,
    ) -> Result<()> {
        let complete = matches!(object.value, HircValue::Raw);
        for name in ["status", "error", "parent", "detail"] {
            put(fields, parent, name, self.origin(selectors, complete))?;
        }
        let Some(count) = object.event_action_count_evidence() else {
            return put(fields, parent, "children", self.origin(selectors, complete));
        };
        let mut collection = selectors.to_vec();
        collection.push(self.word(
            EVENT,
            "actionCount",
            count.range.clone(),
            SourceUseRole::Selector,
        )?);
        if let Some(last) = count.value.checked_sub(1) {
            let range = object
                .event_action_id_byte_range(last as usize)
                .ok_or(EvidenceError::InvalidPrefix)?;
            collection.push(self.word(
                EVENT,
                "actionIds",
                count.range.end..range.end,
                SourceUseRole::Payload,
            )?);
        }
        put(fields, parent, "children", self.origin(&collection, true))?;
        let children = format!("{parent}/children");
        for ordinal in 0..count.value as usize {
            let range = object
                .event_action_id_byte_range(ordinal)
                .ok_or(EvidenceError::InvalidPrefix)?;
            let address = self.word(EVENT, "actionId", range, SourceUseRole::Payload)?;
            put(
                fields,
                &children,
                &ordinal.to_string(),
                self.origin(&[address], true),
            )?;
        }
        Ok(())
    }

    fn origin(&self, sites: &[SourceUse], complete: bool) -> FieldOrigin {
        self.uses.origin(sites, self.complete && complete)
    }

    fn version_use(&self, role: SourceUseRole) -> Result<SourceUse> {
        self.word("bnkBkhd", "version", self.version.clone(), role)
    }

    fn scan(&self, directory: &ObjectDirectory, field: &str) -> Result<SourceUse> {
        self.site(
            "bnkHircObjectDirectory",
            field,
            vec![directory.section_body, directory.admitted],
            SourceUseRole::Selector,
        )
    }

    fn word(
        &self,
        namespace: &str,
        field: &str,
        range: Range<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        range_use(&self.bank, namespace, field, range, role)
    }

    fn site(
        &self,
        namespace: &str,
        field: &str,
        indices: Vec<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        Ok(self.bank.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices,
                field: field.into(),
            },
            role,
        )?)
    }
}

struct ObjectDirectory {
    section_body: u64,
    admitted: u64,
}

const fn bank_offsets(range: Range<usize>) -> Range<u64> {
    range.start as u64..range.end as u64
}
