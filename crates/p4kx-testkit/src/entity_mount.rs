// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_armour::{MALE_ARMOUR, armour_database, armour_files};
use crate::entity_port::helper_node;
use crate::entity_schema::add_port_subhelper_fields;
use crate::{
    DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestLoadout, TestLoadoutEntry,
    TestModel, TestNode, TestPort, TestValue,
};

/// The class name of the character whose backpack hangs on a bone its armour has.
pub const MOUNT_COURIER: &str = "Mount_Courier";

/// The class name of the character whose backpack port names a bone its armour lacks.
pub const MOUNT_DRIFTER: &str = "Mount_Drifter";

/// The replacement carrier whose tagged pack port has no root transform.
pub const MOUNT_REPLACEMENT_WITHOUT_ROOT: &str = "Mount_NoRoot_Carrier";

/// The bone below which the satchel of [`MOUNT_COURIER`] hangs: the nearest bone of the body
/// skeleton above the bone `cloth` of its armour.
pub const MOUNT_BONE: &str = "spine";

/// Where the helper of the satchel sits in its model.
pub const SATCHEL_HELPER_Z: f32 = 0.5;

const IDENTITY: [[f32; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

fn satchel_model(mut helper: [[f32; 4]; 3]) -> TestModel {
    helper[2][3] = SATCHEL_HELPER_Z;
    TestModel::cube().with_nodes(vec![
        TestNode {
            name: "satchel".to_owned(),
            parent: None,
            local: IDENTITY,
            kind: 0,
        },
        TestNode {
            name: "satchel_loc".to_owned(),
            parent: Some(0),
            local: helper,
            kind: 3,
        },
    ])
}

fn satchel_port(helper: &str) -> TestPort {
    rigid_satchel_port()
        .helper(helper, [0.0, 0.1, 0.2], [0.0; 3])
        .item_helper("satchel_loc")
}

fn rigid_satchel_port() -> TestPort {
    TestPort::new("pack").item_type("Char_Armor_Backpack", Vec::<String>::new())
}

fn carrier(guid: u128, class: &str, port: TestPort) -> TestEntity {
    TestEntity::new(guid, class, format!("entities/scitem/{class}.xml"))
        .attach_type("Char_Armor_Torso")
        .geometry_variant("Male", format!("{MALE_ARMOUR}/plain.skin"), None)
        .port(port)
        .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "pack",
            "Mount_Satchel",
        )]))
}

fn character(guid: u128, class: &str, torso: &str) -> TestEntity {
    TestEntity::new(guid, class, format!("entities/scitem/{class}.xml")).loadout(
        TestLoadout::Manual(vec![TestLoadoutEntry::class("torso", torso)]),
    )
}

fn database() -> DataForgeBuilder {
    database_with_carrier(satchel_port("cloth"))
}

fn database_with_carrier(port: TestPort) -> DataForgeBuilder {
    let satchel = TestEntity::new(0x40, "Mount_Satchel", "entities/scitem/mount_satchel.xml")
        .attach_type("Char_Armor_Backpack")
        .geometry_variant("Male", format!("{MALE_ARMOUR}/satchel.cgf"), None);
    [
        satchel,
        carrier(0x41, "Mount_Carrier", port),
        carrier(0x42, "Mount_Lost", satchel_port("nowhere")),
        character(0x43, MOUNT_COURIER, "Mount_Carrier"),
        character(0x44, MOUNT_DRIFTER, "Mount_Lost"),
    ]
    .iter()
    .fold(armour_database(), |builder, entity| {
        builder.record(entity.record())
    })
}

/// Returns the Entries of a synthetic Archive that holds the bodies of [`armour_entries`] and
/// characters that wear a backpack at a port of their armour.
///
/// `Mount_Satchel` (`Char_Armor_Backpack`) is a model without bone weights, with the helper
/// `satchel_loc` at (0, 0, [`SATCHEL_HELPER_Z`]). `Mount_Carrier` (`Char_Armor_Torso`) is worn
/// like `TEST_Plain`, which has the bone `cloth` below `spine`, and has the port `pack`, a bone
/// attachment to `cloth` offset by (0, 0.1, 0.2) that holds the satchel. [`MOUNT_COURIER`] wears
/// the carrier; `Mount_Lost` is a carrier whose port names the bone `nowhere`, worn by
/// [`MOUNT_DRIFTER`].
///
/// [`armour_entries`]: crate::armour_entries
#[must_use]
pub fn mounted_item_entries() -> Vec<TestEntry> {
    mounted_entries(&database())
}

/// Returns the mounted-item fixture with a tagged alternative at original helper ordinals [1, 0].
#[must_use]
pub fn mounted_unresolved_helper_entries() -> Vec<TestEntry> {
    mounted_unresolved_helper_fixture(satchel_port("cloth"))
}

/// Returns the tagged helper fixture with no root transform.
#[must_use]
pub fn mounted_unresolved_helper_without_root_entries() -> Vec<TestEntry> {
    mounted_unresolved_helper_fixture(rigid_satchel_port())
}

/// Returns a mounted item whose replacement carrier has only an unresolved tagged helper.
#[must_use]
pub fn mounted_unresolved_replacement_entries() -> Vec<TestEntry> {
    let port = unresolved_helper_port(rigid_satchel_port());
    let database = database().record(
        overlay_carrier(0x51, MOUNT_REPLACEMENT_WITHOUT_ROOT)
            .port(port)
            .record(),
    );
    mounted_entries(&add_port_subhelper_fields(database))
}

