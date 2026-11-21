// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reading the files inside a container (`.socpak`).

use std::path::Path;

use p4kx_extraction::{Archive, Entry};

/// The largest `.soc` file that is read, 64 MiB.
pub(crate) const MAX_SOC_LEN: u64 = 64 << 20;

/// The extension of the entity file of a container.
const SOC_EXTENSION: &str = "soc";

/// Returns the `.soc` files of `container`, in stored order.
pub(crate) fn soc_entries(container: &Archive) -> impl Iterator<Item = &Entry> {
    container.entries().iter().filter(|inner| {
        Path::new(inner.path())
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case(SOC_EXTENSION))
    })
}
