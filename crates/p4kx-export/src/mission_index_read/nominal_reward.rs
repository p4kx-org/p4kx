// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUse, SourceValue, SourcedValue};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalReferenceObservation;
use crate::mission_facts::choice;
use crate::resource_integer::ResourceInt64;
use crate::{CatalogError, ExportAccountingError};

use super::{MissionIndex, RewardDependency, read};

impl MissionIndex<'_> {
    #[cfg(test)]
    pub(crate) fn reward(&self, guid: Guid) -> Option<i64> {
        self.nominal_rewards(guid)
            .find_map(|(_, reward)| reward.amount())
    }

    /// Selects the last usable Reward amount under the authored nominal GUID.
    pub(crate) fn nominal_reward_amount(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        reference: &OriginalReferenceObservation<'_, '_, '_, '_>,
        uses: &mut Vec<SourceUse>,
    ) -> Result<SourceValue<i64>, CatalogError> {
        self.check_template_source(input.origins())?;
        reference.caller().check_input(input)?;
        if reference.is_instance() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let guid = reference.reference().guid();
        let selector = reference.guid_selector()?;
        if selector.value() != &SourceValue::Present(guid) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        choice::extend(uses, selector.contributions().0)?;
        let mut last_physical_state = None;
        let mut unreadable = false;
        for (&key, reward) in self.nominal_rewards(guid) {
            let value = match reward {
                RewardDependency::Original(reward) => reward.retain_nominal(input, key, uses)?,
                RewardDependency::UnreadablePath(path) => {
                    path.retain_unreadable_path(input, key, uses)?
                }
                #[cfg(test)]
                RewardDependency::Accounting(_) => {
                    return Err(ExportAccountingError::UnboundHealthSubject.into());
                }
            };
            if matches!(value, SourceValue::Present(_)) {
                return Ok(value);
            }
            unreadable |= matches!(value, SourceValue::Unreadable);
            if last_physical_state.is_none() {
                last_physical_state = Some(value);
            }
        }
        if unreadable {
            return Ok(SourceValue::Unreadable);
        }
        Ok(last_physical_state.unwrap_or(SourceValue::Absent))
    }

    pub(crate) fn reward_observed(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        reference: &OriginalReferenceObservation<'_, '_, '_, '_>,
    ) -> Result<Option<SourcedValue<ResourceInt64>>, CatalogError> {
        reference.caller().check_input(input)?;
        if reference.is_instance() {
            return Ok(None);
        }
        let key = (reference.reference().guid(), reference.record_index());
        match self.rewards.get(&key) {
            Some(RewardDependency::Original(reward)) => reward.retain(input, reference).map(Some),
            Some(RewardDependency::UnreadablePath(_)) => {
                Err(ExportAccountingError::ConflictingSourceDatum.into())
            }
            #[cfg(test)]
            Some(RewardDependency::Accounting(_)) => {
                Err(ExportAccountingError::UnboundHealthSubject.into())
            }
            None => {
                let record = reference.record()?;
                input.record_context(&record)?;
                if read::reward_candidate(&record).map_err(CatalogError::DataForge)? {
                    return Err(ExportAccountingError::UnobservedFailure.into());
                }
                Ok(None)
            }
        }
    }

    fn nominal_rewards(
        &self,
        guid: Guid,
    ) -> impl Iterator<Item = (&(Guid, u32), &RewardDependency)> {
        self.rewards.range((guid, 0)..=(guid, u32::MAX)).rev()
    }
}
