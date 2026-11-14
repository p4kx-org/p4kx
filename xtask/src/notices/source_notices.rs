// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::markdown::{Crate, Licence};

const MIT: &str = "MIT License";
const AUTHORED_MIT_HEAD: &str = "MIT License\n\nCopyright (c) Steven Sheldon\n\n";
const MIT_TEMPLATE_NOTICE: &str = "Copyright (c) <year> <copyright holders>";

#[derive(Clone, Copy)]
enum MitSource {
    Authored,
    Linked,
    Manifest,
}

struct PackageSource {
    name: &'static str,
    version: &'static str,
    revision: &'static str,
    origin: MitSource,
}

impl PackageSource {
    const fn new(
        name: &'static str,
        version: &'static str,
        revision: &'static str,
        origin: MitSource,
    ) -> Self {
        Self {
            name,
            version,
            revision,
            origin,
        }
    }

    const fn framework(name: &'static str) -> Self {
        Self::new(
            name,
            "0.3.2",
            "7b1abfd750a2cacaea71d6a56ecfb83cb7de560b",
            MitSource::Linked,
        )
    }

    const fn identity(&self) -> (&'static str, &'static str) {
        (self.name, self.version)
    }

    fn url(&self) -> String {
        let (repository, path) = match self.origin {
            MitSource::Authored => ("madsmtm/objc2", "LICENSE.txt"),
            MitSource::Linked => ("madsmtm/objc2", "LICENSE.md"),
            MitSource::Manifest => ("SSheldon/rust-dispatch", "Cargo.toml"),
        };
        format!(
            "https://github.com/{repository}/blob/{}/{path}",
            self.revision
        )
    }
}

const PACKAGES: &[PackageSource] = &[
    PackageSource::new(
        "block2",
        "0.5.1",
        "4fc083f1c6d6784577e38b0ee8dbd344481e2fd2",
        MitSource::Authored,
    ),
    PackageSource::new(
        "block2",
        "0.6.2",
        "b4167b582b2f75f9a1be75495c41b765344fd03c",
        MitSource::Linked,
    ),
    PackageSource::new(
        "dispatch",
        "0.2.0",
        "82d6c7a5b75dc0c71c3f46f87bb6c16a476f7748",
        MitSource::Manifest,
    ),
    PackageSource::new(
        "dispatch2",
        "0.3.1",
        "8852b424193ca41602281b3d7540d7c8ed51e49a",
        MitSource::Linked,
    ),
    PackageSource::new(
        "objc2",
        "0.5.3",
        "5237cbf081d04ef75a1f6a207b4018e7c1ae7438",
        MitSource::Authored,
    ),
    PackageSource::new(
        "objc2",
        "0.6.5",
        "d7d2fa23ceaa5e6096c923b081040e5d81b3b9df",
        MitSource::Linked,
    ),
    PackageSource::new(
        "objc2-app-kit",
        "0.2.2",
        "e282618be4c3a3b9542957e0c8540e9588472ce8",
        MitSource::Authored,
    ),
    PackageSource::framework("objc2-app-kit"),
    PackageSource::framework("objc2-audio-toolbox"),
    PackageSource::framework("objc2-core-audio"),
    PackageSource::framework("objc2-core-audio-types"),
    PackageSource::framework("objc2-core-foundation"),
    PackageSource::framework("objc2-core-graphics"),
    PackageSource::new(
        "objc2-encode",
        "4.1.0",
        "8d214f5477365ffcbcbb7de058c86ed9a518efb7",
        MitSource::Linked,
    ),
    PackageSource::new(
        "objc2-foundation",
        "0.2.2",
        "e282618be4c3a3b9542957e0c8540e9588472ce8",
        MitSource::Authored,
    ),
    PackageSource::framework("objc2-foundation"),
    PackageSource::framework("objc2-metal"),
    PackageSource::framework("objc2-quartz-core"),
];

fn published_mit(licence: &Licence) -> bool {
    licence.name == MIT
        && licence
            .used_by
            .iter()
            .any(|krate| krate.name == "objc-sys" && krate.version == "0.3.5")
}

