// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Language, LocalizationNote};

use crate::quantization_summary::QuantizationSummary;
use crate::shop_summary::ShopSummary;
use crate::store_slug_summary::StoreSlugSummary;
use crate::{
    CatalogFailure, CraftingWarning, ItemClassSummary, Kind, PlayabilitySummary, StarmapReport,
};

/// The largest catalog file the basetool accepts for upload, in bytes, compressed if it is; it
/// must match the basetool's upload limit (`docs/basetool-catalog.md`).
pub const UPLOAD_LIMIT: u64 = 64_000_000;

/// The size from which a catalog file is close to [`UPLOAD_LIMIT`], 90 % of it.
pub const UPLOAD_WARNING_SIZE: u64 = UPLOAD_LIMIT / 10 * 9;

/// The largest decompressed catalog the basetool accepts, in bytes; it must match the basetool's
/// limit (`docs/basetool-catalog.md`).
pub const DECOMPRESSED_LIMIT: u64 = 1_000_000_000;

/// The decompressed size from which a catalog is close to [`DECOMPRESSED_LIMIT`], 90 % of it.
pub const DECOMPRESSED_WARNING_SIZE: u64 = DECOMPRESSED_LIMIT / 10 * 9;

/// How a catalog's size relates to the basetool upload limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeWarning {
    /// At least [`UPLOAD_WARNING_SIZE`] bytes, but within [`UPLOAD_LIMIT`].
    NearLimit,
    /// More than [`UPLOAD_LIMIT`] bytes; the basetool rejects the upload.
    OverLimit,
}

impl SizeWarning {
    /// Returns the warning for a catalog of `size` bytes, `None` below [`UPLOAD_WARNING_SIZE`].
    #[must_use]
    pub const fn for_size(size: u64) -> Option<Self> {
        Self::against(size, UPLOAD_LIMIT, UPLOAD_WARNING_SIZE)
    }

    /// Returns the warning for a decompressed catalog of `size` bytes against
    /// [`DECOMPRESSED_LIMIT`], `None` below [`DECOMPRESSED_WARNING_SIZE`].
    #[must_use]
    pub const fn for_decompressed_size(size: u64) -> Option<Self> {
        Self::against(size, DECOMPRESSED_LIMIT, DECOMPRESSED_WARNING_SIZE)
    }

    const fn against(size: u64, limit: u64, warning: u64) -> Option<Self> {
        if size > limit {
            Some(Self::OverLimit)
        } else if size >= warning {
            Some(Self::NearLimit)
        } else {
            None
        }
    }
}

/// What a catalog holds of one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindSummary {
    /// The kind.
    pub kind: Kind,
    /// The entities written.
    pub count: usize,
    /// The selected Records left out as development leftovers.
    pub skipped: usize,
    /// Whether the kind has its full selection scope; global tags ignore the Entity Filter.
    pub complete: bool,
}

/// What a catalog holds of paints and their ship assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PaintSummary {
    /// The paints in the catalog.
    pub paints: usize,
    /// The paints that fit at least one ship.
    pub assigned: usize,
    /// The paints that fit no ship.
    pub unassigned: usize,
    /// The ships that at least one paint fits.
    pub ships: usize,
    /// The ships whose vehicle implementation or Modification patch file is present but cannot be
    /// read; they have no paints.
    pub unreadable_vehicles: usize,
}

/// What a catalog holds of loadouts. The port counts run over the whole tree of every loadout, so
/// a port that several ships carry in their trees is counted once per ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadoutSummary {
    /// The loadouts of ships.
    pub ships: usize,
    /// The loadouts of items that have ports.
    pub items: usize,
    /// The ports of all loadouts, nested ports included.
    pub ports: usize,
    /// The ports that hold an Entity, docked vehicles included.
    pub attached: usize,
    /// The ports that hold a docked vehicle; its ports are not listed.
    pub docked_vehicles: usize,
    /// The ports whose attachment cannot be resolved, e.g. a class name no Entity has.
    pub unresolved: usize,
    /// The distinct Entities held by a port that are no entry of `items` or `ships`, e.g.
    /// development leftovers or Entities outside the item folders; a consumer finds no entry for
    /// their `itemGuid`.
    pub outside: usize,
    /// The loadouts that list at least one issue.
    pub with_issues: usize,
}

/// How many variant groups one kind has and how many entities are variants of a base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VariantCounts {
    /// The groups of at least two members.
    pub groups: usize,
    /// The entities with a `variantOf`, every member of a group but its base.
    pub variants: usize,
}

