// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, CancelToken, Entry, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, EntryJson, EntryRead, EntryReceipt, OriginalDatum,
    PhysicalEntryIdentity, PhysicalEntryRef,
};
use crate::catalog_read::parsed_entry_json::ParsedEntryJson;
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::schema_fields::every_field_required;
use crate::shop_inventory_compatibility::{self as compatibility, ItemList};
use crate::shop_inventory_fields::ShopInventoryFields;
use crate::shop_inventory_source::ShopInventorySource;
use crate::starmap_index;
use crate::{CatalogError, Kind};

/// One physical offline inventory file, independent of whether a shop resolves it.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ShopInventoryDefinition {
    #[serde(flatten)]
    pub(super) identity: PhysicalEntryIdentity,
    pub(super) document: SourcedValue<ShopInventoryDocument>,
}

impl ShopInventoryDefinition {
    /// Borrows the physical source identity of this definition.
    #[must_use]
    pub const fn identity(&self) -> &PhysicalEntryIdentity {
        &self.identity
    }

    /// Borrows the complete authored document or its captured source state.
    #[must_use]
    pub const fn document(&self) -> &SourcedValue<ShopInventoryDocument> {
        &self.document
    }
}

/// Full authored source and direct typed navigation for the two inventory shapes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ShopInventoryDocument {
    source: ShopInventorySource,
    fields: ShopInventoryFields,
}

impl ShopInventoryDocument {
    /// Borrows all original values and their source coordinates.
    #[must_use]
    pub const fn source(&self) -> &ShopInventorySource {
        &self.source
    }

    /// Borrows typed navigation to the authored inventory fields.
    #[must_use]
    pub const fn fields(&self) -> &ShopInventoryFields {
        &self.fields
    }
}

pub(super) struct ReadInventory {
    pub(super) definition: ShopInventoryDefinition,
    pub(super) items: RecordedItems,
    file_outcome: Outcome,
    content: Option<SourceUse>,
}

pub(crate) struct InventoryFileEvidence {
    receipt: EntryReceipt,
    pub(crate) content: Option<SourceUse>,
}

impl InventoryFileEvidence {
    pub(crate) fn reference(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<OriginalDatum<PhysicalEntryRef>, CatalogError> {
        self.receipt.physical_reference(capture)
    }
}

pub(super) struct RecordedItems {
    source: EntrySource,
    list: ItemList,
    outcome: Outcome,
}

#[derive(Clone)]
struct Outcome {
    disposition: SourceDisposition,
    diagnosis: EntryDiagnosis,
}

impl Outcome {
    fn projected() -> Self {
        Self {
            disposition: SourceDisposition::Projected,
            diagnosis: EntryDiagnosis::default(),
        }
    }

    const fn malformed(field: &'static str) -> Self {
        Self {
            disposition: SourceDisposition::ProjectionFailed,
            diagnosis: crate::shop_entry_read::diagnosis(
                ExportDiagnosticCode::MalformedSource,
                field,
            ),
        }
    }

    fn record(self, source: EntrySource, kind: Kind, ledger: &mut ExportLedger) {
        source.diagnosed(ledger, kind, self.disposition, self.diagnosis);
    }
}

struct ReadDocument {
    document: SourcedValue<ShopInventoryDocument>,
    list: ItemList,
    file_outcome: Outcome,
    item_outcome: Outcome,
    content: Option<SourceUse>,
}

impl ReadInventory {
    pub(super) fn read(
        archive: &Archive,
        entry: &Entry,
        capture: &EntryCapture<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let source =
            EntrySource::outer(capture, archive, entry)?.operation(EntryOperation::ShopInventory);
        let identity = source.receipt().physical_file(starmap_index::key)?;
        let uses = source.receipt().source_uses();
        let result = if entry.uncompressed_size() == 0 {
            EntryRead::read_verified(source.receipt().clone(), archive, entry, cancel)?
        } else {
            crate::shop_entry_read::read_all(capture, &source, archive, entry, cancel)?
        };
        let read = match result {
            Ok(read) => document(&read, capture, entry, uses, cancel)?,
            Err(error) => failed(
                SourceValue::Unreadable,
                entry.path(),
                &error,
                uses,
                Outcome {
                    disposition: SourceDisposition::ReadFailed,
                    diagnosis: error.diagnosis(),
                },
            ),
        };
        Ok(Self {
            definition: ShopInventoryDefinition {
                identity,
                document: read.document,
            },
            items: RecordedItems {
                source,
                list: read.list,
                outcome: read.item_outcome,
            },
            file_outcome: read.file_outcome,
            content: read.content,
        })
    }

