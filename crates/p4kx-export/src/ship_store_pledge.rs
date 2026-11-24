// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::HashSet;
use std::num::NonZeroU32;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, de::Error};

use crate::CatalogError;
use crate::schema_fields::{compact_string_enum, every_field_required};
use crate::schema_vocabulary::open_string_vocabulary;

const DATA: &str = include_str!("../data/ship-store-pledges.json");
const SHIP_MATRIX: &str = "https://robertsspaceindustries.com/ship-matrix/index";

/// The published production status of an admitted Store pledge.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[non_exhaustive]
#[schemars(transform = open_string_vocabulary)]
pub enum ShipPledgeProductionStatus {
    /// The Ship Matrix labels the pledge In Concept.
    #[serde(rename = "in-concept")]
    InConcept,
}

/// The review population described by a curated Store pledge snapshot.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = compact_string_enum)]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum ShipStorePledgeScope {
    /// The reviewed candidate cohort from the dated source capture.
    ReviewedCandidates,
}

/// The human-reviewed source and revision of a bundled Store pledge section.
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ShipStorePledgeSnapshot(SnapshotData);

/// The source identity and review scope of the bundled Store pledges.
#[derive(Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "ShipStorePledgeSnapshot")]
#[schemars(transform = every_field_required)]
struct SnapshotData {
    /// The official source of the original manually captured population.
    #[schemars(extend("const" = SHIP_MATRIX))]
    source: String,
    /// The date of the original population capture in YYYY-MM-DD form.
    #[schemars(extend("format" = "date"))]
    capture_date: String,
    /// The SHA-256 digest of the original source capture, in lowercase hexadecimal.
    #[schemars(regex(pattern = "^[a-f0-9]{64}$"))]
    source_revision: String,
    /// The date on which the candidate review completed in YYYY-MM-DD form.
    #[schemars(extend("format" = "date"))]
    reviewed_date: String,
    /// The positive revision number of these curated rows.
    revision: NonZeroU32,
    /// The population to which the review applies.
    #[serde(deserialize_with = "decode_scope")]
    scope: ShipStorePledgeScope,
}

impl JsonSchema for ShipStorePledgeSnapshot {
    fn schema_name() -> Cow<'static, str> {
        SnapshotData::schema_name()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        SnapshotData::json_schema(generator)
    }
}

impl ShipStorePledgeSnapshot {
    /// Returns the official source URL.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.0.source
    }

    /// Returns the original capture date as YYYY-MM-DD.
    #[must_use]
    pub fn capture_date(&self) -> &str {
        &self.0.capture_date
    }

    /// Returns the original capture's SHA-256 digest.
    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.0.source_revision
    }

    /// Returns the candidate review date as YYYY-MM-DD.
    #[must_use]
    pub fn reviewed_date(&self) -> &str {
        &self.0.reviewed_date
    }

    /// Returns the curated row revision.
    #[must_use]
    pub const fn revision(&self) -> u32 {
        self.0.revision.get()
    }

    /// Returns the scope the recorded review covered.
    #[must_use]
    pub const fn scope(&self) -> ShipStorePledgeScope {
        self.0.scope
    }
}

/// One reviewed non-flight-ready Store pledge, keyed by its RSI ship identifier.
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ShipStorePledge(PledgeData);

/// One reviewed Store pledge and its published production status.
#[derive(Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "ShipStorePledge")]
#[schemars(transform = every_field_required)]
struct PledgeData {
    /// The positive RSI ship identifier.
    rsi_id: NonZeroU32,
    /// The exact case-sensitive Store slug.
    #[schemars(length(min = 1))]
    slug: String,
    /// The short public Store name.
    #[schemars(length(min = 1))]
    store_name: String,
    /// The manufacturer code published by RSI.
    #[schemars(length(min = 1))]
    manufacturer_code: String,
    /// The published non-flight-ready production status.
    #[serde(deserialize_with = "decode_production_status")]
    production_status: ShipPledgeProductionStatus,
}

impl JsonSchema for ShipStorePledge {
    fn schema_name() -> Cow<'static, str> {
        PledgeData::schema_name()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        PledgeData::json_schema(generator)
    }
}

impl ShipStorePledge {
    /// Returns the RSI ship identifier.
    #[must_use]
    pub const fn rsi_id(&self) -> u32 {
        self.0.rsi_id.get()
    }

    /// Returns the original Store slug.
    #[must_use]
    pub fn slug(&self) -> &str {
        &self.0.slug
    }

    /// Returns the short Store name.
    #[must_use]
    pub fn store_name(&self) -> &str {
        &self.0.store_name
    }

    /// Returns the RSI manufacturer code.
    #[must_use]
    pub fn manufacturer_code(&self) -> &str {
        &self.0.manufacturer_code
    }

    /// Returns the reviewed published production status.
    #[must_use]
    pub const fn production_status(&self) -> ShipPledgeProductionStatus {
        self.0.production_status
    }
}

