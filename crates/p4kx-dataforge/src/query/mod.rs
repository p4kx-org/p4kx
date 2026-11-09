// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Queries over the Records of a DataForge, defined in `docs/query.md`.
//!
//! [`Query::parse`] reads the text into a [`Query`], [`Query::compile`] checks it against one
//! DataForge, and [`CompiledQuery::run`] selects the Records in parallel, keeps those that meet
//! the condition, and builds the rows page by page from the Records' instances.
//! [`QuerySchema::complete`] offers the type and field names that fit at a cursor.

mod comparison;
mod compile;
mod complete;
mod condition;
mod error;
mod expected;
mod lexer;
mod matching;
mod operand;
mod output;
mod parser;
mod path_plan;
mod predicate;
mod reader;
mod resolve;
mod run;
mod schema;
mod selection;
mod selection_control;
mod selection_error;
mod shared;
mod short_text;
mod sink;
mod span;
mod state;
mod suggest;
mod syntax;
mod walk;

pub use compile::{ColumnKind, CompiledQuery, SharedPrefix};
pub use complete::{Candidate, Completion, CompletionKind, MAX_COMPLETIONS, QuerySchema};
pub use condition::{Comparison, Condition, Literal, LiteralValue, MAX_CONDITION_DEPTH, Operator};
pub use error::{Marked, QueryError, QueryErrorKind};
pub use expected::{Expected, Found, LiteralKind};
pub use lexer::Keyword;
pub use output::{RECORD_PATH_COLUMN, ResultFormat, ResultWriter};
pub use run::{Pages, QueryRun, Row};
pub use selection_error::SelectionError;
pub use short_text::{LIST_SHOWN, write_short_text};
pub use span::Span;
pub use syntax::{Glob, Name, Path, Query, Referent, Segment};

/// The most segments a path has behind its first field: `.name`, `[n]`, `[*]`, `[TypeName]`
/// and `->name`.
pub const MAX_PATH_SEGMENTS: usize = 64;

/// The most `->` in one path, each following one Reference or weak pointer.
pub const MAX_PATH_REFERENCES: usize = 16;

/// The most array elements the columns of one row reach together; beyond them a cell is an error.
pub const MAX_PATH_VALUES: usize = 4_096;

/// The longest Query text in bytes.
pub const MAX_QUERY_LEN: usize = 65_536;
