// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_export::{CatalogSummary, Language};
use serde_json::Value;

use crate::real_archive_canonical_texts::{TextDomain, TextIndex, TextReferenceShape};

pub(super) fn count(
    catalog: &Value,
    processes: &[Value],
    summary: &CatalogSummary,
) -> Result<(usize, usize), Box<dyn Error>> {
    let texts = TextIndex::new(catalog)?;
    let (mut english, mut german) = (0, 0);
    for process in processes {
        let Some(name) = texts.reference(
            process
                .get("processName")
                .ok_or("refining process name carrier")?,
            TextDomain::GameText,
            TextReferenceShape::Required,
        )?
        else {
            continue;
        };
        for (language, count) in [
            (Language::English, &mut english),
            (Language::German, &mut german),
        ] {
            if summary.text(language).is_some_and(|info| info.available)
                && name
                    .shown(language.code())?
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
            {
                *count += 1;
            }
        }
    }
    Ok((english, german))
}