/// The variant counts of the ships and of the items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VariantSummary {
    /// The ships and ground vehicles.
    pub ships: VariantCounts,
    /// The items.
    pub items: VariantCounts,
}

/// What a catalog holds of starmap objects and where the system containers place them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StarmapSummary {
    /// The starmap objects in the catalog.
    pub objects: usize,
    /// The objects with a position.
    pub placed: usize,
    /// The objects no system container places; their position is `null`.
    pub unplaced: usize,
    /// The records that Child elements place more than 1 m apart; they have no position.
    pub ambiguous: usize,
    /// The star systems whose main container the Archive holds.
    pub systems: usize,
    /// The containers that are named or listed but cannot be opened or parsed; the objects below
    /// them have no position.
    pub unreadable_containers: usize,
    /// The external containers a Child names that the Archive does not hold, a fact of the game
    /// data; the objects below them have no position.
    pub missing_containers: usize,
    /// The Child elements skipped for a malformed `pos` or `rot`, for a nesting beyond the limit
    /// or beyond the budget of one system; the objects below them have no position.
    pub malformed_children: usize,
}

/// How the localisation keys of a catalog resolved in one language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSummary {
    /// The language.
    pub language: Language,
    /// Whether the Archive holds the language's `global.ini`.
    pub available: bool,
    /// The texts whose key the language resolved.
    pub resolved: usize,
    /// The texts whose key the language lacks, or every text with a key when it is not available;
    /// their text in this language is `null`. A key whose text is empty counts here.
    pub unresolved: usize,
    /// The texts with a [placeholder key](crate::PLACEHOLDER_KEYS), the same in every language;
    /// they are neither resolved nor unresolved.
    pub placeholder: usize,
}

/// What a catalog holds: the counts per kind, how its texts resolved and its size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSummary {
    pub(crate) export_health: crate::ExportHealth,
    pub(crate) kinds: Vec<KindSummary>,
    pub(crate) texts: Vec<TextSummary>,
    pub(crate) notes: Vec<LocalizationNote>,
    pub(crate) item_classes: ItemClassSummary,
    pub(crate) paints: PaintSummary,
    pub(crate) loadouts: LoadoutSummary,
    pub(crate) starmap: StarmapSummary,
    pub(crate) starmap_report: Box<StarmapReport>,
    pub(crate) shops: Box<ShopSummary>,
    pub(crate) store_slugs: Box<StoreSlugSummary>,
    pub(crate) playability: PlayabilitySummary,
    pub(crate) variants: VariantSummary,
    pub(crate) quantizations: QuantizationSummary,
    pub(crate) failures: Vec<CatalogFailure>,
    pub(crate) warnings: Vec<CraftingWarning>,
    pub(crate) size: u64,
    pub(crate) compressed_size: Option<u64>,
    pub(crate) archive_key: crate::ArchiveKeyUse,
}

impl CatalogSummary {
    /// Returns the actual source-read and projection health of the serialized catalog.
    #[must_use]
    pub const fn export_health(&self) -> &crate::ExportHealth {
        &self.export_health
    }
    /// Returns the counts of the paints: how many the catalog holds, how many fit a ship and how
    /// many fit none.
    #[must_use]
    pub const fn paints(&self) -> PaintSummary {
        self.paints
    }

    /// Returns the counts of the loadouts: how many ships and items have one, how many ports they
    /// hold, how many ports hold an Entity and how many of those the catalog has no entry for.
    #[must_use]
    pub const fn loadouts(&self) -> LoadoutSummary {
        self.loadouts
    }

    /// Returns the counts of the starmap objects: how many the catalog holds, how many have a
    /// position, and what the reading of the system containers found.
    #[must_use]
    pub const fn starmap(&self) -> StarmapSummary {
        self.starmap
    }

    /// Returns what the reading of the system containers saw: the containers read with their
    /// Child counts, the ones that failed and why, the records placed twice and the containers no Child
    /// names.
    #[must_use]
    pub fn starmap_report(&self) -> &StarmapReport {
        &self.starmap_report
    }

    /// Returns the counts of the shops: how many the catalog holds, how the registry was read, how
    /// many shops have a kind and a link to a starmap object, and the unlinked location names.
    #[must_use]
    pub fn shops(&self) -> &ShopSummary {
        &self.shops
    }

    /// Returns how the curated store slugs resolved against the ships of the catalog and which
    /// mappings are stale.
    #[must_use]
    pub fn store_slugs(&self) -> &StoreSlugSummary {
        &self.store_slugs
    }

