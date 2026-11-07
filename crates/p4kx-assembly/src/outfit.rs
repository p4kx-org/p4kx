// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Record, TagTree};

use crate::outfit_read::TagSpec;
use crate::resolver::LoadoutResolver;

/// One outfit of an Entity's default Loadout: its name tags, the XML Loadouts it adds and whether
/// it is worn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outfit {
    tags: Vec<String>,
    files: Vec<String>,
    applied: bool,
}

impl Outfit {
    /// Returns the tag paths of the outfit's name tags; empty when it has none.
    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Returns the paths of the XML Loadouts the outfit adds, in order.
    #[must_use]
    pub fn files(&self) -> &[String] {
        &self.files
    }

    /// Returns whether the outfit is worn: every outfit without name tags is; when all outfits
    /// have name tags, the first one is. Only hostile data, with more than 16 layers in all,
    /// cuts the last layers off.
    #[must_use]
    pub const fn is_applied(&self) -> bool {
        self.applied
    }
}

/// The outfits of an Entity whose default Loadout is an outfit (`SItemPortLoadoutOutfitParams`):
/// the base Loadout and every outfit, each marked as worn or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutfitReport {
    base_files: Vec<String>,
    outfits: Vec<Outfit>,
}

impl OutfitReport {
    /// Returns the paths of the XML Loadouts of the base Loadout, in order.
    #[must_use]
    pub fn base_files(&self) -> &[String] {
        &self.base_files
    }

    /// Returns the outfits in the order the Record lists them.
    #[must_use]
    pub fn outfits(&self) -> &[Outfit] {
        &self.outfits
    }
}

impl<'f> LoadoutResolver<'f> {
    /// Returns the base Loadout and the outfits of `entity`, or `None` when its default Loadout
    /// is not an outfit or the Record cannot be read.
    pub fn outfits(&mut self, entity: &Record<'f>) -> Option<OutfitReport> {
        let spec = self.spec_record(entity);
        let info = spec.as_ref().as_ref().ok()?.outfit.as_ref()?;
        let name = |tag: &TagSpec| match tag {
            TagSpec::Name(name) => name.clone(),
            TagSpec::Guid(guid) => self
                .tag_tree
                .get_or_init(|| TagTree::build(self.entities.forge()))
                .path_of(*guid)
                .map_or_else(|| guid.to_string(), str::to_owned),
        };
        Some(OutfitReport {
            base_files: info.base_files.clone(),
            outfits: info
                .outfits
                .iter()
                .map(|outfit| Outfit {
                    tags: outfit.tags.iter().map(&name).collect(),
                    files: outfit.files.clone(),
                    applied: outfit.applied,
                })
                .collect(),
        })
    }
}
