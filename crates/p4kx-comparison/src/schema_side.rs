// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, HashMap};

use p4kx_dataforge::{DataForge, DataForgeError, Property, Struct};

use crate::PropertyShape;

/// The definitions of one DataForge, keyed by name.
#[derive(Debug, Default)]
pub(crate) struct SideSchema<'f> {
    pub(crate) structs: BTreeMap<&'f str, StructDef<'f>>,
    pub(crate) enums: BTreeMap<&'f str, EnumDef<'f>>,
    pub(crate) unreadable: u64,
}

/// A struct with the properties it declares.
#[derive(Debug, Default)]
pub(crate) struct StructDef<'f> {
    pub(crate) parent: Option<&'f str>,
    pub(crate) properties: BTreeMap<&'f str, PropertyShape>,
    pub(crate) enums: Vec<&'f str>,
    pub(crate) records: u64,
}

/// An enum with its option names and the Records that carry it.
#[derive(Debug, Default)]
pub(crate) struct EnumDef<'f> {
    pub(crate) options: BTreeSet<&'f str>,
    pub(crate) records: u64,
}

impl<'f> SideSchema<'f> {
    /// Reads the structs and enums of `forge`. The first definition of a name wins; a definition
    /// whose name cannot be read, a later definition of a name and a parent, property, option or
    /// Record whose name or struct cannot be read are counted in `unreadable` and left out.
    pub(crate) fn read(forge: &'f DataForge<'f>) -> Self {
        let mut side = Self::default();
        for structure in forge.structs() {
            let Ok(name) = structure.name() else {
                side.unreadable += 1;
                continue;
            };
            if side.structs.contains_key(name) {
                side.unreadable += 1;
                continue;
            }
            let def = struct_def(&structure, &mut side.unreadable);
            side.structs.insert(name, def);
        }
        for enumeration in forge.enums() {
            let Ok(name) = enumeration.name() else {
                side.unreadable += 1;
                continue;
            };
            if side.enums.contains_key(name) {
                side.unreadable += 1;
                continue;
            }
            let mut def = EnumDef::default();
            for option in enumeration.options() {
                match option {
                    Ok(option) => {
                        def.options.insert(option);
                    }
                    Err(_) => side.unreadable += 1,
                }
            }
            side.enums.insert(name, def);
        }
        side.count_records(forge);
        side
    }

    /// Counts the Records by the struct they are instances of: a struct gets the Records of
    /// itself and of the structs inheriting from it, an enum those of the structs that declare or
    /// inherit a property of it, each Record once.
    fn count_records(&mut self, forge: &'f DataForge<'f>) {
        let mut own: HashMap<&'f str, u64> = HashMap::new();
        for record in forge.records() {
            match record.struct_type().ok().and_then(|s| s.name().ok()) {
                Some(name) => {
                    let count = own.entry(name).or_default();
                    *count = count.saturating_add(1);
                }
                None => self.unreadable += 1,
            }
        }
        let mut rolled: HashMap<&str, u64> = HashMap::with_capacity(own.len());
        let mut enum_records: HashMap<&str, u64> = HashMap::new();
        for (&name, &count) in &own {
            let mut enums: Vec<&str> = Vec::new();
            for member in self.chain(name) {
                let total = rolled.entry(member).or_default();
                *total = total.saturating_add(count);
                let declared = self.structs.get(member).map_or(&[][..], |def| &def.enums);
                for &enumeration in declared {
                    if !enums.contains(&enumeration) {
                        enums.push(enumeration);
                    }
                }
            }
            for enumeration in enums {
                let total = enum_records.entry(enumeration).or_default();
                *total = total.saturating_add(count);
            }
        }
        for (name, def) in &mut self.structs {
            def.records = rolled.get(name).copied().unwrap_or_default();
        }
        for (name, def) in &mut self.enums {
            def.records = enum_records.get(name).copied().unwrap_or_default();
        }
    }

    /// Returns `name` and its ancestors, nearest first, stopping at a repeated struct.
    fn chain(&self, name: &'f str) -> Vec<&'f str> {
        let mut chain = vec![name];
        let mut seen = BTreeSet::from([name]);
        let mut at = self.structs.get(name).and_then(|def| def.parent);
        while let Some(parent) = at
            && seen.insert(parent)
        {
            chain.push(parent);
            at = self.structs.get(parent).and_then(|def| def.parent);
        }
        chain
    }
}

fn struct_def<'f>(structure: &Struct<'f>, unreadable: &mut u64) -> StructDef<'f> {
    let mut def = StructDef::default();
    if let Some(parent) = structure.parent() {
        match parent.name() {
            Ok(parent) => def.parent = Some(parent),
            Err(_) => *unreadable += 1,
        }
    }
    for property in structure.properties() {
        let Ok(name) = property.name() else {
            *unreadable += 1;
            continue;
        };
        if def.properties.contains_key(name) {
            *unreadable += 1;
            continue;
        }
        if let Some(enumeration) = property.target_enum().and_then(|e| e.name().ok())
            && !def.enums.contains(&enumeration)
        {
            def.enums.push(enumeration);
        }
        def.properties.insert(name, shape(&property));
    }
    def
}

fn shape(property: &Property<'_>) -> PropertyShape {
    let data_type = match property.data_type() {
        Ok(data_type) => data_type.to_string(),
        Err(DataForgeError::UnknownDataType { code }) => format!("Unknown({code:#06x})"),
        Err(_) => "Unknown".to_owned(),
    };
    let target = property
        .target_struct()
        .and_then(|structure| structure.name().ok())
        .or_else(|| property.target_enum().and_then(|e| e.name().ok()))
        .map(str::to_owned);
    PropertyShape {
        data_type,
        conversion: property.conversion(),
        target,
    }
}
