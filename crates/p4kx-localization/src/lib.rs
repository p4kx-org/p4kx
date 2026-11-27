// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Localisation of Star Citizen: the `global.ini` file of a language and the resolution of the
//! localisation keys DataForge Records hold, e.g. `@manufacturer_NameACAS`.
//!
//! Each language's texts are the Entry `Data\Localization\<folder>\global.ini`; the folder is
//! `<language>_(<region>)` in lower case, e.g. `german_(germany)`, except plain `english`.
//! [`Language`] names the eleven languages a Build holds; [`Language::global_ini_path`] names
//! the Entry of a language's `global.ini` and [`Language::from_global_ini_path`] tells which
//! language an Entry path belongs to. [`IniText::decode`] turns the bytes in any of their
//! encodings into text, [`GlobalIni`] indexes the text without copying the texts and looks a key
//! up in constant time, ignoring ASCII case, and a [`TextSet`] resolves keys across languages
//! with a fallback language ([`Lookup`]) and lists every key.

mod error;
mod global_ini;
mod ini_text;
mod key;
mod language;
mod localization_limits;
mod lookup;
mod owned_text_set;
mod text_set;

pub use error::LocalizationError;
pub use global_ini::GlobalIni;
pub use ini_text::{IniEncoding, IniText};
pub use language::{Language, ParseLanguageError};
pub use localization_limits::LocalizationLimits;
pub use lookup::{Lookup, PLACEHOLDER_KEYS, decode_line_breaks, display_text, is_placeholder_key};
pub use owned_text_set::OwnedTextSet;
pub use text_set::TextSet;
