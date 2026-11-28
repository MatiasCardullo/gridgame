use crate::core::planet_units::PlanetUnitPresetId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlanetUnitCategory {
    ExcavationModular,
    LightUtility,
    CargoLogistics,
    Drilling,
}

impl PlanetUnitCategory {
    pub fn label(self) -> &'static str {
        match self {
            PlanetUnitCategory::ExcavationModular => "Excavation / Modular",
            PlanetUnitCategory::LightUtility => "Light Utility",
            PlanetUnitCategory::CargoLogistics => "Cargo / Logistics",
            PlanetUnitCategory::Drilling => "Drilling",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlanetUnitPartItemId {
    ChassisFrame,
    EngineCore,
    ControlModule,
    FuelAssembly,
    PrimaryToolMount,
    CargoModule,
    ConstructionRig,
    DrillHead,
}

impl PlanetUnitPartItemId {
    pub fn label(self) -> &'static str {
        match self {
            PlanetUnitPartItemId::ChassisFrame => "Chassis Frame",
            PlanetUnitPartItemId::EngineCore => "Engine Core",
            PlanetUnitPartItemId::ControlModule => "Control Module",
            PlanetUnitPartItemId::FuelAssembly => "Fuel Assembly",
            PlanetUnitPartItemId::PrimaryToolMount => "Primary Tool Mount",
            PlanetUnitPartItemId::CargoModule => "Cargo Module",
            PlanetUnitPartItemId::ConstructionRig => "Construction Rig",
            PlanetUnitPartItemId::DrillHead => "Drill Head",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlanetUnitAttachmentId {
    Bucket,
    HydraulicHammer,
    Grapple,
    Auger,
    Ripper,
    Saw,
    Drill,
    Blade,
    Mixer,
}

impl PlanetUnitAttachmentId {
    pub fn label(self) -> &'static str {
        match self {
            PlanetUnitAttachmentId::Bucket => "Bucket",
            PlanetUnitAttachmentId::HydraulicHammer => "Hydraulic Hammer",
            PlanetUnitAttachmentId::Grapple => "Grapple",
            PlanetUnitAttachmentId::Auger => "Auger",
            PlanetUnitAttachmentId::Ripper => "Ripper",
            PlanetUnitAttachmentId::Saw => "Saw",
            PlanetUnitAttachmentId::Drill => "Drill",
            PlanetUnitAttachmentId::Blade => "Blade",
            PlanetUnitAttachmentId::Mixer => "Mixer",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlanetUnitItemId {
    CargoHaulerKit,
    SiteShuttleKit,
    BuilderTenderKit,
    ModularExcavatorKit,
    CompactUtilityLoaderKit,
    DrillRigKit,
}

impl PlanetUnitItemId {
    pub fn label(self) -> &'static str {
        match self {
            PlanetUnitItemId::CargoHaulerKit => "Haul Truck Kit",
            PlanetUnitItemId::SiteShuttleKit => "Container Truck Kit",
            PlanetUnitItemId::BuilderTenderKit => "Dump Truck Kit",
            PlanetUnitItemId::ModularExcavatorKit => "Modular Excavator Kit",
            PlanetUnitItemId::CompactUtilityLoaderKit => "Compact Utility Loader Kit",
            PlanetUnitItemId::DrillRigKit => "Drill Rig Kit",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PlanetUnitDefinitionId {
    CargoHauler,
    SiteShuttle,
    BuilderTender,
    ModularExcavator,
    CompactUtilityLoader,
    DrillRig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanetUnitPartStack {
    pub item_id: PlanetUnitPartItemId,
    pub amount: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanetUnitDefinition {
    pub id: PlanetUnitDefinitionId,
    pub item_id: PlanetUnitItemId,
    pub label: &'static str,
    pub category: PlanetUnitCategory,
    pub preset_id: Option<PlanetUnitPresetId>,
    pub supported_attachments: &'static [PlanetUnitAttachmentId],
    pub required_parts: &'static [PlanetUnitPartStack],
}

const EXCAVATOR_ATTACHMENTS: &[PlanetUnitAttachmentId] = &[
    PlanetUnitAttachmentId::Bucket,
    PlanetUnitAttachmentId::HydraulicHammer,
    PlanetUnitAttachmentId::Grapple,
    PlanetUnitAttachmentId::Auger,
    PlanetUnitAttachmentId::Ripper,
];

const LIGHT_UTILITY_ATTACHMENTS: &[PlanetUnitAttachmentId] = &[
    PlanetUnitAttachmentId::Bucket,
    PlanetUnitAttachmentId::Saw,
    PlanetUnitAttachmentId::Drill,
    PlanetUnitAttachmentId::Blade,
    PlanetUnitAttachmentId::Mixer,
];

const NO_ATTACHMENTS: &[PlanetUnitAttachmentId] = &[];

const HAULER_PARTS: &[PlanetUnitPartStack] = &[
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ChassisFrame,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::EngineCore,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ControlModule,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::FuelAssembly,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::CargoModule,
        amount: 1,
    },
];

const SHUTTLE_PARTS: &[PlanetUnitPartStack] = &[
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ChassisFrame,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::EngineCore,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ControlModule,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::FuelAssembly,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::CargoModule,
        amount: 1,
    },
];

const BUILDER_PARTS: &[PlanetUnitPartStack] = &[
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ChassisFrame,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::EngineCore,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ControlModule,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::FuelAssembly,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ConstructionRig,
        amount: 1,
    },
];

const EXCAVATOR_PARTS: &[PlanetUnitPartStack] = &[
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ChassisFrame,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::EngineCore,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ControlModule,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::FuelAssembly,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::PrimaryToolMount,
        amount: 1,
    },
];

const UTILITY_LOADER_PARTS: &[PlanetUnitPartStack] = &[
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ChassisFrame,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::EngineCore,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ControlModule,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::FuelAssembly,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::PrimaryToolMount,
        amount: 1,
    },
];

const DRILL_RIG_PARTS: &[PlanetUnitPartStack] = &[
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ChassisFrame,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::EngineCore,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::ControlModule,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::FuelAssembly,
        amount: 1,
    },
    PlanetUnitPartStack {
        item_id: PlanetUnitPartItemId::DrillHead,
        amount: 1,
    },
];

const UNIT_DEFINITIONS: &[PlanetUnitDefinition] = &[
    PlanetUnitDefinition {
        id: PlanetUnitDefinitionId::CargoHauler,
        item_id: PlanetUnitItemId::CargoHaulerKit,
        label: "Haul Truck",
        category: PlanetUnitCategory::CargoLogistics,
        preset_id: Some(PlanetUnitPresetId::Hauler),
        supported_attachments: NO_ATTACHMENTS,
        required_parts: HAULER_PARTS,
    },
    PlanetUnitDefinition {
        id: PlanetUnitDefinitionId::SiteShuttle,
        item_id: PlanetUnitItemId::SiteShuttleKit,
        label: "Container Truck",
        category: PlanetUnitCategory::CargoLogistics,
        preset_id: Some(PlanetUnitPresetId::Shuttle),
        supported_attachments: NO_ATTACHMENTS,
        required_parts: SHUTTLE_PARTS,
    },
    PlanetUnitDefinition {
        id: PlanetUnitDefinitionId::BuilderTender,
        item_id: PlanetUnitItemId::BuilderTenderKit,
        label: "Dump Truck",
        category: PlanetUnitCategory::LightUtility,
        preset_id: Some(PlanetUnitPresetId::BuilderSupply),
        supported_attachments: LIGHT_UTILITY_ATTACHMENTS,
        required_parts: BUILDER_PARTS,
    },
    PlanetUnitDefinition {
        id: PlanetUnitDefinitionId::ModularExcavator,
        item_id: PlanetUnitItemId::ModularExcavatorKit,
        label: "Modular Excavator",
        category: PlanetUnitCategory::ExcavationModular,
        preset_id: None,
        supported_attachments: EXCAVATOR_ATTACHMENTS,
        required_parts: EXCAVATOR_PARTS,
    },
    PlanetUnitDefinition {
        id: PlanetUnitDefinitionId::CompactUtilityLoader,
        item_id: PlanetUnitItemId::CompactUtilityLoaderKit,
        label: "Compact Utility Loader",
        category: PlanetUnitCategory::LightUtility,
        preset_id: None,
        supported_attachments: LIGHT_UTILITY_ATTACHMENTS,
        required_parts: UTILITY_LOADER_PARTS,
    },
    PlanetUnitDefinition {
        id: PlanetUnitDefinitionId::DrillRig,
        item_id: PlanetUnitItemId::DrillRigKit,
        label: "Drill Rig",
        category: PlanetUnitCategory::Drilling,
        preset_id: None,
        supported_attachments: NO_ATTACHMENTS,
        required_parts: DRILL_RIG_PARTS,
    },
];

pub fn unit_definition(id: PlanetUnitDefinitionId) -> &'static PlanetUnitDefinition {
    UNIT_DEFINITIONS
        .iter()
        .find(|definition| definition.id == id)
        .unwrap_or(&UNIT_DEFINITIONS[0])
}

pub fn runtime_definition_for_preset(
    preset_id: PlanetUnitPresetId,
) -> &'static PlanetUnitDefinition {
    UNIT_DEFINITIONS
        .iter()
        .find(|definition| definition.preset_id == Some(preset_id))
        .unwrap_or(&UNIT_DEFINITIONS[0])
}

#[cfg(test)]
#[path = "../../tests/core/planet_unit_catalog.rs"]
mod tests;
