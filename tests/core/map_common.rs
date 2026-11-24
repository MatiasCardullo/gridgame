use super::*;

#[test]
fn confirm_label_missing_target() {
    let label = confirm_label_for_target(None, |_target| Some("X".to_string()));
    assert_eq!(label, "Block not found");
}

#[test]
fn confirm_label_with_target() {
    let target = Axial { q: 1, r: -2 };
    let label = confirm_label_for_target(Some(target), |_target| Some("Test".to_string()));
    assert_eq!(label, "Demolish Test?");
}
