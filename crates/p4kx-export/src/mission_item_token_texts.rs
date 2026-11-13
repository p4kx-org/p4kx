// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use super::evidence::{Consulted, Decisions, Fact, Fields, MissionKey};
use super::item_choice::MissionItemGap;
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::catalog_v3::tag_catalog::{CapturedTags, TagNameSelector};

#[derive(Debug)]
struct Variant<'source> {
    tag: Option<TagNameSelector>,
    key: MissionKey<'source>,
}

/// An original item's eligible authored locale variants and their selection evidence.
#[derive(Debug)]
pub(crate) struct MissionItemTexts<'source> {
    variants: Vec<Variant<'source>>,
    decisions: Decisions,
}

impl<'source> MissionItemTexts<'source> {
    pub(crate) fn read(
        original: &OriginalFlowFields<'_, '_, 'source>,
        origins: &CatalogOrigins,
        tags: &CapturedTags<'_, 'source>,
    ) -> Result<Self, CatalogError> {
        let fields = Fields::new(original, origins)?;
        let mut source = Self {
            variants: Vec::new(),
            decisions: fields.decisions(),
        };
        let Some(variants) = super::child(&fields, None, "stringVariants", &mut source.decisions)?
        else {
            return Ok(source);
        };
        if let Some(list) = fields.field(Some(variants), "variants", &mut source.decisions)? {
            for variant in fields.items(list)? {
                source.read_variant(&fields, variant, tags)?;
            }
        }
        Ok(source)
    }

    fn read_variant<'fields, 'scope, 'read>(
        &mut self,
        fields: &Fields<'fields, 'scope, 'read, 'source>,
        variant: Fact<'fields, 'scope, 'read, 'source>,
        tags: &CapturedTags<'_, 'source>,
    ) -> Result<(), CatalogError> {
        if !matches!(variant.value(), Value::Struct(_)) {
            return fields.record(variant, OriginalValuePart::Value, &mut self.decisions);
        }
        let tag = fields
            .field(Some(variant), "tag", &mut self.decisions)?
            .filter(|tag| {
                matches!(tag.value(), Value::Reference(target) if target.location().is_none())
            });
        let Some(key) = fields
            .field(Some(variant), "string", &mut self.decisions)?
            .filter(|fact| matches!(fact.value(), Value::Locale(_)))
            .map(|fact| fields.key(fact))
            .transpose()?
            .flatten()
        else {
            return Ok(());
        };
        let tag = tag
            .map(|tag| tags.name_selector(&tag.reference_observation()?))
            .transpose()?;
        self.variants
            .try_reserve(1)
            .map_err(CatalogError::MissionTextAllocation)?;
        self.variants.push(Variant { tag, key });
        Ok(())
    }

    pub(crate) fn select<'read>(
        &'read self,
        name: Option<&str>,
        origins: &CatalogOrigins,
        evidence: &mut Consulted<'read>,
    ) -> Result<Result<&'read MissionKey<'source>, MissionItemGap>, CatalogError> {
        evidence.all(&self.decisions, origins)?;
        if self.decisions.unreadable() {
            return Ok(Err(MissionItemGap::MissingVariant));
        }
        let mut selected = None;
        for candidate in &self.variants {
            if let Some(name) = name {
                let Some(tag) = &candidate.tag else {
                    continue;
                };
                if !tag.matches(name, origins, evidence.uses_mut())? {
                    continue;
                }
            }
            if selected.replace(&candidate.key).is_some() {
                return Ok(Err(MissionItemGap::AmbiguousVariants));
            }
        }
        Ok(selected.ok_or(MissionItemGap::MissingVariant))
    }
}