fn mounted_unresolved_helper_fixture(port: TestPort) -> Vec<TestEntry> {
    let database = database_with_carrier(unresolved_helper_port(port));
    mounted_entries(&add_port_subhelper_fields(database))
}

fn unresolved_helper_port(port: TestPort) -> TestPort {
    let node_type = "SItemPortDefHelperNode";
    let alternative = helper_node("pelvis", "satchel", [0.0, 0.1, 0.8], [0.0; 3])
        .value("Tags", TestValue::text("FixtureUnresolvedTag"));
    let group = TestInstance::new(node_type)
        .value("Tags", TestValue::text(""))
        .value(
            "SubHelpers",
            TestValue::Array(vec![TestValue::Struct(alternative)]),
        );
    port.sub_helpers(vec![TestValue::Null, TestValue::Struct(group)])
}

fn mounted_entries(database: &DataForgeBuilder) -> Vec<TestEntry> {
    mounted_entries_with_satchel(database, &satchel_model(IDENTITY))
}

/// Returns a worn satchel with perpendicular port and locator rotation axes.
#[must_use]
pub fn mounted_item_frame_entries(item_helper: &str) -> Vec<TestEntry> {
    let port = rigid_satchel_port()
        .helper("cloth", [0.0, 0.1, 0.2], [90.0, 0.0, 0.0])
        .item_helper(item_helper);
    let locator = [[0.0, -1.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], IDENTITY[2]];
    mounted_entries_with_satchel(&database_with_carrier(port), &satchel_model(locator))
}

fn mounted_entries_with_satchel(
    database: &DataForgeBuilder,
    satchel: &TestModel,
) -> Vec<TestEntry> {
    let mut entries = vec![TestEntry::zstd("Data\\Game2.dcb", database.build())];
    entries.extend(armour_files());
    entries.push(TestEntry::stored(
        format!("Data\\{}\\satchel.cgf", MALE_ARMOUR.replace('/', "\\")),
        satchel.cgf(),
    ));
    entries.push(TestEntry::stored(
        format!("Data\\{}\\satchel.cgfm", MALE_ARMOUR.replace('/', "\\")),
        satchel.cgfm(),
    ));
    entries
}

/// Returns the mounted-item fixture with replacement carriers and an explicit satchel.
#[must_use]
pub fn mounted_overlay_entries() -> Vec<TestEntry> {
    let explicit_satchel = TestEntity::new(
        0x45,
        "Mount_Explicit_Satchel",
        "entities/scitem/mount_explicit_satchel.xml",
    )
    .attach_type("Char_Armor_Backpack")
    .geometry_variant("Male", format!("{MALE_ARMOUR}/satchel.cgf"), None);
    let database = database()
        .record(explicit_satchel.record())
        .record(
            overlay_carrier(0x46, "Mount_New_Carrier")
                .port(overlay_port())
                .record(),
        )
        .record(
            overlay_carrier(0x47, "Mount_Duplicate_Carrier")
                .port(overlay_port())
                .port(overlay_port())
                .record(),
        )
        .record(
            overlay_carrier(0x48, "Mount_Incompatible_Carrier")
                .port(
                    TestPort::new("pack")
                        .item_type("Char_Armor_Helmet", Vec::<String>::new())
                        .helper("cloth", [0.0, 0.1, 0.8], [0.0; 3]),
                )
                .record(),
        );
    mounted_entries(&with_overlay_occupancy(database))
}

fn with_overlay_occupancy(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .record(character(0x49, "Mount_Free_Wearer", "Mount_New_Carrier").record())
        .record(character(0x4a, "Mount_Occupied_Wearer", "Mount_Occupied_Carrier").record())
        .record(character(0x4b, "Mount_Ambiguous_Wearer", "Mount_Duplicate_Carrier").record())
        .record(character(0x4c, "Mount_Unknown_Wearer", "Mount_Unknown_Carrier").record())
        .record(
            overlay_carrier(0x4d, "Mount_Occupied_Carrier")
                .port(overlay_port())
                .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
                    "pack",
                    "Mount_Nonworn_Prop",
                )]))
                .record(),
        )
        .record(
            TestEntity::new(
                0x4e,
                "Mount_Nonworn_Prop",
                "entities/scitem/nonworn_prop.xml",
            )
            .attach_type("Misc")
            .record(),
        )
        .record(
            overlay_carrier(0x4f, "Mount_Unknown_Carrier")
                .port(overlay_port().tags("", "SyntheticRequirement"))
                .record(),
        )
        .record(
            TestEntity::new(
                0x50,
                "Mount_Second_Satchel",
                "entities/scitem/mount_second_satchel.xml",
            )
            .attach_type("Char_Armor_Backpack")
            .geometry_variant("Male", format!("{MALE_ARMOUR}/satchel.cgf"), None)
            .record(),
        )
}

fn overlay_port() -> TestPort {
    TestPort::new("pack")
        .item_type("Char_Armor_Backpack", Vec::<String>::new())
        .helper("cloth", [0.0, 0.1, 0.8], [0.0; 3])
        .item_helper("satchel_loc")
}

fn overlay_carrier(guid: u128, class: &str) -> TestEntity {
    TestEntity::new(guid, class, format!("entities/scitem/{class}.xml"))
        .attach_type("Char_Armor_Torso")
        .geometry_variant("Male", format!("{MALE_ARMOUR}/plain.skin"), None)
}
