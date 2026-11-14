// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::BankHircObject;
use p4kx_extraction::{InputScope, SourceUseRole};

use crate::audio_bank_hirc_origins::ObjectSites;
use crate::audio_bank_origins::section_routing;
use crate::audio_record_origins as records;
use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

use super::super::bank::{Bank, HircRead};
use super::super::{Blocker, Header, HeaderRead, Query};

pub(super) fn bank(
    capture: &mut OriginCapture,
    bank: &Bank<'_>,
    relevant: bool,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    for field in ["path", "entry_index", "encryption"] {
        capture.add(
            &bank.origin,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-directory", &[], field),
            SourceUseRole::Selector,
            writer,
        )?;
    }
    match bank.header {
        HeaderRead::Known(header) => {
            for field in ["bankId", "version"] {
                capture.add(
                    &bank.origin,
                    InputScope::Payload,
                    OriginSite::new("bnkBkhd", &[header.body], field),
                    SourceUseRole::Selector,
                    writer,
                )?;
            }
        }
        HeaderRead::Unavailable(
            Blocker::MalformedBank
            | Blocker::IncompleteBankDirectory
            | Blocker::MissingBankHeader
            | Blocker::MalformedBankHeader
            | Blocker::AmbiguousBankHeader,
        ) => {
            capture.add(
                &bank.origin,
                InputScope::Payload,
                OriginSite::new("bnkDirectory", &[], "headerAvailability"),
                SourceUseRole::Selector,
                writer,
            )?;
        }
        HeaderRead::Unavailable(_) => {}
    }
    if relevant && matches!(bank.hirc, HircRead::Observed(_)) {
        for (site, role) in crate::audio_bank_hirc_origins::inventory_sites() {
            capture.add(&bank.origin, InputScope::Payload, site, role, writer)?;
        }
    }
    Ok(())
}

pub(super) fn search(
    owner: &MediaOriginInput,
    query: &Query,
    evidence: OriginContributions,
    complete: bool,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = records::prepare_for(owner, OriginProfile::AudioBank)?;
    records::indices(&mut prepared, query.evidence.clone(), writer)?;
    records::field(
        &mut prepared,
        "/data/payload/owner_entry_index",
        directory_field(&query.source, "entry_index", writer)?,
        writer,
    )?;
    records::field(
        &mut prepared,
        "/data/payload/target_id",
        query.evidence.clone(),
        writer,
    )?;
    records::subtree(
        &mut prepared,
        "/data/payload/selector",
        query.evidence.clone(),
        writer,
    )?;
    records::field(
        &mut prepared,
        "/data/payload/directory_complete",
        crate::origin_lookup::directory(Some(owner), writer)?,
        writer,
    )?;
    for path in [
        "/data/payload/candidates_complete",
        "/data/payload/observed_candidates",
        "/data/payload/matching_candidates",
        "/data/payload/blocked_banks",
    ] {
        records::field(&mut prepared, path, evidence.clone(), writer)?;
    }
    let resolution = if complete {
        evidence
    } else {
        let mut capture = writer.capture_origins();
        capture.include(&evidence, writer)?;
        capture.include(&OriginContributions::unknown(&owner.owner), writer)?;
        capture.finish()
    };
    records::field(
        &mut prepared,
        "/data/payload/resolution",
        resolution,
        writer,
    )?;
    Ok(prepared)
}

pub(super) fn candidate(
    owner: &MediaOriginInput,
    query: &Query,
    target: &Bank<'_>,
    header: Header,
    object: &BankHircObject,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = records::prepare_for(owner, OriginProfile::AudioBank)?;
    let sites = ObjectSites::new(object);
    let selectors = sites.target_sites();
    let mut capture = writer.capture_origins();
    capture.include(&query.evidence, writer)?;
    bank(&mut capture, target, true, writer)?;
    for (site, role) in selectors {
        capture.add(&target.origin, InputScope::Payload, site, role, writer)?;
    }
    let link = capture.finish();
    records::indices(&mut prepared, link.clone(), writer)?;
    records::field(
        &mut prepared,
        "/data/payload/matches_target_kind",
        link,
        writer,
    )?;
    records::field(
        &mut prepared,
        "/data/payload/target_entry_index",
        directory_field(&target.origin, "entry_index", writer)?,
        writer,
    )?;
    let bkhd = [header.section];
    let hirc = [object.section_ordinal()];
    for (path, site) in [
        (
            "/data/payload/target_bkhd_section_ordinal",
            section_routing(&bkhd),
        ),
        (
            "/data/payload/target_hirc_section_ordinal",
            section_routing(&hirc),
        ),
        ("/data/payload/target_object_ordinal", sites.route()),
        ("/data/payload/target_type_code", selectors[2]),
    ] {
        let evidence = records::capture(&target.origin, [site], writer)?;
        records::field(&mut prepared, path, evidence, writer)?;
    }
    Ok(prepared)
}

pub(super) fn blocker(
    owner: &MediaOriginInput,
    query: &Query,
    target: &Bank<'_>,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut capture = writer.capture_origins();
    capture.include(&query.evidence, writer)?;
    bank(&mut capture, target, true, writer)?;
    let evidence = capture.finish();
    let mut prepared = records::prepare_for(owner, OriginProfile::AudioBank)?;
    records::indices(&mut prepared, evidence.clone(), writer)?;
    records::field(&mut prepared, "/data/payload/reason", evidence, writer)?;
    for (path, field) in [
        ("/data/payload/target_entry_index", "entry_index"),
        ("/data/payload/filtered", "path"),
        ("/data/payload/key_required", "encryption"),
    ] {
        records::field(
            &mut prepared,
            path,
            directory_field(&target.origin, field, writer)?,
            writer,
        )?;
    }
    Ok(prepared)
}

fn directory_field(
    input: &MediaOriginInput,
    field: &str,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.add(
        input,
        InputScope::ArchiveDirectoryMetadata,
        OriginSite::new("archive-directory", &[], field),
        SourceUseRole::Selector,
        writer,
    )?;
    Ok(capture.finish())
}
