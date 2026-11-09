// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::{WineContext, WineDrive, drive_names};

fn root() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"Z:\synthetic-wine-context")
    } else {
        PathBuf::from("/synthetic-wine-context")
    }
}

fn context() -> WineContext {
    let prefix = root();
    WineContext {
        c_drive: WineDrive {
            letter: 'c',
            path: prefix.join("drive_c"),
            canonical: None,
        },
        devices: prefix.join("dosdevices"),
        prefix,
        drives: Vec::new(),
    }
}

#[test]
fn exact_lowercase_drive_name_wins_regardless_of_enumeration_order() {
    for names in [["D:", "d:"], ["d:", "D:"]] {
        let names: Vec<_> = names.into_iter().map(OsString::from).collect();
        let selected = drive_names(&names);
        assert_eq!(
            selected.get(&'d').copied(),
            Some(OsStr::new("d:")),
            "the actual drive selector chooses the exact lowercase spelling before an alias"
        );
        assert_eq!(selected.len(), 1, "one ASCII drive has one interpretation");
    }
    let names = vec![
        OsString::from("D:"),
        OsString::from("drive"),
        OsString::from("c:"),
    ];
    let selected = drive_names(&names);
    assert_eq!(
        selected.get(&'d').copied(),
        Some(OsStr::new("D:")),
        "a unique uppercase fallback retains its original spelling"
    );
    assert_eq!(
        selected.len(),
        1,
        "the dedicated C mapping and unrelated names are excluded"
    );
}

#[test]
fn missing_or_drive_relative_non_c_mappings_cannot_escape_the_prefix_namespace() {
    let mut wine = context();
    assert_eq!(
        wine.host_path(Path::new(r"D:\Games")),
        None,
        "an unknown mapping is not guessed as a host drive-relative path"
    );
    wine.drives.push(WineDrive {
        letter: 'd',
        path: PathBuf::from("relative-drive"),
        canonical: None,
    });
    assert_eq!(
        wine.host_path(Path::new(r"D:\Games")),
        None,
        "an unresolved relative mapping is refused on every host"
    );
    if cfg!(windows) {
        wine.drives.clear();
        wine.drives.push(WineDrive {
            letter: 'd',
            path: PathBuf::from("d:"),
            canonical: None,
        });
        assert_eq!(
            wine.host_path(Path::new(r"D:\Games")),
            None,
            "Windows prefix replacement cannot release a d:Games path"
        );
    }
}

#[test]
fn known_absolute_and_resolved_relative_drive_bases_remain_host_absolute() {
    let mut wine = context();
    let mapped = root().join("dosdevices/mapped-drive");
    wine.drives.push(WineDrive {
        letter: 'd',
        path: mapped.clone(),
        canonical: None,
    });
    assert_eq!(
        wine.host_path(Path::new(r"D:\Games")),
        Some(mapped.join("Games")),
        "the actual host mapper uses the observed absolute drive base"
    );
    wine.drives.clear();
    wine.prefix = PathBuf::from("relative-prefix");
    wine.devices = wine.prefix.join("dosdevices");
    wine.drives.push(WineDrive {
        letter: 'd',
        path: wine.devices.join("mapped-drive"),
        canonical: Some(mapped.clone()),
    });
    assert_eq!(
        wine.host_path(Path::new(r"D:\Games")),
        Some(mapped.join("Games")),
        "an explicitly observed absolute resolution supports a relative input namespace"
    );
}

#[test]
fn embedded_drive_prefixes_cannot_replace_an_admitted_mapping_base() {
    let wine = context();
    for path in [
        r"C:\Games\D:\Elsewhere",
        r"C:\Games\D:Elsewhere",
        r"C:\Games\C:\Elsewhere",
    ] {
        assert_eq!(
            wine.host_path(Path::new(path)),
            None,
            "a drive-bearing rest component is refused on every host: {path}"
        );
    }
}