    /// Returns Record failures ordered by GUID, including failures that retain partial rows.
    #[must_use]
    pub fn failures(&self) -> &[CatalogFailure] {
        &self.failures
    }

    /// Returns source warnings and incomplete or ambiguous catalog relationships.
    #[must_use]
    pub fn warnings(&self) -> &[CraftingWarning] {
        &self.warnings
    }

    /// Returns whether every required source read and projection completed without a source warning.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.export_health.is_complete()
    }

    /// Returns how the item classes of the items were read.
    #[must_use]
    pub const fn item_classes(&self) -> ItemClassSummary {
        self.item_classes
    }

    /// Returns how many items and ships carry each playability value.
    #[must_use]
    pub const fn playability(&self) -> PlayabilitySummary {
        self.playability
    }

    /// Returns how many quality quantizations no `ResourceType` uses and how many links reach no
    /// quantization of the catalog.
    #[must_use]
    pub const fn quantizations(&self) -> QuantizationSummary {
        self.quantizations
    }

    /// Returns how many variant groups and variants the ships and the items have.
    #[must_use]
    pub const fn variants(&self) -> VariantSummary {
        self.variants
    }

    /// Returns how the texts resolved per language, English first.
    #[must_use]
    pub fn texts(&self) -> &[TextSummary] {
        &self.texts
    }

    /// Returns how the texts resolved in `language`.
    #[must_use]
    pub fn text(&self, language: Language) -> Option<TextSummary> {
        self.texts
            .iter()
            .copied()
            .find(|summary| summary.language == language)
    }

    /// Returns a note for every `global.ini` Entry that was read lossily or had lines skipped.
    #[must_use]
    pub fn text_notes(&self) -> &[LocalizationNote] {
        &self.notes
    }

    /// Returns the unresolved texts over every language.
    #[must_use]
    pub fn unresolved(&self) -> usize {
        self.texts.iter().map(|summary| summary.unresolved).sum()
    }

    /// Returns the counts of every kind, in the order of the catalog.
    #[must_use]
    pub fn kinds(&self) -> &[KindSummary] {
        &self.kinds
    }

    /// Returns the counts of `kind`.
    #[must_use]
    pub fn kind(&self, kind: Kind) -> Option<KindSummary> {
        self.kinds
            .iter()
            .copied()
            .find(|summary| summary.kind == kind)
    }

    /// Returns the Records left out as development leftovers over every kind that selects Records;
    /// a derived kind counts the leftovers of the kind it is derived from, so it is not added.
    #[must_use]
    pub fn skipped(&self) -> usize {
        self.kinds
            .iter()
            .filter(|summary| !summary.kind.is_derived())
            .map(|summary| summary.skipped)
            .sum()
    }

    /// Returns the size of the catalog JSON in bytes, uncompressed.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Returns the size of the compressed catalog in bytes, `None` for a plain JSON catalog.
    #[must_use]
    pub const fn compressed_size(&self) -> Option<u64> {
        self.compressed_size
    }

    /// Returns the size of the catalog file, the compressed one if it is compressed; the size the
    /// basetool checks against [`UPLOAD_LIMIT`].
    #[must_use]
    pub fn upload_size(&self) -> u64 {
        self.compressed_size.unwrap_or(self.size)
    }

    /// Returns whether the catalog file is close to or over the basetool upload limit.
    #[must_use]
    pub fn size_warning(&self) -> Option<SizeWarning> {
        SizeWarning::for_size(self.upload_size())
    }

    /// Returns whether the decompressed catalog is close to or over the basetool limit for
    /// decompressed content.
    #[must_use]
    pub const fn decompressed_warning(&self) -> Option<SizeWarning> {
        SizeWarning::for_decompressed_size(self.size)
    }
}

/// A built Basetool catalog: its JSON and its summary.
#[derive(Debug, Clone)]
pub struct BuiltCatalog {
    pub(crate) json: Vec<u8>,
    pub(crate) summary: CatalogSummary,
}

impl BuiltCatalog {
    /// Returns the catalog as UTF-8 JSON ending in a newline.
    #[must_use]
    pub fn json(&self) -> &[u8] {
        &self.json
    }

    /// Returns the summary.
    #[must_use]
    pub const fn summary(&self) -> &CatalogSummary {
        &self.summary
    }

    /// Returns the JSON and the summary.
    #[must_use]
    pub fn into_parts(self) -> (Vec<u8>, CatalogSummary) {
        (self.json, self.summary)
    }
}
