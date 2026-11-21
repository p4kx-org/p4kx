// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::CatalogError;
use crate::catalog_read::original::OriginalFlowFields;
use crate::contract_facts::{ContractFacts, LayeredFacts};
use crate::mission_intros::Requirements;
use crate::mission_layers::Layer;
use crate::mission_results::RunTerms;

#[path = "mission_fact_choice.rs"]
pub(crate) mod choice;
#[path = "mission_layered_facts.rs"]
mod layered;
#[path = "mission_fact_source.rs"]
pub(crate) mod source;

pub(crate) use self::layered::PreparedFacts;

/// The layers of one entry, nearest first: sub contract (when the entry is one), contract, handler.
pub(crate) struct Levels<'a, 'f> {
    pub(crate) sub: Option<&'a Layer<'f>>,
    pub(crate) contract: &'a Layer<'f>,
    pub(crate) handler: &'a Layer<'f>,
}

impl<'f> Levels<'_, 'f> {
    /// Returns the layers, nearest first.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Layer<'f>> {
        self.sub.into_iter().chain([self.contract, self.handler])
    }
}

/// The run terms and completion tags shared by the rows of one contract.
#[derive(Debug)]
pub(crate) struct SharedFacts {
    terms: RunTerms,
    pub(crate) requirements: Requirements,
}

impl SharedFacts {
    /// Reads the shared facts of `contract`, a contract of `handler`.
    pub(crate) fn read<'source>(
        fields: &OriginalFlowFields<'_, '_, 'source>,
        handler: &StructValue<'source>,
        contract: &StructValue<'source>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            terms: RunTerms::read(fields, contract)?,
            requirements: Requirements::shared(fields, handler, contract)?,
        })
    }

    /// Combines one row's selected layered facts with its run terms.
    pub(crate) fn facts(&self, layered: LayeredFacts) -> ContractFacts {
        ContractFacts {
            time_to_complete: self.terms.time_to_complete.clone(),
            buy_in_amount: self.terms.buy_in_amount.clone(),
            layered,
        }
    }
}
