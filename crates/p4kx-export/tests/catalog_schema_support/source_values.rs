// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUses, SourceValue};
use serde_json::{Value, json};

pub(super) fn present(value: Value) -> Value {
    source(SourceValue::Present(value))
}

pub(super) fn source(value: SourceValue<Value>) -> Value {
    json!(SourceUses::new().value(value, &[], false))
}
