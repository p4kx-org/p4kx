// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank_wem_evidence::{BankWemEvidence, BankWemRead};
use p4kx_extraction::wem_payload_evidence::{EvidenceError, put};
use p4kx_extraction::{Bank, FieldOrigins, SourceInventory};

use crate::audio_wem::payload_origins::MCP_FIELDS;

const ROOT: &str = "/structuredContent";

pub(super) fn capture(
    bank: &Bank,
    rows: &[BankWemRead<'_>],
    cursor: Option<&str>,
) -> Result<(FieldOrigins, SourceInventory), EvidenceError> {
    let evidence = BankWemEvidence::new(bank)?;
    let mut fields = FieldOrigins::new();
    put(&mut fields, ROOT, "archive", evidence.request())?;
    evidence.header(ROOT, &mut fields)?;
    let membership = evidence.membership()?;
    put(&mut fields, ROOT, "total", membership.clone())?;
    put(&mut fields, ROOT, "wems", membership.clone())?;
    if cursor.is_some() {
        put(&mut fields, ROOT, "next_cursor", membership)?;
    }
    for (position, row) in rows.iter().enumerate() {
        evidence.row(
            &format!("{ROOT}/wems/{position}"),
            row,
            MCP_FIELDS,
            &mut fields,
        )?;
    }
    Ok((fields, evidence.finish()?))
}
