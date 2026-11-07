// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_shader::{Definition, Field, PropertyFieldKind};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn first_field<'definition>(
    definition: &'definition Definition<'_>,
    kind: PropertyFieldKind,
) -> Result<Field<'definition>, Box<dyn std::error::Error>> {
    let property = definition.properties().next().ok_or("missing Property")?;
    property
        .fields_of(kind)
        .next()
        .ok_or_else(|| "missing field".into())
}
