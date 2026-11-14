// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Guid};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;

use super::contract::{
    self, QualityDistributionDefinition, QualityOwner, QualityQuantizationDefinition,
};
use super::eligibility::Eligibility;
use super::evidence::Evidence;
use super::inverse;
use super::link_index::Links;
use super::resource_links::ResourceLinks;
use super::rows::QualityRows;

pub(crate) enum Input<'source> {
    Distribution {
        definition: Box<QualityDistributionDefinition<'source>>,
        retention: Retention,
    },
    Quantization {
        definition: Box<QualityQuantizationDefinition<'source>>,
        eligibility: Eligibility,
        retention: Retention,
    },
    Resource(ResourceLinks),
    UnavailableQuantization {
        guid: Guid,
        record_index: u32,
        evidence: Evidence,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum Retention {
    Selected,
    Dependency,
    Leftover,
}

#[derive(Default)]
pub(crate) struct Step<'source> {
    pub(crate) inventory: Evidence,
    pub(crate) input: Option<Input<'source>>,
}

pub(super) struct Quantizer<'source> {
    pub(super) owner: Option<QualityOwner<'source>>,
    pub(super) guid: Guid,
    pub(super) record_index: u32,
    pub(super) eligibility: Eligibility,
    pub(super) retention: Retention,
}

#[derive(Default)]
pub(crate) struct Assembly<'source> {
    rows: QualityRows<'source>,
    quantizers: Vec<Quantizer<'source>>,
    resources: Vec<ResourceLinks>,
    inventory: Evidence,
}

impl<'source> Assembly<'source> {
    pub(crate) fn add(&mut self, step: Step<'source>) -> Result<(), CatalogError> {
        self.inventory.append(&step.inventory)?;
        match step.input {
            Some(Input::Distribution {
                definition: row,
                retention,
            }) => match retention {
                Retention::Selected => self.rows.distributions.push(*row),
                Retention::Leftover => self.rows.skipped_distributions += 1,
                Retention::Dependency => {}
            },
            Some(Input::Quantization {
                definition: row,
                eligibility,
                retention,
            }) => self.quantization(*row, eligibility, retention),
            Some(Input::Resource(resource)) => self.resources.push(resource),
            Some(Input::UnavailableQuantization {
                guid,
                record_index,
                evidence,
            }) => self.quantizers.push(Quantizer {
                owner: None,
                guid,
                record_index,
                retention: Retention::Dependency,
                eligibility: Eligibility {
                    result: contract::QuantizationEligibility::NotEligible(
                        contract::QuantizationIneligibility::Unavailable,
                    ),
                    evidence,
                    admission: Evidence::default(),
                },
            }),
            None => {}
        }
        Ok(())
    }

    fn quantization(
        &mut self,
        row: QualityQuantizationDefinition<'source>,
        eligibility: Eligibility,
        retention: Retention,
    ) {
        self.quantizers.push(Quantizer {
            owner: Some(row.owner),
            guid: row.owner.guid.0,
            record_index: row.owner.record_index,
            eligibility,
            retention,
        });
        match retention {
            Retention::Selected => self.rows.quantizations.push(row),
            Retention::Leftover => self.rows.skipped_quantizations += 1,
            Retention::Dependency => {}
        }
    }

    pub(crate) fn finish(
        mut self,
        origins: &CatalogOrigins,
        cancel: Option<&CancelToken>,
    ) -> Result<QualityRows<'source>, CatalogError> {
        let links = Links::read(self.resources, self.inventory, cancel)?;
        inverse::fill(&mut self.rows, &self.quantizers, links, origins, cancel)?;
        self.rows
            .distributions
            .sort_by_cached_key(|row| (row.owner.guid.text(), row.owner.record_index));
        self.rows
            .quantizations
            .sort_by_cached_key(|row| (row.owner.guid.text(), row.owner.record_index));
        self.rows
            .inverse
            .sort_by_cached_key(|row| (row.physical_guid.text(), row.physical_index));
        Ok(self.rows)
    }
}
