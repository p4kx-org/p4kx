// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::HealthText;

/// Value-only body fields compared against an independently retained source receipt.
pub(super) type ExampleDescription = (Option<HealthText>, Option<HealthText>, Option<Guid>);
