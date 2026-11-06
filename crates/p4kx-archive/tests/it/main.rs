// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Integration tests of `p4kx-archive`, built as one test binary.

mod archive_key;
mod build;
mod container_limits;
mod containers;
mod decoding;
mod effective_encoding;
mod hash_block;
mod installation_v2;
mod layout_v2;
mod layout_v2_extensions;
mod listing;
mod malformed;
mod no_launcher_log;
mod overlong;
mod ranged;
mod read_only;
mod reading;
mod recovery;
mod robustness;
mod sha256;
mod verified;
