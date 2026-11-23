// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::Args;

use crate::cli::QueryFormat;

/// Arguments of `p4kx dcb fields`.
#[derive(Debug, Args)]
pub(crate) struct DcbFieldsArgs {
    /// Path of the Data.p4k Archive, or of a DataForge file such as Game2.dcb (recognised by its
    /// content, not by its name).
    pub(crate) archive: PathBuf,
    /// The Records to count: a type name, e.g. `EntityClassDefinition`, which selects the Records
    /// of that type and of the types deriving from it, or a Query, defined in docs/query.md, whose
    /// `from`, `at`, `referencing` and `where` clauses select Records and whose `select` clause is
    /// ignored, e.g. `from EntityClassDefinition at "*/spaceships/*"`. Without it every Record is
    /// counted.
    pub(crate) selection: Option<String>,
    /// Lists only the field paths that match, with the rules of `p4kx search`: a glob with `*` or
    /// `?`, otherwise text anywhere in the path; ignores case. Paths are written with `/` between
    /// the field names, e.g. `Components/SCItemWeaponComponentParams/*`.
    #[arg(long, value_name = "GLOB")]
    pub(crate) path: Option<String>,
    /// Output format. Defaults to table, or JSON with --json. JSON includes per-field origins.
    #[arg(long, value_enum)]
    pub(crate) format: Option<QueryFormat>,
    /// With --format csv: writes the exact values, without the byte order mark and without the
    /// leading ' that keeps a spreadsheet from running a text as a formula.
    #[arg(long)]
    pub(crate) csv_raw: bool,
}
