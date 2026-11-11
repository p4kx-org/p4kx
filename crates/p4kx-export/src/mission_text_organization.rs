// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::catalog_v3::tag_catalog::{CapturedTags, TagNameSelector};

use super::evidence::{Consulted, Decisions, Fact, Fields, MissionKey};

#[derive(Debug)]
struct Variant<'source> {
    tag: TagNameSelector,
    key: MissionKey<'source>,
    prefix: usize,
}

/// An Organization's actual eligible variants with their ordered selector inputs.
#[derive(Debug)]
pub(crate) struct OrganizationTexts<'source> {
    variants: Vec<Variant<'source>>,
    decisions: Decisions,
}

impl<'source> OrganizationTexts<'source> {
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
        let Some(tag) = fields
            .field(Some(variant), "tag", &mut self.decisions)?
            .filter(|fact| {
                matches!(fact.value(), Value::Reference(target)
                if target.location().is_none())
            })
        else {
            return Ok(());
        };
        let Some(key) = fields
            .field(Some(variant), "string", &mut self.decisions)?
            .map(|fact| fields.key(fact))
            .transpose()?
            .flatten()
        else {
            return Ok(());
        };
        let tag = tags.name_selector(&tag.reference_observation()?)?;
        self.variants
            .try_reserve(1)
            .map_err(CatalogError::MissionTextAllocation)?;
        self.variants.push(Variant {
            tag,
            key,
            prefix: self.decisions.len(),
        });
        Ok(())
    }

    pub(crate) fn select<'read>(
        &'read self,
        variant: Option<&str>,
        origins: &CatalogOrigins,
        into: &mut Consulted<'read>,
    ) -> Result<Option<&'read MissionKey<'source>>, CatalogError> {
        if let Some(name) = variant {
            for candidate in &self.variants {
                if candidate.tag.matches(name, origins, into.uses_mut())? {
                    into.prefix(&self.decisions, candidate.prefix, origins)?;
                    return Ok(Some(&candidate.key));
                }
            }
            into.all(&self.decisions, origins)?;
            return Ok(None);
        }
        into.all(&self.decisions, origins)?;
        Ok(match self.variants.as_slice() {
            [variant] => Some(&variant.key),
            _ => None,
        })
    }
}