    pub(super) fn retain(
        self,
        ledger: &mut ExportLedger,
    ) -> (
        ShopInventoryDefinition,
        RecordedItems,
        InventoryFileEvidence,
    ) {
        self.file_outcome
            .record(self.items.source.clone(), Kind::ShopInventories, ledger);
        let evidence = InventoryFileEvidence {
            receipt: self.items.source.receipt().clone(),
            content: self.content,
        };
        (self.definition, self.items, evidence)
    }
}

impl RecordedItems {
    pub(super) fn selected(self, ledger: &mut ExportLedger) -> ItemList {
        self.outcome
            .record(self.source.dependency(), Kind::Shops, ledger);
        self.list
    }
}

fn document(
    read: &EntryRead,
    capture: &EntryCapture<'_>,
    entry: &Entry,
    uses: &SourceUses,
    cancel: Option<&CancelToken>,
) -> Result<ReadDocument, CatalogError> {
    let path = entry.path();
    if entry.uncompressed_size() == 0
        && let Some(content) = read.empty_payload(capture)?
    {
        return Ok(empty_source(path, uses, content));
    }
    let Ok(root) = EntryJson::parse(read, capture)? else {
        return Ok(ReadDocument {
            item_outcome: Outcome::malformed("Collection.Inventory"),
            ..failed(
                SourceValue::Invalid,
                path,
                &compatibility::why_unparsed(read.bytes()),
                uses,
                Outcome::malformed("content"),
            )
        });
    };
    let parsed = ParsedEntryJson::read(root, cancel)?;
    let fields = ShopInventoryFields::read(&parsed, uses, cancel)?;
    let items = compatibility::items(&fields, &parsed, cancel)?;
    let public_strings = fields.public_strings(&parsed, cancel)?;
    let source = parsed.into_wire(uses, &public_strings, cancel)?;
    let content = root.observe(SourceUseRole::Payload)?;
    let document = uses.value(
        SourceValue::Present(ShopInventoryDocument { source, fields }),
        std::slice::from_ref(&content),
        true,
    );
    let (list, item_outcome) = match items {
        Some(ItemList::Items { guids, dropped }) if dropped > 0 => (
            ItemList::Items { guids, dropped },
            Outcome::malformed("Inventory[].ID.ID[]"),
        ),
        Some(list) => (list, Outcome::projected()),
        None => (
            unreadable(path, &compatibility::why_unparsed(read.bytes())),
            Outcome::malformed("Collection.Inventory"),
        ),
    };
    Ok(ReadDocument {
        document,
        list,
        file_outcome: Outcome::projected(),
        item_outcome,
        content: Some(content),
    })
}

fn empty_source(path: &str, uses: &SourceUses, content: SourceUse) -> ReadDocument {
    ReadDocument {
        document: uses.value(SourceValue::Absent, std::slice::from_ref(&content), true),
        list: ItemList::EmptySource(format!("{path}: the file is empty")),
        file_outcome: Outcome::projected(),
        item_outcome: Outcome::projected(),
        content: Some(content),
    }
}

fn failed(
    state: SourceValue<ShopInventoryDocument>,
    path: &str,
    reason: &dyn std::fmt::Display,
    uses: &SourceUses,
    outcome: Outcome,
) -> ReadDocument {
    ReadDocument {
        document: uses.value(state, &[], false),
        list: unreadable(path, reason),
        file_outcome: outcome.clone(),
        item_outcome: outcome,
        content: None,
    }
}

fn unreadable(path: &str, reason: &dyn std::fmt::Display) -> ItemList {
    ItemList::Unreadable(format!("{path}: {reason}"))
}
