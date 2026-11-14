// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const ITEM: u128 = 0x8303;
pub(super) const ITEM_INDEX: u32 = 2;
pub(super) const ALPHA: &str = "FixtureSelectorAlpha";
pub(super) const BETA: &str = "FixtureSelectorBeta";
pub(super) const RAPID: &str = "SWeaponActionFireRapidParams";
pub(super) const BURST: &str = "SWeaponActionFireBurstParams";
pub(super) const BEAM: &str = "SWeaponActionFireBeamParams";
pub(super) const WEAPON: &str = "SCItemWeaponComponentParams";
pub(super) const ACTION: &str = "SWeaponActionParams";
pub(super) const NEW_ACTION: &str = "SWeaponActionFixtureExtension";
pub(super) const UNRELATED: &str = "FixtureUnrelatedActionBase";
pub(super) const PROJECTILE: &str = "SProjectileLauncher";
pub(super) const LAUNCHER_BASE: &str = "SLauncherBase";
pub(super) const NEW_LAUNCHER: &str = "SLauncherFixtureExtension";
pub(super) const LAUNCHER_MARKER: u32 = 7;
pub(super) const MALFORMED_HEAT: u32 = 11;
const CONTAINER: &str = "SAmmoContainerComponentParams";
const PRIMARY: u128 = 0x8301;
const SECONDARY: u128 = 0x8302;

#[derive(Clone, Copy)]
pub(super) enum Case {
    Selectors,
    WrongDeclaration,
    UnreadableLabel,
    AuthoredBody,
    NewAction,
    WrongActionParent,
    NewLauncher,
    WrongLauncherParent,
    MalformedHeat,
}

impl Case {
    const fn enum_name(self) -> &'static str {
        match self {
            Self::WrongDeclaration => "FixtureUnrelatedProjectileType",
            _ => "EProjectileType",
        }
    }

    const fn rapid_heat_type(self) -> TestType {
        match self {
            Self::MalformedHeat => TestType::UInt32,
            _ => TestType::Single,
        }
    }

    const fn rapid_heat(self) -> TestValue {
        match self {
            Self::MalformedHeat => TestValue::UInt32(MALFORMED_HEAT),
            _ => TestValue::Single(0.5),
        }
    }
}

pub(super) const BAD_LABEL_OFFSET: u32 = u32::MAX - 1;
pub(super) const MELEE: &str = "SWeaponActionMeleeParams";

pub(super) fn database() -> DataForgeBuilder {
    database_for(Case::Selectors)
}

pub(super) fn database_for(case: Case) -> DataForgeBuilder {
    let types = item_types(launcher_types(DataForgeBuilder::new().version(8), case));
    types
        .record(ammo(PRIMARY, "FixturePrimaryAmmo", 1.25))
        .record(ammo(SECONDARY, "FixtureSecondaryAmmo", 2.5))
        .record(item(case))
}

fn launcher_types(types: DataForgeBuilder, case: Case) -> DataForgeBuilder {
    let selector_type = case.enum_name();
    let rapid_parent = if matches!(case, Case::WrongActionParent) {
        UNRELATED
    } else {
        ACTION
    };
    let projectile_parent = if matches!(case, Case::WrongLauncherParent) {
        UNRELATED
    } else {
        LAUNCHER_BASE
    };
    types
        .enumeration(selector_type, [ALPHA, BETA])
        .structure(TestStruct::new(LAUNCHER_BASE))
        .structure(TestStruct::new(NEW_LAUNCHER).parent(PROJECTILE))
        .structure(
            TestStruct::new(PROJECTILE)
                .parent(projectile_parent)
                .field("projectileType", TestType::Enum(selector_type.to_owned()))
                .field("heatCost", TestType::Single)
                .field("damageMultiplier", TestType::Single)
                .field("fixtureMarker", TestType::UInt32),
        )
        .structure(TestStruct::new(ACTION).field("hasReloadModesOnUI", TestType::Bool))
        .structure(TestStruct::new(MELEE).parent(ACTION))
        .structure(TestStruct::new(NEW_ACTION).parent(MELEE))
        .structure(TestStruct::new(UNRELATED))
        .structure(
            TestStruct::new(RAPID)
                .parent(rapid_parent)
                .field("launchParams", pointer("SLauncherBase"))
                .field("heatPerShot", case.rapid_heat_type())
                .field("fireDuringSpinUp", TestType::Bool),
        )
        .structure(
            TestStruct::new(BURST)
                .parent(ACTION)
                .field("launchParams", pointer("SLauncherBase"))
                .field("heatPerShot", TestType::Single)
                .field("shotCount", TestType::UInt8),
        )
        .structure(
            TestStruct::new(BEAM)
                .parent(ACTION)
                .field("heatCostPerSecond", TestType::Single)
                .field("hitType", TestType::String)
                .field("fullDamageRange", TestType::Single)
                .field("zeroDamageRange", TestType::Single)
                .field("damagePerSecond", pointer("DamageBase")),
        )
        .structure(TestStruct::new("DamageBase"))
        .structure(
            TestStruct::new("DamageInfo")
                .parent("DamageBase")
                .field("DamagePhysical", TestType::Single)
                .field("DamageEnergy", TestType::Single)
                .field("fixtureUnprojected", TestType::Bool),
        )
}

