// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{self, Write};
use std::ops::Range;

use anyhow::{Context, Result};
use p4kx_export::{
    Language, LocalizationTableDatum, TableFormat, write_observed_localization_table,
};
use p4kx_extraction::{
    Archive, DecimalU64, FieldOrigins, FieldPath, LocalizationSet, OriginArtifactBinding,
    OriginCompanionSpool, OriginOutputAddress, SourceInventory, SourceLocation, SourceUse,
    SourceUseRole, SourceUses, StagedOriginCompanion, TextSet,
};

use super::LocaleScope;

pub(super) struct TableOrigins<'entry> {
    archive: &'entry Archive,
    set: &'entry LocalizationSet,
    texts: &'entry TextSet<'entry>,
    columns: &'entry [Language],
    format: TableFormat,
    automatic_columns: bool,
}

impl<'entry> TableOrigins<'entry> {
    pub(super) const fn new(
        archive: &'entry Archive,
        set: &'entry LocalizationSet,
        texts: &'entry TextSet<'entry>,
        columns: &'entry [Language],
        format: TableFormat,
        automatic_columns: bool,
    ) -> Self {
        Self {
            archive,
            set,
            texts,
            columns,
            format,
            automatic_columns,
        }
    }

    pub(super) fn render(&self, out: &mut dyn Write) -> io::Result<TablePublication> {
        let spool = OriginCompanionSpool::new()?;
        let locales = self.locales(spool.sources()).map_err(io::Error::other)?;
        let mut capture = Capture {
            table: self,
            locales,
            spool,
            current_key: None,
        };
        let keys = write_observed_localization_table(
            self.texts,
            self.columns,
            self.format,
            out,
            |datum, bytes| capture.field(datum, bytes).map_err(io::Error::other),
        )?;
        Ok(TablePublication {
            keys,
            companion: RefCell::new(capture.spool.finish()?),
        })
    }

    pub(super) fn summary_origins(&self) -> Result<(FieldOrigins, SourceInventory)> {
        let uses = SourceUses::with_compact_use_refs();
        let mut fields = FieldOrigins::new();
        for field in ["/archive", "/out", "/format", "/originReceipt"] {
            fields.insert(FieldPath::parse(field)?, uses.origin(&[], true))?;
        }
        let mut complete = true;
        let mut key_inputs = Vec::new();
        for locale in self.locales(&uses)? {
            let decoded_len = u64::try_from(locale.entry.text().text().len())?;
            key_inputs.push(locale.scope.at(
                SourceLocation::Subresource {
                    namespace: "globalIniDecodedText".to_owned(),
                    indices: vec![0, decoded_len],
                    field: "keySurvey".to_owned(),
                },
                SourceUseRole::Selector,
            )?);
            complete &= locale.complete;
        }
        fields.insert(
            FieldPath::parse("/keys")?,
            uses.origin(&key_inputs, complete),
        )?;
        let selection = self.language_selection(&uses)?;
        fields.insert(
            FieldPath::parse("/languages")?,
            uses.origin(&selection.inputs, selection.complete),
        )?;
        for index in 0..self.columns.len() {
            let mut path = FieldPath::parse("/languages")?;
            path.push_index(index)?;
            fields.insert(path, uses.origin(&selection.inputs, selection.complete))?;
        }
        Ok((fields, uses.freeze()?))
    }

    fn locales(&self, uses: &SourceUses) -> Result<Vec<LocaleScope<'entry>>> {
        self.set
            .entries()
            .iter()
            .map(|entry| LocaleScope::new(uses, entry))
            .collect()
    }

    fn language_selection(&self, uses: &SourceUses) -> Result<Selection> {
        if !self.automatic_columns {
            return Ok(Selection {
                inputs: Vec::new(),
                complete: true,
            });
        }
        let directory = uses.observe_root_directory_paths(self.archive)?;
        Ok(Selection {
            complete: directory.is_some(),
            inputs: directory.into_iter().collect(),
        })
    }
}

struct Selection {
    inputs: Vec<SourceUse>,
    complete: bool,
}

struct Capture<'table, 'entry> {
    table: &'table TableOrigins<'entry>,
    locales: Vec<LocaleScope<'entry>>,
    spool: OriginCompanionSpool,
    current_key: Option<KeySelector>,
}

struct KeySelector {
    inputs: Vec<SourceUse>,
    complete: bool,
}

