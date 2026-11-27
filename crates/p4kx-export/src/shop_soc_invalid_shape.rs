// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::shop_inventory_name;

const PROPERTY: &str = "offlineInventory";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in super::super) struct InvalidShape {
    field: Field,
    whitespace: Whitespace,
    non_ascii: bool,
    control: bool,
    folder: Folder,
    extension: Extension,
    outer_trim: OuterTrim,
}

impl InvalidShape {
    pub(super) fn of(field: &str, value: &str) -> Self {
        let trimmed = value.trim();
        let folder = folder(trimmed);
        let file = trimmed.rsplit(['/', '\\']).next().unwrap_or_default();
        Self {
            field: Field::of(field),
            whitespace: Whitespace::of(value),
            non_ascii: !value.is_ascii(),
            control: value.chars().any(char::is_control),
            folder,
            extension: Extension::of(file),
            outer_trim: OuterTrim::of(value, trimmed, folder),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Whitespace {
    None,
    Leading,
    Trailing,
    Both,
}

impl Whitespace {
    fn of(value: &str) -> Self {
        match (
            value.trim_start().len() != value.len(),
            value.trim_end().len() != value.len(),
        ) {
            (false, false) => Self::None,
            (true, false) => Self::Leading,
            (false, true) => Self::Trailing,
            (true, true) => Self::Both,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Field {
    Base,
    Json,
    XmlName,
    Other,
}

impl Field {
    fn of(field: &str) -> Self {
        if field.eq_ignore_ascii_case(PROPERTY) {
            Self::Base
        } else if field.eq_ignore_ascii_case("offlineInventoryJSON") {
            Self::Json
        } else if field.eq_ignore_ascii_case("offlineInventoryXMLName") {
            Self::XmlName
        } else {
            Self::Other
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Folder {
    Bare,
    ScriptsShopInventories,
    DataScriptsShopInventories,
    OtherRelative,
    RootedOrDrive,
    EmptyOrTraversal,
}

fn folder(value: &str) -> Folder {
    if value.starts_with(['/', '\\']) || value.contains(':') {
        return Folder::RootedOrDrive;
    }
    if value
        .split(['/', '\\'])
        .any(|part| matches!(part, "" | "." | ".."))
    {
        return Folder::EmptyOrTraversal;
    }
    let Some((parent, _)) = value.rsplit_once(['/', '\\']) else {
        return Folder::Bare;
    };
    let parent = parent.replace('\\', "/");
    if parent.eq_ignore_ascii_case("scripts/shopinventories") {
        Folder::ScriptsShopInventories
    } else if parent.eq_ignore_ascii_case("data/scripts/shopinventories") {
        Folder::DataScriptsShopInventories
    } else {
        Folder::OtherRelative
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Extension {
    Missing,
    Empty,
    Json,
    Xml,
    Other,
}

impl Extension {
    fn of(file: &str) -> Self {
        match file.rsplit_once('.').map(|(_, extension)| extension) {
            None => Self::Missing,
            Some("") => Self::Empty,
            Some(value) if value.eq_ignore_ascii_case("json") => Self::Json,
            Some(value) if value.eq_ignore_ascii_case("xml") => Self::Xml,
            Some(_) => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum OuterTrim {
    None,
    Ascii,
    Unicode,
}

impl OuterTrim {
    fn of(original: &str, trimmed: &str, folder: Folder) -> Self {
        if original == trimmed
            || shop_inventory_name::stem(original).is_some()
            || !matches!(
                folder,
                Folder::Bare | Folder::ScriptsShopInventories | Folder::DataScriptsShopInventories
            )
            || !shop_inventory_name::stem(trimmed).is_some_and(|stem| {
                stem == stem.trim()
                    && stem.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b' ')
                    })
            })
        {
            return Self::None;
        }

        if original.trim_matches(|character: char| character.is_ascii_whitespace()) == trimmed {
            Self::Ascii
        } else {
            Self::Unicode
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Extension, Field, Folder, InvalidShape, OuterTrim, Whitespace};

    #[test]
    fn a_trailing_separator_or_unknown_extension_stays_unresolved() {
        let other = InvalidShape::of("offlineInventoryXMLName", "unused.hypothetical");
        assert_eq!(other.field, Field::XmlName, "the declared field family");
        assert_eq!(
            other.extension,
            Extension::Other,
            "the exact suffix stays private"
        );
        assert_eq!(
            other.folder,
            Folder::Bare,
            "the value contains no directory"
        );
        assert_eq!(other.outer_trim, OuterTrim::None);
        let trailing = InvalidShape::of("offlineInventory", "Scripts/ShopInventories/");
        assert_eq!(trailing.folder, Folder::EmptyOrTraversal);
        assert_eq!(trailing.outer_trim, OuterTrim::None);
    }

    #[test]
    fn trim_candidates_keep_whitespace_and_grammar_distinct() {
        let ascii = InvalidShape::of(
            "offlineInventoryJSON",
            " Scripts\\ShopInventories\\unused.json\t",
        );
        assert_eq!(ascii.folder, Folder::ScriptsShopInventories);
        assert_eq!(ascii.extension, Extension::Json);
        assert_eq!(
            ascii.whitespace,
            Whitespace::Both,
            "the candidate has whitespace at both ends"
        );
        assert!(ascii.control, "the trailing tab is a control character");
        assert_eq!(ascii.outer_trim, OuterTrim::Ascii);
        let unicode = InvalidShape::of(
            "offlineInventory",
            "Data/Scripts/ShopInventories/unused.xml\u{a0}",
        );
        assert!(unicode.non_ascii);
        assert_eq!(unicode.folder, Folder::DataScriptsShopInventories);
        assert_eq!(unicode.extension, Extension::Xml);
        assert_eq!(unicode.outer_trim, OuterTrim::Unicode);
    }

    #[test]
    fn outer_trim_cannot_admit_foreign_rooted_or_traversing_folders() {
        for (value, folder) in [
            ("Other/unused.json\t", Folder::OtherRelative),
            (
                "C:\\Scripts\\ShopInventories\\unused.json\t",
                Folder::RootedOrDrive,
            ),
            (
                "Scripts/ShopInventories/../unused.json\t",
                Folder::EmptyOrTraversal,
            ),
        ] {
            let shape = InvalidShape::of("offlineInventory", value);
            assert_eq!(
                shape.folder, folder,
                "report the original candidate's folder class"
            );
            assert_eq!(shape.outer_trim, OuterTrim::None);
        }
    }
}
