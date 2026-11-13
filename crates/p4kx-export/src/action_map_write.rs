// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::io::{self, Write};

use p4kx_extraction::{CSV_BYTE_ORDER_MARK, Language, TextSet, decode_line_breaks, write_csv_cell};
use serde::Serialize;

use crate::action_map::{Action, ActionMaps, Binding, Device};
use crate::action_map_origin::{ActionMapOrigins, CsvColumn, CsvRow, Recorder};
use crate::action_map_profile::ActionMapProfile;

/// The file format of an action map export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActionMapFormat {
    /// One JSON object: the `language` and the `action_maps` with their actions and bindings.
    Json,
    /// CSV with a UTF-8 byte order mark and CRLF line ends, one row per action; a text cell that
    /// a spreadsheet would run as a formula starts with `'`.
    Csv,
    /// As [`Csv`](Self::Csv) with the exact texts and no `'`; the byte order mark stays.
    CsvRaw,
}

/// What an action map export wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActionMapCounts {
    /// The action maps written.
    pub action_maps: usize,
    /// The actions written.
    pub actions: usize,
    /// The labels and map labels whose key has no text in the language.
    pub unresolved_labels: usize,
}

/// Writes the action maps with the labels in `language`.
///
/// A label is the text of its localisation key with `\n` decoded to a line feed; a key without a
/// text in `language` leaves the label empty in CSV and `null` in JSON. The CSV has the columns
/// `action_map`, `action_map_label`, `action`, `label`, `activation_mode`, `keyboard`, `mouse`,
/// `gamepad` and `joystick`; a device cell joins its bindings with ` | `, and a binding whose
/// activation mode differs from the action's carries it in parentheses. The JSON states every
/// binding with its device, input and activation mode.
///
/// # Errors
///
/// Returns the [`io::Error`] of the first failed write.
pub fn write_action_maps(
    maps: &ActionMaps,
    texts: &TextSet<'_>,
    language: Language,
    format: ActionMapFormat,
    out: &mut impl Write,
) -> io::Result<ActionMapCounts> {
    write(maps, texts, language, format, out, None)
}

/// An indexed language Entry and its actual action-label source.
#[derive(Debug)]
pub struct ActionMapLabels<'entry> {
    entry: &'entry p4kx_extraction::LocalizationEntry,
    texts: TextSet<'entry>,
}

impl<'entry> ActionMapLabels<'entry> {
    /// Indexes the original language Entry.
    ///
    /// # Errors
    /// Returns an error if the Entry cannot be indexed.
    pub fn new(
        entry: &'entry p4kx_extraction::LocalizationEntry,
    ) -> Result<Self, p4kx_extraction::LocalizationError> {
        Ok(Self {
            entry,
            texts: TextSet::new([(entry.language(), entry.parse()?)]),
        })
    }

    pub(crate) const fn entry(&self) -> &'entry p4kx_extraction::LocalizationEntry {
        self.entry
    }

    /// Returns the index for language diagnostics.
    #[must_use]
    pub const fn texts(&self) -> &TextSet<'entry> {
        &self.texts
    }
}

/// Writes the unchanged artifact and retains its exact per-field source evidence.
///
/// # Errors
/// Returns an output or evidence error.
pub fn write_action_maps_with_origins(
    profile: &ActionMapProfile,
    labels: &ActionMapLabels<'_>,
    format: ActionMapFormat,
    out: &mut dyn Write,
) -> io::Result<(ActionMapCounts, ActionMapOrigins)> {
    let mut origins = Recorder::new(profile, labels.entry, &labels.texts)?;
    let counts = write(
        profile.maps(),
        &labels.texts,
        labels.entry.language(),
        format,
        out,
        Some(&mut origins),
    )?;
    Ok((counts, origins.finish()?))
}

fn write(
    maps: &ActionMaps,
    texts: &TextSet<'_>,
    language: Language,
    format: ActionMapFormat,
    mut out: &mut dyn Write,
    mut origins: Option<&mut Recorder<'_>>,
) -> io::Result<ActionMapCounts> {
    let labels = Labels {
        texts,
        language,
        unresolved: std::cell::Cell::new(0),
    };
    match format {
        ActionMapFormat::Json => {
            write_json(maps, &labels, &mut out)?;
            if let Some(origins) = &mut origins {
                origins.json()?;
            }
        }
        ActionMapFormat::Csv => write_csv(maps, &labels, out, true, origins)?,
        ActionMapFormat::CsvRaw => write_csv(maps, &labels, out, false, origins)?,
    }
    Ok(ActionMapCounts {
        action_maps: maps.maps.len(),
        actions: maps.action_count(),
        unresolved_labels: labels.unresolved.get(),
    })
}

struct Labels<'a, 't> {
    texts: &'a TextSet<'t>,
    language: Language,
    unresolved: std::cell::Cell<usize>,
}

impl Labels<'_, '_> {
    fn resolve(&self, key: Option<&str>) -> Option<Cow<'_, str>> {
        let key = key?;
        let found = self
            .texts
            .lookup(key, self.language, None)
            .text()
            .map(decode_line_breaks);
        if found.is_none() {
            self.unresolved.set(self.unresolved.get() + 1);
        }
        found
    }
}

