// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, Entry, SourceUseRole};

use super::super::family_origin;
use super::super::family_payload::{
    FamilyPayload, MEMBER, Member, MemberFilter, MemberIdentity, PayloadAccess, Relation,
};
use super::super::origin::put;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{Identity, MediaError, MediaWriter, ReferenceCandidate, Status, Subresource};

pub(super) fn relationship(
    owner: &MediaOriginInput,
    owner_index: usize,
    member: &MediaOriginInput,
    member_index: usize,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    for (input, index) in [(owner, owner_index), (member, member_index)] {
        capture.include(
            &family_origin::entry(input, index as u64, "path", SourceUseRole::Selector, writer)?,
            writer,
        )?;
    }
    Ok(capture.finish())
}

pub(super) struct MemberProjection<'a> {
    pub(super) input: &'a MediaOriginInput,
    pub(super) entry: &'a Entry,
    pub(super) relation: Relation,
    pub(super) reference: ReferenceCandidate,
    pub(super) filter: MemberFilter,
    pub(super) relationship: OriginContributions,
    pub(super) size: OriginContributions,
    pub(super) key_missing: bool,
}

impl MemberProjection<'_> {
    pub(super) fn write(
        self,
        owner: &MediaOriginInput,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let Identity::Entry { entry, .. } = self.reference.target.source else {
            return Err(MediaError::Contract(
                "DDS family candidate has no original Entry".into(),
            ));
        };
        let index = self.entry.index() as u64;
        let mut prepared = family_origin::prepare(owner)?;
        put(
            &mut prepared,
            "/data/payload/relation",
            self.relationship,
            writer,
        )?;
        entry_origins(
            &mut prepared,
            self.input,
            index,
            entry.raw_path_hex.is_some(),
            writer,
        )?;
        selector_origins(&mut prepared, self.input, index, writer)?;
        put(
            &mut prepared,
            "/data/payload/declared_bytes",
            self.size,
            writer,
        )?;

        prepared.write(
            writer,
            &Subresource::new(MEMBER, vec![index]),
            Status::Complete,
            &FamilyPayload::Member(Member {
                entry: MemberIdentity {
                    entry_index: entry.entry_index,
                    path: entry.path,
                    raw_path_hex: entry.raw_path_hex,
                },
                relation: self.relation,
                selection: self.reference.state,
                filter: self.filter,
                declared_bytes: DecimalU64::new(self.entry.uncompressed_size()),
                payload_access: if self.key_missing && self.entry.is_encrypted() {
                    PayloadAccess::KeyRequired
                } else {
                    PayloadAccess::NotInspected
                },
            }),
            &[],
        )
    }
}

fn entry_origins(
    prepared: &mut PreparedRecordOrigins,
    input: &MediaOriginInput,
    index: u64,
    lossy: bool,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let fields = [("entry_index", "entryIndex"), ("path", "path")];
    let raw_path = lossy.then_some(("raw_path_hex", "rawPath"));
    for (wire, original) in fields.into_iter().chain(raw_path) {
        let proof = family_origin::entry(input, index, original, SourceUseRole::Payload, writer)?;
        put(
            prepared,
            &format!("/data/payload/entry/{wire}"),
            proof,
            writer,
        )?;
    }
    Ok(())
}

fn selector_origins(
    prepared: &mut PreparedRecordOrigins,
    input: &MediaOriginInput,
    index: u64,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let selection = family_origin::entry(input, index, "path", SourceUseRole::Selector, writer)?;
    put(
        prepared,
        "/data/payload/selection",
        selection.clone(),
        writer,
    )?;
    put(prepared, "/data/payload/filter", selection, writer)?;
    let access = family_origin::entry(input, index, "encryption", SourceUseRole::Selector, writer)?;
    put(prepared, "/data/payload/payload_access", access, writer)?;
    let identity =
        family_origin::entry(input, index, "entryIndex", SourceUseRole::Selector, writer)?;
    put(prepared, "/subresource/indices", identity, writer)
}
