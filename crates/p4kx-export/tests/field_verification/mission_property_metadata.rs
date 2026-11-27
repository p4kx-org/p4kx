// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::resource_declarations::Scope;
use super::resource_metadata;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn mission_property_owned_type_declarations_only() -> super::Result<()> {
    resource_metadata::run(Scope::MissionProperties)
}
