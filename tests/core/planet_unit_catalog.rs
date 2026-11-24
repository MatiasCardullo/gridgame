use std::collections::HashSet;

use super::*;

#[test]
fn catalog_covers_all_v1_categories() {
    let categories: HashSet<PlanetUnitCategory> = UNIT_DEFINITIONS
        .iter()
        .map(|definition| definition.category)
        .collect();
    assert!(categories.contains(&PlanetUnitCategory::ExcavationModular));
    assert!(categories.contains(&PlanetUnitCategory::LightUtility));
    assert!(categories.contains(&PlanetUnitCategory::CargoLogistics));
    assert!(categories.contains(&PlanetUnitCategory::Drilling));
}

#[test]
fn every_definition_has_required_parts() {
    for definition in UNIT_DEFINITIONS {
        assert!(
            !definition.required_parts.is_empty(),
            "definition {:?} should declare parts",
            definition.id
        );
    }
}

#[test]
fn attachment_compatibility_is_category_bound() {
    let supports = |category: PlanetUnitCategory, attachment_id: PlanetUnitAttachmentId| {
        UNIT_DEFINITIONS
            .iter()
            .filter(|definition| definition.category == category)
            .any(|definition| definition.supported_attachments.contains(&attachment_id))
    };

    assert!(supports(
        PlanetUnitCategory::ExcavationModular,
        PlanetUnitAttachmentId::HydraulicHammer
    ));
    assert!(!supports(
        PlanetUnitCategory::CargoLogistics,
        PlanetUnitAttachmentId::HydraulicHammer
    ));
    assert!(!supports(
        PlanetUnitCategory::Drilling,
        PlanetUnitAttachmentId::Bucket
    ));
}

#[test]
fn runtime_preset_maps_to_catalog_definition() {
    assert_eq!(
        runtime_definition_for_preset(PlanetUnitPresetId::Hauler).id,
        PlanetUnitDefinitionId::CargoHauler
    );
    assert_eq!(
        runtime_definition_for_preset(PlanetUnitPresetId::BuilderSupply).id,
        PlanetUnitDefinitionId::BuilderTender
    );
}