fn item_types(types: DataForgeBuilder) -> DataForgeBuilder {
    types
        .enumeration("FixtureItemType", ["WeaponGun"])
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", pointer("DataForgeComponentParams")),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(TestStruct::new("IWeaponComponentParams").parent("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", TestType::Class("SItemDefinition".to_owned())),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureItemType".to_owned())),
        )
        .structure(
            TestStruct::new(WEAPON)
                .parent("IWeaponComponentParams")
                .array("fireActions", pointer(ACTION)),
        )
        .structure(
            TestStruct::new(CONTAINER)
                .parent("DataForgeComponentParams")
                .field("ammoParamsRecord", reference("AmmoParams"))
                .field("secondaryAmmoParamsRecord", reference("AmmoParams"))
                .field("maxAmmoCount", TestType::Int32),
        )
        .structure(TestStruct::new("AmmoParams").field("speed", TestType::Single))
}

fn ammo(guid: u128, name: &str, speed: f32) -> TestRecord {
    TestRecord::new(
        guid,
        "AmmoParams",
        format!("AmmoParams.{name}"),
        format!("fixture/ammo/{name}.xml"),
    )
    .value("speed", TestValue::Single(speed))
}

fn item(case: Case) -> TestRecord {
    let attach = node(TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        node(TestInstance::new("SItemDefinition").value("Type", TestValue::text("WeaponGun"))),
    ));
    let container = node(
        TestInstance::new(CONTAINER)
            .value("ammoParamsRecord", TestValue::Reference(PRIMARY))
            .value("secondaryAmmoParamsRecord", TestValue::Reference(SECONDARY))
            .value("maxAmmoCount", TestValue::Int32(8)),
    );
    let burst = node(
        TestInstance::new(BURST)
            .value(
                "launchParams",
                launch(PROJECTILE, TestValue::text(BETA), 1.0, 2.0),
            )
            .value("heatPerShot", TestValue::Single(1.25))
            .value("shotCount", TestValue::UInt8(0)),
    );
    let beam = beam_action();
    let label = match case {
        Case::UnreadableLabel => TestValue::Raw(BAD_LABEL_OFFSET.to_le_bytes().to_vec()),
        _ => TestValue::text(ALPHA),
    };
    let launcher = if matches!(case, Case::NewLauncher) {
        NEW_LAUNCHER
    } else {
        PROJECTILE
    };
    let mut slots = vec![rapid(launcher, label, case), TestValue::Null, burst, beam];
    let other = match case {
        Case::AuthoredBody => Some(MELEE),
        Case::NewAction => Some(NEW_ACTION),
        _ => None,
    };
    if let Some(other) = other {
        slots.push(node(
            TestInstance::new(other).value("hasReloadModesOnUI", TestValue::Bool(false)),
        ));
    }
    let actions = weapon(slots);
    TestRecord::new(
        ITEM,
        "EntityClassDefinition",
        "EntityClassDefinition.FixtureActionOwner",
        "libs/foundry/records/entities/scitem/ships/fixture_weapon_actions.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![
            attach,
            actions,
            container,
            weapon(vec![rapid(PROJECTILE, TestValue::text(ALPHA), case)]),
        ]),
    )
}

fn beam_action() -> TestValue {
    let damage = TestInstance::new("DamageInfo")
        .value("DamagePhysical", TestValue::Single(0.0))
        .value("DamageEnergy", TestValue::Single(12.0))
        .value("fixtureUnprojected", TestValue::Bool(false));
    node(
        TestInstance::new(BEAM)
            .value("heatCostPerSecond", TestValue::Single(6.0))
            .value("hitType", TestValue::text("FixtureBeamHit"))
            .value("fullDamageRange", TestValue::Single(19.0))
            .value("zeroDamageRange", TestValue::Single(27.0))
            .value("damagePerSecond", node(damage)),
    )
}

fn weapon(actions: Vec<TestValue>) -> TestValue {
    node(TestInstance::new(WEAPON).value("fireActions", TestValue::Array(actions)))
}

fn rapid(launcher: &str, selector: TestValue, case: Case) -> TestValue {
    node(
        TestInstance::new(RAPID)
            .value("launchParams", launch(launcher, selector, 0.25, 0.0))
            .value("heatPerShot", case.rapid_heat())
            .value("fireDuringSpinUp", TestValue::Bool(false)),
    )
}

fn launch(launcher: &str, selector: TestValue, heat: f32, multiplier: f32) -> TestValue {
    node(
        TestInstance::new(launcher)
            .value("projectileType", selector)
            .value("heatCost", TestValue::Single(heat))
            .value("damageMultiplier", TestValue::Single(multiplier))
            .value("fixtureMarker", TestValue::UInt32(LAUNCHER_MARKER)),
    )
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

const fn node(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
