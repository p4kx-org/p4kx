// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `list_soundbank_objects`: one page of the `HIRC` objects of a soundbank, in the shape of
//! `p4kx bnk objects --json`.

use std::io::ErrorKind;
use std::sync::Arc;

use p4kx_extraction::hirc_select::{LayoutCounts, select_with_sources};
use p4kx_extraction::hirc_view::ObjectView;
use p4kx_extraction::{FieldOrigins, HircKind, SourceInventory};
use schemars::JsonSchema;
use serde::{Serialize, Serializer};
use serde_json::value::RawValue;

use crate::archives::Opened;
use crate::audio_params::SoundbankObjectsParams;
use crate::audio_wem::{EvidenceError, open_bank};
use crate::budget::{Capped, MAX_RESPONSE_LEN, fill_within, fit_prefix};
use crate::error::ToolError;
use crate::page::Page;

#[path = "audio_objects_origins.rs"]
mod origins;

/// The `HIRC` objects of a soundbank, one page.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SoundbankObjectsResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Entry path of the soundbank.
    pub(crate) soundbank: String,
    /// The bank version.
    pub(crate) version: u32,
    /// The soundbank ID.
    pub(crate) bank_id: u32,
    /// How many objects the soundbank holds.
    pub(crate) total: usize,
    /// How many objects of the whole bank the reader decodes, does not decode, does not support
    /// in the bank's version or finds malformed.
    pub(crate) layouts: LayoutCounts,
    /// How many objects pass the filters, on this page and beyond it.
    pub(crate) matched: usize,
    /// The matching objects on this page in bank order, as `p4kx bnk objects --json` lists them.
    #[schemars(with = "Vec<ObjectView<'static>>")]
    pub(crate) objects: Vec<Listed>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
    #[serde(rename = "fieldOrigins")]
    pub(crate) field_origins: FieldOrigins,
    pub(crate) provenance: SourceInventory,
}

/// An object rendered once as JSON, with its ID for messages.
#[derive(Debug, Clone)]
pub(crate) struct Listed {
    id: u32,
    json: Arc<RawValue>,
}

impl Serialize for Listed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.json.serialize(serializer)
    }
}

impl Listed {
    fn new(view: &ObjectView<'_>) -> Result<Self, ToolError> {
        let mut output = Capped::new(MAX_RESPONSE_LEN);
        if let Err(error) = serde_json::to_writer(&mut output, view) {
            if error.io_error_kind() == Some(ErrorKind::FileTooLarge) {
                return Err(ToolError::TooLargeItem {
                    what: format!("object {}", view.id),
                    limit: MAX_RESPONSE_LEN,
                    hint: "the complete original object exceeds one response",
                });
            }
            return Err(error.into());
        }
        let json = RawValue::from_string(String::from_utf8(output.into_bytes())?)?;
        Ok(Self {
            id: view.id,
            json: json.into(),
        })
    }
}

/// Returns the object type `text` names, as [`HircKind::slug`] spells it.
fn parse_kind(text: &str) -> Result<HircKind, ToolError> {
    HircKind::from_slug(text).ok_or_else(|| {
        let known: Vec<&str> = HircKind::known().map(HircKind::slug).collect();
        ToolError::Refused(format!(
            "`{text}` is no object type; the types are {}",
            known.join(", ")
        ))
    })
}

/// Returns one page of the `HIRC` objects of the soundbank at `params.soundbank` that pass the
/// filters.
pub(crate) fn soundbank_objects(
    opened: &Opened,
    params: &SoundbankObjectsParams,
    page: Page,
) -> Result<SoundbankObjectsResult, ToolError> {
    let kind = params.kind.as_deref().map(parse_kind).transpose()?;
    let bank = open_bank(opened, &params.soundbank)?;
    let soundbank = bank.soundbank();
    let selected = select_with_sources(&bank, kind, params.id, page.offset(), page.limit());
    page.check(selected.matched)?;
    let items = selected.shown.iter().map(Listed::new);
    let (candidates, _) = fill_within(page, selected.matched, 0, items, Listed::describe)
        .map_err(narrowest_filter)?;
    let LayoutCounts {
        decoded,
        undecoded,
        unsupported,
        malformed,
    } = LayoutCounts::of(soundbank.objects());
    let response = |count| {
        let rows = selected
            .shown
            .get(..count)
            .ok_or(ToolError::HircEvidence(EvidenceError::InvalidPrefix))?;
        let objects = candidates
            .get(..count)
            .ok_or(ToolError::HircEvidence(EvidenceError::InvalidPrefix))?;
        let next_cursor = page.next_cursor_after(count, selected.matched)?;
        let (field_origins, provenance) = origins::capture(
            &bank,
            rows,
            origins::Listing {
                filter_kind: kind,
                filter_id: params.id.is_some(),
                cursor: next_cursor.is_some(),
                limited_by_bytes: count < selected.shown.len(),
            },
        )
        .map_err(ToolError::HircEvidence)?;
        Ok(SoundbankObjectsResult {
            archive: opened.archive().path().display().to_string(),
            soundbank: bank.name().to_owned(),
            version: soundbank.header().version,
            bank_id: soundbank.header().id,
            total: soundbank.objects().len(),
            layouts: LayoutCounts {
                decoded,
                undecoded,
                unsupported,
                malformed,
            },
            matched: selected.matched,
            objects: objects.to_vec(),
            next_cursor,
            field_origins,
            provenance,
        })
    };
    let (_, result) = fit_prefix(candidates.len(), response, || {
        candidates
            .first()
            .map_or_else(|| "soundbank object listing".to_owned(), Listed::describe)
    })
    .map_err(narrowest_filter)?;
    Ok(result)
}

impl Listed {
    fn describe(&self) -> String {
        format!("object {}", self.id)
    }
}

/// Rewrites the hint of an object too large for a response: it is too large alone, so the ID
/// filter cannot narrow it further.
fn narrowest_filter(error: ToolError) -> ToolError {
    match error {
        ToolError::TooLargeItem { what, limit, .. } => ToolError::TooLargeItem {
            what,
            limit,
            hint: "the object alone exceeds it, so no filter narrows it further; read the \
                   soundbank with `p4kx bnk objects` instead",
        },
        other => other,
    }
}
