// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

pub(crate) fn result<Value>(result: Result<Value, Infallible>) -> Value {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}
