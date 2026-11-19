// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

macro_rules! source_getters {
    ($($(#[$doc:meta])+ $field:ident : $value:ty,)+) => {
        $(
            $(#[$doc])+
            #[must_use]
            pub const fn $field(&self) -> &$value {
                &self.$field
            }
        )+
    };
}

pub(in crate::catalog_v3) use source_getters;