#[derive(Serialize)]
struct JsonFile<'a> {
    language: &'static str,
    action_maps: Vec<JsonMap<'a>>,
}

#[derive(Serialize)]
struct JsonMap<'a> {
    name: &'a str,
    label_key: Option<&'a str>,
    label: Option<Cow<'a, str>>,
    actions: Vec<JsonAction<'a>>,
}

#[derive(Serialize)]
struct JsonAction<'a> {
    name: &'a str,
    label_key: Option<&'a str>,
    label: Option<Cow<'a, str>>,
    activation_mode: Option<&'a str>,
    bindings: Vec<JsonBinding<'a>>,
}

#[derive(Serialize)]
struct JsonBinding<'a> {
    device: &'static str,
    input: &'a str,
    activation_mode: Option<&'a str>,
}

fn write_json(maps: &ActionMaps, labels: &Labels<'_, '_>, out: &mut impl Write) -> io::Result<()> {
    let file = JsonFile {
        language: labels.language.code(),
        action_maps: maps
            .maps
            .iter()
            .map(|map| JsonMap {
                name: &map.name,
                label_key: map.label_key.as_deref(),
                label: labels.resolve(map.label_key.as_deref()),
                actions: map
                    .actions
                    .iter()
                    .map(|action| JsonAction {
                        name: &action.name,
                        label_key: action.label_key.as_deref(),
                        label: labels.resolve(action.label_key.as_deref()),
                        activation_mode: action.activation_mode.as_deref(),
                        bindings: action
                            .bindings
                            .iter()
                            .map(|binding| JsonBinding {
                                device: binding.device.name(),
                                input: &binding.input,
                                activation_mode: binding.activation_mode.as_deref(),
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    };
    serde_json::to_writer(&mut *out, &file)?;
    out.write_all(b"\n")
}

fn write_csv(
    maps: &ActionMaps,
    labels: &Labels<'_, '_>,
    out: &mut dyn Write,
    neutralise: bool,
    mut origins: Option<&mut Recorder<'_>>,
) -> io::Result<()> {
    let mut out = Counted { out, position: 0 };
    write_csv_header(&mut out)?;
    let mut cell = String::new();
    let mut index = 0;
    for (map_index, map) in maps.maps.iter().enumerate() {
        let map_label = labels.resolve(map.label_key.as_deref());
        for (action_index, action) in map.actions.iter().enumerate() {
            let row = CsvRow {
                map: map_index,
                action: action_index,
                index,
            };
            let label = labels.resolve(action.label_key.as_deref());
            let leading = [
                (map.name.as_str(), CsvColumn::Map),
                (
                    map_label.as_deref().unwrap_or_default(),
                    CsvColumn::MapLabel,
                ),
                (action.name.as_str(), CsvColumn::Action),
                (label.as_deref().unwrap_or_default(), CsvColumn::ActionLabel),
                (
                    action.activation_mode.as_deref().unwrap_or_default(),
                    CsvColumn::ActivationMode,
                ),
            ];
            for (position, (text, column)) in leading.into_iter().enumerate() {
                if position > 0 {
                    out.write_all(b",")?;
                }
                write_cell(&mut out, text, neutralise, &mut origins, &row, column)?;
            }
            for device in Device::ALL {
                out.write_all(b",")?;
                cell.clear();
                for binding in action
                    .bindings
                    .iter()
                    .filter(|binding| binding.device == device)
                {
                    if !cell.is_empty() {
                        cell.push_str(" | ");
                    }
                    push_binding(&mut cell, binding, action);
                }
                write_cell(
                    &mut out,
                    &cell,
                    neutralise,
                    &mut origins,
                    &row,
                    CsvColumn::Device(device),
                )?;
            }
            out.write_all(b"\r\n")?;
            index += 1;
        }
    }
    Ok(())
}

fn write_csv_header(out: &mut impl Write) -> io::Result<()> {
    out.write_all(CSV_BYTE_ORDER_MARK)?;
    out.write_all(b"action_map,action_map_label,action,label,activation_mode")?;
    for device in Device::ALL {
        out.write_all(b",")?;
        out.write_all(device.name().as_bytes())?;
    }
    out.write_all(b"\r\n")
}

fn write_cell(
    out: &mut Counted<'_>,
    text: &str,
    neutralise: bool,
    origins: &mut Option<&mut Recorder<'_>>,
    row: &CsvRow,
    column: CsvColumn,
) -> io::Result<()> {
    let start = out.position;
    write_csv_cell(out, text, neutralise)?;
    if let Some(origins) = origins {
        origins.csv(row, column, start..out.position)?;
    }
    Ok(())
}

struct Counted<'output> {
    out: &'output mut dyn Write,
    position: u64,
}

impl Write for Counted<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.out.write(bytes)?;
        self.position = self
            .position
            .checked_add(written as u64)
            .ok_or_else(|| io::Error::other("action-map output offset exceeds u64"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

fn push_binding(cell: &mut String, binding: &Binding, action: &Action) {
    cell.push_str(&binding.input);
    if let Some(mode) = &binding.activation_mode
        && binding.activation_mode != action.activation_mode
    {
        cell.push_str(" (");
        cell.push_str(mode);
        cell.push(')');
    }
}
