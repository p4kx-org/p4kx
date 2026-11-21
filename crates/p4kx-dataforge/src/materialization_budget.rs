// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::DataForgeError;

/// Admits Field and Array-element Value slots before their output storage is reserved.
#[derive(Debug, Default)]
pub(super) struct MaterializationBudget {
    limit: Option<u64>,
    admitted: u64,
}

impl MaterializationBudget {
    pub(super) const fn new(limit: Option<u64>) -> Self {
        Self { limit, admitted: 0 }
    }

    fn admit(&mut self, values: u64) -> Result<(), DataForgeError> {
        let required = self
            .admitted
            .checked_add(values)
            .ok_or(DataForgeError::TooLarge {
                part: "Record materialized values",
            })?;
        if let Some(limit) = self.limit
            && required > limit
        {
            return Err(DataForgeError::MaterializationBudgetExceeded { limit, required });
        }
        self.admitted = required;
        Ok(())
    }

    pub(super) fn reserve<T>(
        &mut self,
        values: u64,
        part: &'static str,
    ) -> Result<Vec<T>, DataForgeError> {
        self.admit(values)?;
        let count = usize::try_from(values).map_err(|_| DataForgeError::TooLarge { part })?;
        if count
            .checked_mul(size_of::<T>())
            .is_none_or(|bytes| bytes > isize::MAX.unsigned_abs())
        {
            return Err(DataForgeError::TooLarge { part });
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(count)
            .map_err(|_| DataForgeError::MaterializationAllocationRefused { part, values })?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::MaterializationBudget;
    use crate::{DataForgeError, Value};

    #[test]
    fn admission_precedes_a_huge_output_capacity_request() {
        let mut budget = MaterializationBudget::new(Some(1));
        let root: Vec<Value<'_>> = budget
            .reserve(1, "Record fields")
            .expect("one admitted root field");
        assert!(
            root.is_empty() && root.capacity() >= 1,
            "one genuine output slot is reserved"
        );
        assert!(
            matches!(
                budget.reserve::<Value<'_>>(u64::from(u32::MAX), "Record array"),
                Err(DataForgeError::MaterializationBudgetExceeded {
                    limit: 1,
                    required: 4_294_967_296
                })
            ),
            "budget refusal must happen before any huge Array capacity request"
        );
        assert_eq!(
            budget.admitted, 1,
            "refused slots do not become admitted output"
        );
    }

    #[test]
    fn platform_capacity_is_separate_from_an_optional_budget() {
        let mut budget = MaterializationBudget::default();
        let values = u64::try_from(isize::MAX.unsigned_abs() / size_of::<u64>())
            .expect("the platform output capacity fits u64")
            + 1;
        assert!(
            matches!(
                budget.reserve::<u64>(values, "Record array"),
                Err(DataForgeError::TooLarge {
                    part: "Record array"
                })
            ),
            "an unaddressable vector remains a platform error without a policy quota"
        );
        let empty: Vec<Value<'_>> = MaterializationBudget::new(Some(0))
            .reserve(0, "Record fields")
            .expect("zero slots need no allowance");
        assert!(
            empty.is_empty(),
            "a genuinely empty output is admitted with zero allowance"
        );
    }
}