pub(super) fn apply(licences: &mut Vec<Licence>) -> Result<()> {
    let mut reviewed = take_reviewed_packages(licences)?;
    let published = licences
        .iter_mut()
        .find(|licence| published_mit(licence))
        .context("missing the checksum-verified objc-sys MIT text")?;
    let terms = published
        .text
        .strip_prefix(AUTHORED_MIT_HEAD)
        .filter(|text| text.starts_with("Permission is hereby granted"))
        .context("unexpected objc-sys MIT copyright notice or text")?;
    let linked_text = format!(
        "{MIT}\n\nThe linked upstream licence files declare MIT terms for these packages.\n\n{terms}"
    );
    let dispatch_text =
        format!("{MIT}\n\nThe package manifest lists Steven Sheldon as the author.\n\n{terms}");
    let mut linked = Vec::new();
    let mut manifest = Vec::new();
    for source in PACKAGES {
        let mut krate = reviewed.remove(&source.identity()).with_context(|| {
            format!(
                "expected the audited MIT template for {} {}",
                source.name, source.version
            )
        })?;
        krate.licence_source = Some(source.url());
        match source.origin {
            MitSource::Authored => published.used_by.push(krate),
            MitSource::Linked => linked.push(krate),
            MitSource::Manifest => manifest.push(krate),
        }
    }
    licences.retain(|licence| !licence.used_by.is_empty());
    licences.extend([
        Licence {
            name: MIT.to_owned(),
            text: linked_text,
            used_by: linked,
        },
        Licence {
            name: MIT.to_owned(),
            text: dispatch_text,
            used_by: manifest,
        },
    ]);
    Ok(())
}

fn take_reviewed_packages(
    licences: &mut [Licence],
) -> Result<BTreeMap<(&'static str, &'static str), Crate>> {
    let mut reviewed = BTreeMap::new();
    for licence in licences
        .iter_mut()
        .filter(|licence| licence.name == MIT && licence.text.contains(MIT_TEMPLATE_NOTICE))
    {
        for krate in licence
            .used_by
            .extract_if(.., |krate| source_for(krate).is_some())
        {
            let source = source_for(&krate).context("audited package identity changed")?;
            let previous = reviewed.insert(source.identity(), krate);
            ensure!(
                previous.is_none(),
                "multiple MIT templates for {} {}",
                source.name,
                source.version
            );
        }
    }
    Ok(reviewed)
}

fn source_for(krate: &Crate) -> Option<&'static PackageSource> {
    PACKAGES
        .iter()
        .find(|source| (krate.name.as_str(), krate.version.as_str()) == source.identity())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, version: &str) -> Crate {
        Crate {
            name: name.to_owned(),
            version: version.to_owned(),
            repository: None,
            licence_source: None,
        }
    }

    fn licences() -> Vec<Licence> {
        vec![
            Licence {
                name: MIT.to_owned(),
                text: MIT_TEMPLATE_NOTICE.to_owned(),
                used_by: PACKAGES
                    .iter()
                    .map(|item| krate(item.name, item.version))
                    .collect(),
            },
            Licence {
                name: MIT.to_owned(),
                text: format!("{AUTHORED_MIT_HEAD}Permission is hereby granted"),
                used_by: vec![krate("objc-sys", "0.3.5")],
            },
        ]
    }

    #[test]
    fn only_audited_package_versions_get_their_published_notice_sources() -> Result<()> {
        let mut licences = licences();
        licences[0].used_by.push(krate("objc2", "1.0.0"));
        apply(&mut licences)?;
        assert_eq!(licences[0].used_by, vec![krate("objc2", "1.0.0")]);
        assert_eq!(licences[0].text, MIT_TEMPLATE_NOTICE);
        assert_eq!(licences[1].used_by.len(), 5);
        assert!(licences[1].text.starts_with(AUTHORED_MIT_HEAD));
        assert!(!licences[2].text.contains("Copyright (c)"));
        assert!(
            licences[3]
                .text
                .contains("The package manifest lists Steven Sheldon")
        );
        assert!(
            licences[3].used_by[0]
                .licence_source
                .as_deref()
                .is_some_and(|url| url.ends_with("/Cargo.toml"))
        );
        Ok(())
    }

    #[test]
    fn missing_and_duplicate_template_sources_fail_generation() {
        let mut missing = licences();
        missing[0].used_by.pop();
        assert!(apply(&mut missing).is_err());
        let mut duplicate = licences();
        duplicate[0].used_by.push(krate("block2", "0.5.1"));
        assert!(apply(&mut duplicate).is_err());
    }
}
