use super::*;

#[test]
fn parses_cell_query_with_bounded_pagination() {
    assert!(matches!(
        parse_request("GET", "/api/v1/cells?offset=250&limit=50", &[]),
        Ok(ApiCommand::Cells {
            offset: 250,
            limit: 50
        })
    ));
    let error = parse_request("GET", "/api/v1/cells?limit=251", &[]).unwrap_err();
    assert_eq!(error.code, "invalid_limit");
}

#[test]
fn parses_building_and_unit_commands() {
    assert!(matches!(
        parse_request("GET", "/api/v1/networks", &[]),
        Ok(ApiCommand::Networks)
    ));
    assert!(matches!(
        parse_request(
            "POST",
            "/api/v1/buildings",
            br#"{"cell_index":12,"kind":"mine"}"#
        ),
        Ok(ApiCommand::PlaceBuilding {
            cell: 12,
            triangle_index: 0,
            kind: ApiBuildingKind::Mine
        })
    ));
    assert!(
        parse_request(
            "POST",
            "/api/v1/buildings",
            br#"{"cell_index":12,"kind":"route"}"#
        )
        .is_err()
    );
    assert!(matches!(
        parse_request(
            "POST",
            "/api/v1/buildings",
            br#"{"cell_index":12,"triangle_index":5,"kind":"factory"}"#
        ),
        Ok(ApiCommand::PlaceBuilding {
            cell: 12,
            triangle_index: 5,
            kind: ApiBuildingKind::Factory
        })
    ));
    assert!(matches!(
        parse_request(
            "POST",
            "/api/v1/networks",
            br#"{"kind":"route","placement":{"shared_side":{"first_cell":1,"second_cell":2}}}"#
        ),
        Ok(ApiCommand::PlaceNetwork {
            kind: NetworkKind::Route,
            placement: NetworkPlacement::SharedSide {
                first_cell: 1,
                second_cell: 2
            }
        })
    ));
    assert!(matches!(
            parse_request(
                "POST",
                "/api/v1/networks",
                br#"{"kind":"route","placement":{"anchor_segment":{"start":{"cell_center":{"cell_index":3}},"end":{"cell_corner":{"face_index":8}}}}}"#
            ),
            Ok(ApiCommand::PlaceNetwork {
                kind: NetworkKind::Route,
                placement: NetworkPlacement::AnchorSegment {
                    start: NetworkAnchor::CellCenter { cell_index: 3 },
                    end: NetworkAnchor::CellCorner { face_index: 8 }
                }
            })
        ));
    assert!(matches!(
        parse_request("DELETE", "/api/v1/buildings/12/4", &[]),
        Ok(ApiCommand::RemoveBuilding {
            cell: 12,
            triangle_index: 4
        })
    ));
    assert!(matches!(
        parse_request("DELETE", "/api/v1/networks/7", &[]),
        Ok(ApiCommand::RemoveNetwork { id: 7 })
    ));
    assert!(matches!(
        parse_request(
            "POST",
            "/api/v1/units",
            br#"{"depot":4,"preset":"hauler","nodes":{"pickup":5,"dropoff":8}}"#
        ),
        Ok(ApiCommand::SpawnUnit {
            depot: 4,
            preset: ApiUnitPreset::Hauler,
            ..
        })
    ));
}

#[test]
fn parses_cad_building_design_commands() {
    assert!(matches!(
        parse_request("GET", "/api/v1/cad/buildings", &[]),
        Ok(ApiCommand::CadBuildings)
    ));
    assert!(matches!(
        parse_request("GET", "/api/v1/cad/buildings/factory", &[]),
        Ok(ApiCommand::CadBuilding {
            kind: ApiBuildingKind::Factory
        })
    ));
    assert!(matches!(
        parse_request(
            "POST",
            "/api/v1/cad/buildings/base/primitives",
            br#"{"primitive":"polygon","sides":5,"center":[0,1,0],"dimensions":[2,1,2]}"#
        ),
        Ok(ApiCommand::AddCadPrimitive {
            kind: ApiBuildingKind::Base,
            primitive: MeshPrimitive::Polygon,
            sides: Some(5),
            replace: false,
            color: None,
            ..
        })
    ));
    assert!(matches!(
                parse_request(
                    "POST",
                    "/api/v1/cad/buildings/factory/primitives",
                    br#"{"primitive":"cube","center":[0,0.5,0],"dimensions":[2,1,2],"color":[40,80,160,255],"replace":true}"#
                ),
                Ok(ApiCommand::AddCadPrimitive {
                    kind: ApiBuildingKind::Factory,
                    color: Some([40, 80, 160, 255]),
                    replace: true,
                    ..
                })
        ));
    assert!(matches!(
        parse_request(
            "PATCH",
            "/api/v1/cad/buildings/mine/transform",
            br#"{"translation":[1,0,0],"rotation_degrees":[0,90,0],"scale":[1,2,1]}"#
        ),
        Ok(ApiCommand::TransformCadBuilding {
            kind: ApiBuildingKind::Mine,
            ..
        })
    ));
    assert!(matches!(
        parse_request("PUT", "/api/v1/cad/buildings/base/save", &[]),
        Ok(ApiCommand::SaveCadBuilding {
            kind: ApiBuildingKind::Base
        })
    ));
    assert_eq!(
        parse_request("GET", "/api/v1/cad/buildings/unknown", &[])
            .unwrap_err()
            .code,
        "invalid_building_kind"
    );
}

