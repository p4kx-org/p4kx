// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_comparison::{
    Archive, EnumChange, KindCounts, OptionChangeKind, PropertyChange, SchemaDiff, Side,
    StructChange, compare_schema, schema_report,
};

use crate::compare::{finish, write_line, write_side};
use crate::model_text::printable;
use crate::{dataforges, json};

/// Compares the schema of both DataForges and prints the summary and the changes, or the report
/// as JSON; returns 0 without and 1 with differences.
pub(crate) fn run(old: &Archive, new: &Archive, as_json: bool) -> Result<ExitCode> {
    let (old_entry, new_entry) = dataforges::read_both(old, new)?;
    let (old_forge, new_forge) = dataforges::open_both(&old_entry, &new_entry)?;
    let diff = compare_schema(&old_forge, &new_forge);
    let printed = if as_json {
        let patches = crate::launcher_discovery::patches()?;
        json::print(&schema_report(old, new, &diff, Some(&patches)))
    } else {
        print(&diff, old, new)
    };
    finish(printed, diff.has_differences())
}

fn print(diff: &SchemaDiff, old: &Archive, new: &Archive) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    write_side(&mut out, "Old", &Side::of(old))?;
    write_side(&mut out, "New", &Side::of(new))?;
    let counts = &diff.counts;
    for (label, kinds) in [
        ("Structs", &counts.structs),
        ("Properties", &counts.properties),
        ("Enums", &counts.enums),
        ("Enum options", &counts.options),
    ] {
        write_counts(&mut out, label, kinds)?;
    }
    if diff.unreadable.old + diff.unreadable.new > 0 {
        let text = format!(
            "{} old, {} new (left out)",
            diff.unreadable.old, diff.unreadable.new
        );
        write_line(&mut out, "Unreadable", &text)?;
    }
    if !diff.structs.is_empty() {
        write_head(&mut out, "STRUCT")?;
        for change in &diff.structs {
            write_struct(&mut out, change)?;
        }
    }
    if !diff.enums.is_empty() {
        write_head(&mut out, "ENUM")?;
        for change in &diff.enums {
            write_enum(&mut out, change)?;
        }
    }
    out.flush()
}

fn write_counts(out: &mut impl Write, label: &str, counts: &KindCounts) -> io::Result<()> {
    let text = format!(
        "{} added, {} removed, {} changed",
        counts.added, counts.removed, counts.changed
    );
    write_line(out, label, &text)
}

fn write_head(out: &mut impl Write, what: &str) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "{what:<48} {:<8} {:>9} {:>9}", "CHANGE", "OLD", "NEW")
}

fn write_struct(out: &mut impl Write, change: &StructChange) -> io::Result<()> {
    writeln!(
        out,
        "{:<48} {:<8} {:>9} {:>9}",
        printable(&change.name),
        change.change,
        change.records.old,
        change.records.new
    )?;
    if let Some(parent) = &change.parent {
        writeln!(
            out,
            "  parent: {} -> {}",
            printable(parent.old.as_deref().unwrap_or("none")),
            printable(parent.new.as_deref().unwrap_or("none"))
        )?;
    }
    change
        .properties
        .iter()
        .try_for_each(|property| write_property(out, property))
}

fn write_enum(out: &mut impl Write, change: &EnumChange) -> io::Result<()> {
    writeln!(
        out,
        "{:<48} {:<8} {:>9} {:>9}",
        printable(&change.name),
        change.change,
        change.records.old,
        change.records.new
    )?;
    for option in &change.options {
        let mark = if option.change == OptionChangeKind::Added {
            '+'
        } else {
            '-'
        };
        writeln!(out, "  {mark} {}", printable(&option.name))?;
    }
    Ok(())
}

fn write_property(out: &mut impl Write, property: &PropertyChange) -> io::Result<()> {
    let describe = |shape: Option<&p4kx_comparison::PropertyShape>| {
        shape.map_or_else(String::new, |shape| {
            printable(&shape.to_string()).into_owned()
        })
    };
    let name = printable(&property.name);
    let (old, new) = (
        describe(property.old.as_ref()),
        describe(property.new.as_ref()),
    );
    if property.old.is_none() {
        writeln!(out, "  + {name}: {new}")
    } else if property.new.is_none() {
        writeln!(out, "  - {name}: {old}")
    } else {
        let aspects: Vec<String> = property.aspects.iter().map(ToString::to_string).collect();
        writeln!(out, "  ~ {name}: {old} -> {new} ({})", aspects.join(", "))
    }
}
