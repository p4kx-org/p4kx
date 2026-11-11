// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::BankHircObject;

use crate::audio_bank_hirc_origins::{ObjectSites, inventory_sites};
use crate::audio_bank_origins::{BankOrigins, EvidenceSite};
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

use super::super::origins::Sites;
use super::ForeignLookup;

pub(super) fn resolution(
    input: &MediaOriginInput,
    sites: &Sites,
    complete: bool,
    foreign: &ForeignLookup,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    prepared.indices(prepared.capture([sites.route()])?)?;
    prepared.field(
        "/data/payload/target_id",
        prepared.capture([sites.target_site()?])?,
    )?;
    let observed = prepared.capture(selectors(sites)?.chain(inventory_sites()))?;
    for field in [
        "/data/payload/local/observed_candidates",
        "/data/payload/local/matching_candidates",
        "/data/payload/local/candidates_complete",
    ] {
        prepared.field(field, observed.clone())?;
    }
    let resolution = if complete {
        observed
    } else {
        let mut capture = writer.capture_origins();
        capture.include(&observed, writer)?;
        capture.include(&OriginContributions::unknown(&input.owner), writer)?;
        capture.finish()
    };
    prepared.field("/data/payload/local/resolution", resolution)?;
    foreign_fields(&mut prepared, sites, foreign)?;
    Ok(prepared.finish())
}

fn foreign_fields(
    prepared: &mut BankOrigins<'_, '_, '_>,
    sites: &Sites,
    foreign: &ForeignLookup,
) -> Result<(), MediaError> {
    let selector = selectors(sites)?.chain(sites.bank_selectors());
    if matches!(foreign, ForeignLookup::LocalOnly) {
        return prepared.field("/data/payload/foreign/status", prepared.capture(selector)?);
    }
    let status = prepared.capture(selector.chain(inventory_sites()))?;
    prepared.field("/data/payload/foreign/status", status.clone())?;
    if matches!(foreign, ForeignLookup::Unavailable { .. }) {
        prepared.field("/data/payload/foreign/reason", status)?;
    }
    prepared.subtree(
        "/data/payload/foreign/target",
        prepared.capture(selectors(sites)?.chain(sites.bank_selectors()))?,
    )
}

pub(super) fn candidate(
    input: &MediaOriginInput,
    sites: &Sites,
    target: &BankHircObject,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let target_sites = ObjectSites::new(target);
    let evidence = prepared.capture(selectors(sites)?.chain(target_sites.target_sites()))?;
    prepared.indices(evidence.clone())?;
    let route = prepared.capture([sites.route()])?;
    prepared.field("/subresource/indices/0", route.clone())?;
    prepared.field("/subresource/indices/1", route)?;
    target_sites.target_fields(&mut prepared, &evidence)?;
    prepared.field("/data/payload/matches_target_kind", evidence)?;
    Ok(prepared.finish())
}

fn selectors(sites: &Sites) -> Result<impl Iterator<Item = EvidenceSite<'_>>, MediaError> {
    Ok(sites
        .base()
        .into_iter()
        .chain([sites.action_selector()?, sites.target_site()?]))
}
