// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Maximum retained decision-input observations per optional proof group.
pub(crate) const MAX_DECISION_INPUTS: usize = 4096;
/// Maximum retained variable metadata per optional decision proof group.
pub(crate) const MAX_DECISION_INPUT_BYTES: usize = 1024 * 1024;