impl Capture<'_, '_> {
    fn field(&mut self, datum: LocalizationTableDatum<'_>, bytes: Range<u64>) -> Result<()> {
        let path = datum_path(datum)?;
        let address = match self.table.format {
            TableFormat::Json => OriginOutputAddress::JsonField { path },
            _ => OriginOutputAddress::TextBytes {
                start: DecimalU64::new(bytes.start),
                end_exclusive: DecimalU64::new(bytes.end),
                field: path,
            },
        };
        match datum {
            LocalizationTableDatum::KeyHeader => self.spool.push(&address, &[], true)?,
            LocalizationTableDatum::Language { .. } => {
                let selection = self.table.language_selection(self.spool.sources())?;
                self.spool
                    .push(&address, &selection.inputs, selection.complete)?;
            }
            LocalizationTableDatum::Key { key, .. } => {
                self.spool.finish_batch()?;
                self.key(&address, key, bytes.end - bytes.start)?;
            }
            LocalizationTableDatum::Text {
                key,
                language,
                stored,
                ..
            } => {
                self.text(&address, key, language, stored, bytes.end - bytes.start)?;
            }
            _ => anyhow::bail!("unhandled localization table datum"),
        }
        Ok(())
    }

    fn key(&mut self, address: &OriginOutputAddress, key: &str, written: u64) -> Result<()> {
        let winner = self
            .table
            .texts
            .key_language(key)
            .context("original localization key")?;
        let mut inputs = Vec::new();
        let mut complete = true;
        for locale in &self.locales {
            complete &= locale.complete;
            if locale.entry.language() == winner {
                let location = locale.decoded_location(key, "key")?;
                let mut selectors = inputs.clone();
                selectors.push(locale.scope.at(location.clone(), SourceUseRole::Selector)?);
                inputs.push(locale.scope.at(location.clone(), SourceUseRole::Payload)?);
                if locale.decoded() || csv_changes(self.table.format, key, written) {
                    inputs.push(locale.scope.at(location, SourceUseRole::Conversion)?);
                }
                self.current_key = Some(KeySelector {
                    inputs: selectors,
                    complete,
                });
                break;
            }
            inputs.push(
                locale
                    .scope
                    .at(locale.location(key, None)?, SourceUseRole::Selector)?,
            );
        }
        self.spool.push(address, &inputs, complete)?;
        Ok(())
    }

    fn text(
        &mut self,
        address: &OriginOutputAddress,
        key: &str,
        language: Language,
        stored: Option<&str>,
        written: u64,
    ) -> Result<()> {
        let selector = self.current_key.as_ref().context("localization row key")?;
        let locale = self
            .locales
            .iter()
            .find(|locale| locale.entry.language() == language)
            .context("localization table column")?;
        let location = locale.location(key, stored)?;
        let mut inputs = selector.inputs.clone();
        inputs.push(locale.scope.at(location.clone(), SourceUseRole::Selector)?);
        if let Some(value) = stored {
            inputs.extend(locale.text_uses(location.clone(), value)?);
            if csv_changes(self.table.format, value, written) {
                inputs.push(locale.scope.at(location, SourceUseRole::Conversion)?);
            }
        }
        self.spool
            .push(address, &inputs, selector.complete && locale.complete)?;
        Ok(())
    }
}

fn datum_path(datum: LocalizationTableDatum<'_>) -> Result<FieldPath> {
    let mut path = FieldPath::root();
    match datum {
        LocalizationTableDatum::KeyHeader => path.push_field("key")?,
        LocalizationTableDatum::Language { column, .. } => {
            path.push_field("languages")?;
            path.push_index(column)?;
        }
        LocalizationTableDatum::Key { row, .. } => {
            path.push_field("entries")?;
            path.push_index(row)?;
            path.push_field("key")?;
        }
        LocalizationTableDatum::Text { row, language, .. } => {
            path.push_field("entries")?;
            path.push_index(row)?;
            path.push_field("texts")?;
            path.push_field(language.code())?;
        }
        _ => anyhow::bail!("unhandled localization table datum"),
    }
    Ok(path)
}

const fn csv_changes(format: TableFormat, value: &str, written: u64) -> bool {
    matches!(format, TableFormat::Csv | TableFormat::CsvRaw) && written != value.len() as u64
}

pub(super) struct TablePublication {
    pub(super) keys: usize,
    companion: RefCell<StagedOriginCompanion>,
}

impl TablePublication {
    pub(super) fn write(
        &self,
        writer: &mut dyn Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        self.companion
            .try_borrow_mut()
            .map_err(|_| io::Error::other("staged localization origin interrupted"))?
            .write(writer, binding)
    }
}
