// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{
    DataForgeOrigin, InputScope, RecordTree, SourceEncryption, SourceInput, SourceScope,
    SourceUses, Value as Original,
};
use serde_json::{Value, json};

use super::Result;
use crate::authored_definition_witness::field;
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_item_durability::{Trace, indexed};
use crate::real_archive_law_definitions::verify_scalar;

pub(super) struct Receipt<'catalog> {
    uses: HashMap<&'catalog str, &'catalog Value>,
    scope: SourceScope,
    encrypted: Option<bool>,
}

impl<'catalog> Receipt<'catalog> {
    pub(super) fn new(catalog: &'catalog Value, origin: &DataForgeOrigin) -> Result<Self> {
        let DataForgeOrigin::Archive(entry) = origin else {
            return Err("the original DCB has no Archive identity".into());
        };
        let scope = SourceUses::new().observe(SourceInput::Archive(entry), InputScope::Payload)?;
        Ok(Self {
            uses: indexed(catalog, "/provenance/uses", "id")?,
            scope,
            encrypted: match origin.encryption() {
                SourceEncryption::Encrypted => Some(true),
                SourceEncryption::Unencrypted => Some(false),
                _ => None,
            },
        })
    }

    pub(super) fn trace<'a, 'source>(&'a self, tree: &RecordTree<'source>) -> Trace<'a, 'source> {
        Trace::of(tree, &self.uses, &self.scope, self.encrypted)
    }
}

pub(super) fn scalar(
    trace: &Trace<'_, '_>,
    row: &Value,
    name: &str,
    source: Option<&Original<'_>>,
) -> Result {
    let cell = field(row, name)?;
    let expected = match source {
        Some(Original::String(value)) => json!(value),
        _ => return verify_scalar_and_trace(trace, source, cell),
    };
    assert!(
        consumer::present(cell)? == &expected,
        "exact original {name} value"
    );
    trace.check(cell, source.ok_or("original scalar")?, "payload")
}

fn verify_scalar_and_trace(
    trace: &Trace<'_, '_>,
    source: Option<&Original<'_>>,
    cell: &Value,
) -> Result {
    verify_scalar(source, cell)?;
    if let Some(raw @ (Original::Int32(_) | Original::Bool(_))) = source {
        trace.check(cell, raw, "payload")?;
    }
    Ok(())
}
