// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestInstance, TestValue};

/// A Loadout of a synthetic Entity or Loadout entry.
#[derive(Debug, Clone, PartialEq)]
pub enum TestLoadout {
    /// A manual Loadout with its entries in order.
    Manual(Vec<TestLoadoutEntry>),
    /// An XML Loadout naming the file that holds it.
    Xml(String),
    /// A Loadout generated at random from the `SLoadoutAssortment` Records with these GUIDs, see
    /// [`assortment_record`](crate::assortment_record).
    RandomGenerated(Vec<u128>),
    /// An outfit: a base Loadout and outfits, each with its name tags and the Loadouts it adds.
    Outfit {
        /// The base Loadout.
        base: Box<Self>,
        /// The outfits: name tags and Loadouts.
        outfits: Vec<(Vec<String>, Vec<Self>)>,
    },
}

impl TestLoadout {
    pub(crate) fn value(&self) -> TestValue {
        TestValue::Struct(match self {
            Self::Manual(entries) => TestInstance::new("SItemPortLoadoutManualParams").value(
                "entries",
                TestValue::Array(entries.iter().map(TestLoadoutEntry::value).collect()),
            ),
            Self::Xml(path) => TestInstance::new("SItemPortLoadoutXMLParams")
                .value("loadoutPath", TestValue::text(path.clone())),
            Self::RandomGenerated(assortments) => {
                TestInstance::new("SItemPortLoadoutRandomGeneratedParams").value(
                    "LoadoutAssortments",
                    TestValue::Array(
                        assortments
                            .iter()
                            .copied()
                            .map(TestValue::Reference)
                            .collect(),
                    ),
                )
            }
            Self::Outfit { base, outfits } => {
                let outfit = |(tags, loadouts): &(Vec<String>, Vec<Self>)| {
                    TestValue::Struct(
                        TestInstance::new("SItemPortLoadoutOutfitTagParams")
                            .value(
                                "OutfitNameTags",
                                TestValue::Array(
                                    tags.iter().cloned().map(TestValue::Text).collect(),
                                ),
                            )
                            .value(
                                "Loadouts",
                                TestValue::Array(loadouts.iter().map(Self::value).collect()),
                            ),
                    )
                };
                TestInstance::new("SItemPortLoadoutOutfitParams")
                    .value("BaseLoadout", base.value())
                    .value(
                        "Outfits",
                        TestValue::Array(outfits.iter().map(outfit).collect()),
                    )
            }
        })
    }
}

/// One entry of a manual Loadout: a port and what is attached to it.
#[derive(Debug, Clone, PartialEq)]
pub struct TestLoadoutEntry {
    port: String,
    class_name: String,
    reference: Option<u128>,
    loadout: Option<TestLoadout>,
}

impl TestLoadoutEntry {
    /// Creates an entry for the port `port` with nothing attached.
    #[must_use]
    pub fn empty(port: impl Into<String>) -> Self {
        Self {
            port: port.into(),
            class_name: String::new(),
            reference: None,
            loadout: None,
        }
    }

    /// Creates an entry attaching the Entity of class name `class_name`.
    #[must_use]
    pub fn class(port: impl Into<String>, class_name: impl Into<String>) -> Self {
        Self {
            class_name: class_name.into(),
            ..Self::empty(port)
        }
    }

    /// Creates an entry attaching the Entity by a Reference to `guid`.
    #[must_use]
    pub fn reference(port: impl Into<String>, guid: u128) -> Self {
        Self {
            reference: Some(guid),
            ..Self::empty(port)
        }
    }

    /// Sets the class name the entry carries next to its Reference, which may name another Entity.
    #[must_use]
    pub fn named(mut self, class_name: impl Into<String>) -> Self {
        self.class_name = class_name.into();
        self
    }

    /// Sets the Loadout the entry gives the attached Entity.
    #[must_use]
    pub fn loadout(mut self, loadout: TestLoadout) -> Self {
        self.loadout = Some(loadout);
        self
    }

    fn value(&self) -> TestValue {
        let mut instance = TestInstance::new("SItemPortLoadoutEntryParams")
            .value("itemPortName", TestValue::text(self.port.clone()))
            .value("entityClassName", TestValue::text(self.class_name.clone()));
        if let Some(guid) = self.reference {
            instance = instance.value("entityClassReference", TestValue::Reference(guid));
        }
        if let Some(loadout) = &self.loadout {
            instance = instance.value("loadout", loadout.value());
        }
        TestValue::Struct(instance)
    }
}