#[test]
fn parses_base_interior_commands() {
    assert!(matches!(
        parse_request("GET", "/api/v1/bases/7/interior", &[]),
        Ok(ApiCommand::GetBaseInterior { base_cell: 7 })
    ));
    assert!(matches!(
        parse_request(
            "POST",
            "/api/v1/bases/7/interior/blocks",
            br#"{"hex":{"q":1,"r":-2},"kind":"assembler","rotation":3}"#
        ),
        Ok(ApiCommand::PlaceInteriorBlock {
            base_cell: 7,
            kind: ApiInteriorBlockKind::Assembler,
            rotation: 3,
            ..
        })
    ));
    assert!(matches!(
        parse_request(
            "PUT",
            "/api/v1/bases/7/interior/assemblers/1/-2/recipe",
            br#"{"recipe":"builder_tender"}"#
        ),
        Ok(ApiCommand::SetAssemblerRecipe {
            base_cell: 7,
            recipe: ApiAssemblerRecipe::BuilderTender,
            ..
        })
    ));
    assert!(matches!(
            parse_request(
                "PUT",
                "/api/v1/bases/7/interior/arms/1/0",
                br#"{"input_mode":"explicit_inputs","input_hexes":[{"q":0,"r":0}],"output_hexes":[{"q":2,"r":0}],"filters":["chassis_frame"]}"#
            ),
            Ok(ApiCommand::ConfigureMechanicArm {
                base_cell: 7,
                config: ApiMechanicArmConfig {
                    input_mode: ApiArmInputMode::ExplicitInputs,
                    ..
                },
                ..
            })
        ));
    assert!(matches!(
        parse_request("DELETE", "/api/v1/bases/7/interior/blocks/1/-2", &[]),
        Ok(ApiCommand::RemoveInteriorBlock { base_cell: 7, .. })
    ));
}

#[test]
fn parses_planet_scene_activation() {
    assert!(matches!(
        parse_request("PUT", "/api/v1/scene/planet", &[]),
        Ok(ApiCommand::EnterPlanet)
    ));
}

#[test]
fn parses_screenshot_request() {
    assert!(matches!(
        parse_request("GET", "/api/v1/screenshot", &[]),
        Ok(ApiCommand::Screenshot)
    ));
}

#[test]
fn rejects_unknown_routes_and_malformed_commands() {
    assert_eq!(
        parse_request("GET", "/api/v1/anything", &[])
            .unwrap_err()
            .status,
        404
    );
    assert_eq!(
        parse_request("POST", "/api/v1/buildings", b"{bad")
            .unwrap_err()
            .code,
        "invalid_json"
    );
    assert_eq!(
        parse_request("DELETE", "/api/v1/buildings/nope", &[])
            .unwrap_err()
            .code,
        "invalid_cell"
    );
}

#[test]
fn dispatches_requests_through_the_game_loop_channel() {
    let (sender, receiver) = mpsc::channel();
    let api_thread = thread::spawn(move || dispatch(ApiCommand::Status, &sender));
    let message = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(matches!(message.command, ApiCommand::Status));
    message
        .reply
        .send(ApiReply::json(200, json!({ "planet_ready": true })))
        .unwrap();
    let reply = api_thread.join().unwrap();
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body["planet_ready"], true);
}

#[test]
fn limits_browser_access_to_loopback_and_json_posts() {
    assert!(is_allowed_host(Some("127.0.0.1:39123")));
    assert!(!is_allowed_host(Some("example.com:39123")));
    assert!(is_allowed_origin("http://localhost:39123"));
    assert!(!is_allowed_origin("https://example.com"));
    assert!(is_json_content_type(Some(
        "application/json; charset=utf-8"
    )));
    assert!(!is_json_content_type(Some("text/plain")));
    assert!(!is_json_content_type(None));
}
