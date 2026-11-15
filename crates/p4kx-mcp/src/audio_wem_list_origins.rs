// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Bank, FieldOrigin, FieldOrigins, InputScope, SectionId, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};

use crate::audio_wem::payload_origins::{
    DidxWord, EvidenceError, PayloadPlan, metadata, put, range_use,
};
use crate::audio_wem_input::WemOps as _;

use super::{ListedWem, Outcome};

const ROOT: &str = "/structuredContent";
const WORD_BYTES: u64 = size_of::<u32>() as u64;

type Result<T> = std::result::Result<T, EvidenceError>;

struct Evidence {
    uses: SourceUses,
    bank: SourceScope,
    complete: bool,
}

pub(super) fn capture(
    bank: &Bank,
    rows: &[ListedWem<'_>],
    cursor: Option<&str>,
) -> Result<(FieldOrigins, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let origin = bank.archive_origin();
    let scope = uses.observe(
        origin.map_or(SourceInput::Unknown, SourceInput::Archive),
        InputScope::Payload,
    )?;
    let evidence = Evidence {
        complete: origin.is_some_and(|origin| origin.ancestors().all(|step| step.is_some())),
        uses,
        bank: scope,
    };
    let mut fields = FieldOrigins::new();
    evidence.envelope(bank, cursor, &mut fields)?;
    for (position, row) in rows.iter().enumerate() {
        evidence.row(position, row, &mut fields)?;
    }
    Ok((fields, evidence.uses.freeze()?))
}

impl Evidence {
    fn envelope(&self, bank: &Bank, cursor: Option<&str>, fields: &mut FieldOrigins) -> Result<()> {
        let path = metadata(&self.bank, "path", SourceUseRole::Payload)?;
        put(fields, ROOT, "archive", self.uses.origin(&[], true))?;
        put(fields, ROOT, "soundbank", self.origin(&[path], true))?;
        let version = bank
            .soundbank()
            .version_byte_range()
            .ok_or(EvidenceError::MissingHeader)?;
        let version = version.start as u64..version.end as u64;
        for (output, original, range) in [
            ("version", "version", version.clone()),
            ("bank_id", "bankId", version.end..version.end + WORD_BYTES),
        ] {
            let word = range_use(
                &self.bank,
                "bnkBkhd",
                original,
                range,
                SourceUseRole::Payload,
            )?;
            put(fields, ROOT, output, self.origin(&[word], true))?;
        }
        let membership = self.origin(&self.membership(bank)?, true);
        put(fields, ROOT, "total", membership.clone())?;
        put(fields, ROOT, "wems", membership.clone())?;
        if cursor.is_some() {
            put(fields, ROOT, "next_cursor", membership)?;
        }
        Ok(())
    }

    fn membership(&self, bank: &Bank) -> Result<Vec<SourceUse>> {
        let mut decoded_end = 0;
        let mut didx = None;
        let mut data = None;
        for (tag, body) in bank.soundbank().source_section_bodies() {
            decoded_end = body.end;
            match tag {
                SectionId::DIDX => didx = Some(body),
                SectionId::DATA => data = Some(body),
                _ => {}
            }
        }
        let routing = self.site(
            "bnkSectionDirectory",
            "headerScan",
            &[decoded_end],
            SourceUseRole::Selector,
        )?;
        let mut membership = vec![routing];
        if let (Some(didx), Some(data)) = (didx, data) {
            membership.push(range_use(
                &self.bank,
                "bnkDidx",
                "triplets",
                didx,
                SourceUseRole::Conversion,
            )?);
            membership.push(range_use(
                &self.bank,
                "bnkData",
                "bodyLength",
                data.start - WORD_BYTES..data.start,
                SourceUseRole::Selector,
            )?);
        }
        Ok(membership)
    }

    fn row(&self, index: usize, row: &ListedWem<'_>, fields: &mut FieldOrigins) -> Result<()> {
        let parent = format!("{ROOT}/wems/{index}");
        let wem = &row.wem;
        let triplet = wem.source().triplet_offset();
        for (name, field) in [("id", DidxWord::Id), ("size", DidxWord::Length)] {
            let word = field.bind(&self.bank, triplet, SourceUseRole::Payload)?;
            put(fields, &parent, name, self.origin(&[word], true))?;
        }
        let plan = match &row.outcome {
            Outcome::Prefetch => PayloadPlan::embedded_failure(wem, None),
            Outcome::Parsed(parsed) => {
                PayloadPlan::parsed(&parsed.origin_plan(), wem.bytes().len() as u64, Some(wem))
            }
            Outcome::Unreadable(error) => PayloadPlan::embedded_failure(wem, Some(error)),
        };
        plan.record(&self.uses, &self.bank, self.complete, &parent, fields)
    }

    fn site(
        &self,
        namespace: &str,
        field: &str,
        indices: &[u64],
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        Ok(self.bank.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices: indices.to_vec(),
                field: field.into(),
            },
            role,
        )?)
    }

    fn origin(&self, sites: &[SourceUse], complete: bool) -> FieldOrigin {
        self.uses.origin(sites, self.complete && complete)
    }
}