/// The complete bundled section within its reviewed candidate scope.
#[derive(Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ShipStorePledges {
    /// Shared source identity and review scope for every row.
    snapshot: ShipStorePledgeSnapshot,
    /// All admitted rows in strictly ascending RSI ship identifier order.
    rows: Vec<ShipStorePledge>,
}

impl ShipStorePledges {
    /// Borrows the review identity and scope carried by this section.
    #[must_use]
    pub const fn snapshot(&self) -> &ShipStorePledgeSnapshot {
        &self.snapshot
    }

    /// Borrows the reviewed rows in ascending RSI identifier order.
    #[must_use]
    pub fn rows(&self) -> &[ShipStorePledge] {
        &self.rows
    }

    /// Returns the reviewed row for an exact RSI identifier when present in this section.
    #[must_use]
    pub fn by_rsi_id(&self, rsi_id: u32) -> Option<&ShipStorePledge> {
        let index = self
            .rows
            .binary_search_by_key(&rsi_id, ShipStorePledge::rsi_id)
            .ok()?;
        self.rows.get(index)
    }
}

/// Reads the same reviewed Store pledge section included in Catalog v3.
///
/// This reads bundled metadata and needs no Archive, Filter or network.
///
/// # Errors
/// Returns an error if the bundled snapshot or rows violate their source contract.
pub fn ship_store_pledges() -> Result<ShipStorePledges, CatalogError> {
    parse(DATA)
}

fn parse(text: &str) -> Result<ShipStorePledges, CatalogError> {
    #[derive(Deserialize)]
    struct BundleData {
        snapshot: SnapshotData,
        rows: Vec<PledgeData>,
    }

    let data: BundleData =
        serde_json::from_str(text).map_err(|source| invalid(source.to_string()))?;
    validate_snapshot(&data.snapshot)?;
    validate_rows(&data.rows)?;
    Ok(ShipStorePledges {
        snapshot: ShipStorePledgeSnapshot(data.snapshot),
        rows: data.rows.into_iter().map(ShipStorePledge).collect(),
    })
}

fn decode_scope<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<ShipStorePledgeScope, D::Error> {
    const REVIEWED_CANDIDATES: &str = "reviewedCandidates";
    match String::deserialize(deserializer)?.as_str() {
        REVIEWED_CANDIDATES => Ok(ShipStorePledgeScope::ReviewedCandidates),
        scope => Err(D::Error::unknown_variant(scope, &[REVIEWED_CANDIDATES])),
    }
}

fn decode_production_status<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<ShipPledgeProductionStatus, D::Error> {
    const IN_CONCEPT: &str = "in-concept";
    match String::deserialize(deserializer)?.as_str() {
        IN_CONCEPT => Ok(ShipPledgeProductionStatus::InConcept),
        status => Err(D::Error::unknown_variant(status, &[IN_CONCEPT])),
    }
}

fn validate_snapshot(snapshot: &SnapshotData) -> Result<(), CatalogError> {
    if snapshot.source != SHIP_MATRIX {
        return Err(invalid("unrecognized Ship Matrix source"));
    }
    if snapshot.source_revision.len() != 64
        || !snapshot
            .source_revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("expected a lowercase SHA-256 source revision"));
    }
    let capture = date(&snapshot.capture_date).ok_or_else(|| invalid("invalid capture date"))?;
    let reviewed = date(&snapshot.reviewed_date).ok_or_else(|| invalid("invalid review date"))?;
    if reviewed < capture {
        return Err(invalid("the review date precedes the source capture"));
    }
    Ok(())
}

fn date(text: &str) -> Option<(u16, u8, u8)> {
    let mut fields = text.split('-');
    let year = decimal(fields.next()?, 4)?;
    let month = u8::try_from(decimal(fields.next()?, 2)?).ok()?;
    let day = u8::try_from(decimal(fields.next()?, 2)?).ok()?;
    if fields.next().is_some() || year == 0 {
        return None;
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => 28 + u8::from(leap),
        _ => return None,
    };
    (1..=days).contains(&day).then_some((year, month, day))
}

fn decimal(text: &str, width: usize) -> Option<u16> {
    if text.len() != width || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn validate_rows(rows: &[PledgeData]) -> Result<(), CatalogError> {
    let mut previous = 0;
    let mut slugs = HashSet::with_capacity(rows.len());
    for row in rows {
        if row.rsi_id.get() <= previous {
            return Err(invalid(
                "RSI identifiers must be unique and strictly ascending",
            ));
        }
        previous = row.rsi_id.get();
        if !slugs.insert(row.slug.as_str()) {
            return Err(invalid("Store slugs must be unique"));
        }
        if row.slug.is_empty()
            || row.slug.contains(['/', '?', '#'])
            || row.slug.chars().any(char::is_whitespace)
            || row.store_name.trim().is_empty()
            || row.manufacturer_code.trim().is_empty()
        {
            return Err(invalid(
                "each pledge requires a Store slug, short name and manufacturer",
            ));
        }
    }
    Ok(())
}

fn invalid(reason: impl Into<String>) -> CatalogError {
    CatalogError::ShipStorePledges(reason.into())
}

#[cfg(test)]
#[path = "ship_store_pledge_tests.rs"]
mod tests;
