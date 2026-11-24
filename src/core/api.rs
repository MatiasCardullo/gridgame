use std::io::Read;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tiny_http::{Header, Request, Response, Server, StatusCode};

use crate::core::Axial;
use crate::core::object_assets::MeshPrimitive;
#[cfg(test)]
use crate::core::planet_placement::NetworkAnchor;
use crate::core::planet_placement::{NetworkKind, NetworkPlacement};

pub const API_ADDRESS: &str = "127.0.0.1:39123";
const API_PREFIX: &str = "/api/v1";
const MAX_BODY_BYTES: u64 = 16 * 1024;
const MAX_CELLS_PER_PAGE: usize = 250;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiBuildingKind {
    Base,
    Builder,
    Housing,
    Factory,
    Mine,
    Warehouse,
    Refuel,
    Logistics,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiUnitPreset {
    Hauler,
    BuilderSupply,
    Shuttle,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiInteriorBlockKind {
    ConveyorBelt,
    MechanicArm,
    Chest,
    Assembler,
    Furnace,
    Pallet,
    Crate,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiAssemblerRecipe {
    BuilderTender,
    CargoHauler,
    SiteShuttle,
}

#[derive(Clone, Copy, Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApiArmInputMode {
    #[default]
    AnyNeighbor,
    ExplicitInputs,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiInteriorPartKind {
    ChassisFrame,
    WheelAssembly,
    EngineCore,
    ControlModule,
    ForkCarriage,
    MastSegment,
    HydraulicSet,
    SensorPack,
    FastenerBundle,
    BuilderTenderKit,
    CargoHaulerKit,
    SiteShuttleKit,
}

#[derive(Debug)]
pub enum ApiCommand {
    Screenshot,
    EnterPlanet,
    Status,
    Cells {
        offset: usize,
        limit: usize,
    },
    Buildings,
    Networks,
    Units,
    PlaceBuilding {
        cell: u32,
        triangle_index: u8,
        kind: ApiBuildingKind,
    },
    RemoveBuilding {
        cell: u32,
        triangle_index: u8,
    },
    PlaceNetwork {
        kind: NetworkKind,
        placement: NetworkPlacement,
    },
    RemoveNetwork {
        id: u32,
    },
    SpawnUnit {
        depot: u32,
        preset: ApiUnitPreset,
        nodes: ApiUnitNodes,
    },
    GetBaseInterior {
        base_cell: u32,
    },
    PlaceInteriorBlock {
        base_cell: u32,
        hex: Axial,
        kind: ApiInteriorBlockKind,
        rotation: u8,
    },
    RotateInteriorBlock {
        base_cell: u32,
        hex: Axial,
        rotation: u8,
    },
    RemoveInteriorBlock {
        base_cell: u32,
        hex: Axial,
    },
    SetAssemblerRecipe {
        base_cell: u32,
        hex: Axial,
        recipe: ApiAssemblerRecipe,
    },
    ConfigureMechanicArm {
        base_cell: u32,
        hex: Axial,
        config: ApiMechanicArmConfig,
    },
    CadBuildings,
    CadBuilding {
        kind: ApiBuildingKind,
    },
    AddCadPrimitive {
        kind: ApiBuildingKind,
        primitive: MeshPrimitive,
        center: [f32; 3],
        dimensions: [f32; 3],
        rotation_degrees: [f32; 3],
        sides: Option<usize>,
        color: Option<[u8; 4]>,
        replace: bool,
    },
    TransformCadBuilding {
        kind: ApiBuildingKind,
        translation: [f32; 3],
        rotation_degrees: [f32; 3],
        scale: [f32; 3],
    },
    SaveCadBuilding {
        kind: ApiBuildingKind,
    },
}

#[derive(Debug, Deserialize, Default)]
pub struct ApiUnitNodes {
    #[serde(default)]
    pub pickup: Option<u32>,
    #[serde(default)]
    pub dropoff: Option<u32>,
    #[serde(default)]
    pub refuel: Option<u32>,
    #[serde(default)]
    pub construction: Option<u32>,
    #[serde(default)]
    pub wait: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ApiMechanicArmConfig {
    #[serde(default)]
    pub input_mode: ApiArmInputMode,
    #[serde(default)]
    pub input_hexes: Vec<Axial>,
    #[serde(default)]
    pub output_hexes: Vec<Axial>,
    #[serde(default)]
    pub filters: Vec<ApiInteriorPartKind>,
}

#[derive(Debug)]
pub struct ApiMessage {
    pub command: ApiCommand,
    pub reply: SyncSender<ApiReply>,
}

#[derive(Clone, Debug)]
pub struct ApiReply {
    pub status: u16,
    pub body: Value,
    pub png: Option<Vec<u8>>,
}

impl ApiReply {
    pub fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            body,
            png: None,
        }
    }

    pub fn png(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            body: Value::Null,
            png: Some(body),
        }
    }

    pub fn error(status: u16, code: &str, message: impl Into<String>) -> Self {
        Self::json(
            status,
            json!({
                "error": {
                    "code": code,
                    "message": message.into()
                }
            }),
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ParseError {
    status: u16,
    code: &'static str,
    message: String,
}

#[derive(Deserialize)]
struct PlaceBuildingBody {
    cell_index: u32,
    #[serde(default)]
    triangle_index: u8,
    kind: ApiBuildingKind,
}

#[derive(Deserialize)]
struct PlaceNetworkBody {
    kind: NetworkKind,
    placement: NetworkPlacement,
}

#[derive(Deserialize)]
struct SpawnUnitBody {
    depot: u32,
    preset: ApiUnitPreset,
    #[serde(default)]
    nodes: ApiUnitNodes,
}

#[derive(Deserialize)]
struct CadPrimitiveBody {
    primitive: MeshPrimitive,
    center: [f32; 3],
    #[serde(default = "unit_dimensions")]
    dimensions: [f32; 3],
    #[serde(default)]
    rotation_degrees: [f32; 3],
    #[serde(default)]
    sides: Option<usize>,
    #[serde(default)]
    color: Option<[u8; 4]>,
    #[serde(default)]
    replace: bool,
}

#[derive(Deserialize)]
struct CadTransformBody {
    #[serde(default)]
    translation: [f32; 3],
    #[serde(default)]
    rotation_degrees: [f32; 3],
    #[serde(default = "unit_dimensions")]
    scale: [f32; 3],
}

fn unit_dimensions() -> [f32; 3] {
    [1.0; 3]
}

#[derive(Deserialize)]
struct InteriorBlockBody {
    hex: Axial,
    kind: ApiInteriorBlockKind,
    #[serde(default)]
    rotation: u8,
}

#[derive(Deserialize)]
struct InteriorRotationBody {
    rotation: u8,
}

#[derive(Deserialize)]
struct AssemblerRecipeBody {
    recipe: ApiAssemblerRecipe,
}

#[derive(Deserialize)]
struct MechanicArmConfigBody {
    #[serde(default)]
    input_mode: ApiArmInputMode,
    #[serde(default)]
    input_hexes: Vec<Axial>,
    #[serde(default)]
    output_hexes: Vec<Axial>,
    #[serde(default)]
    filters: Vec<ApiInteriorPartKind>,
}

pub fn start(sender: Sender<ApiMessage>) -> Result<(), String> {
    let server = Server::http(API_ADDRESS)
        .map_err(|error| format!("could not bind API at {API_ADDRESS}: {error}"))?;

    thread::Builder::new()
        .name("gridgame-api".to_string())
        .spawn(move || serve_requests(server, sender))
        .map_err(|error| format!("could not start API thread: {error}"))?;
    eprintln!("GridGame local API listening at http://{API_ADDRESS}{API_PREFIX}");
    Ok(())
}

fn serve_requests(server: Server, sender: Sender<ApiMessage>) {
    loop {
        match server.recv() {
            Ok(request) => handle_request(request, &sender),
            Err(error) => {
                eprintln!("GridGame local API request listener failed: {error}");
                break;
            }
        }
    }
}

fn handle_request(mut request: Request, sender: &Sender<ApiMessage>) {
    let reply = match validate_request_safety(&request) {
        Err(reply) => reply,
        Ok(()) => {
            let mut body = Vec::new();
            let read_result = request
                .as_reader()
                .take(MAX_BODY_BYTES + 1)
                .read_to_end(&mut body);
            match read_result {
                Err(error) => {
                    ApiReply::error(400, "invalid_body", format!("could not read body: {error}"))
                }
                Ok(_) if body.len() as u64 > MAX_BODY_BYTES => {
                    ApiReply::error(413, "body_too_large", "request body exceeds 16 KiB")
                }
                Ok(_) => match parse_request(request.method().as_str(), request.url(), &body) {
                    Err(error) => ApiReply::error(error.status, error.code, error.message),
                    Ok(command) => dispatch(command, sender),
                },
            }
        }
    };

    let (response, content_type) = match reply.png {
        Some(png) => (Response::from_data(png), "image/png"),
        None => {
            let serialized = match serde_json::to_string(&reply.body) {
                Ok(serialized) => serialized,
                Err(error) => {
                    eprintln!("GridGame API response serialization failed: {error}");
                    r#"{"error":{"code":"serialization_error","message":"could not serialize response"}}"#
                        .to_string()
                }
            };
            (
                Response::from_string(serialized),
                "application/json; charset=utf-8",
            )
        }
    };
    let header = match Header::from_bytes("Content-Type", content_type) {
        Ok(header) => header,
        Err(error) => {
            eprintln!("GridGame API could not construct response header: {error:?}");
            return;
        }
    };
    let response = response
        .with_status_code(StatusCode(reply.status))
        .with_header(header);
    if let Err(error) = request.respond(response) {
        eprintln!("GridGame API response failed: {error}");
    }
}

fn validate_request_safety(request: &Request) -> Result<(), ApiReply> {
    let host = request_header(request, "Host");
    if !is_allowed_host(host) {
        return Err(ApiReply::error(
            403,
            "host_not_allowed",
            "request Host must target the local API address",
        ));
    }

    if let Some(origin) = request_header(request, "Origin") {
        if !is_allowed_origin(origin) {
            return Err(ApiReply::error(
                403,
                "origin_not_allowed",
                "cross-origin browser requests are not allowed",
            ));
        }
    }

    if request.method().as_str() == "POST" {
        if !is_json_content_type(request_header(request, "Content-Type")) {
            return Err(ApiReply::error(
                415,
                "content_type_required",
                "POST requests must use application/json",
            ));
        }
    }
    Ok(())
}

fn request_header<'a>(request: &'a Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(name))
        .map(|header| header.value.as_str())
}

fn is_allowed_host(host: Option<&str>) -> bool {
    matches!(host, Some("127.0.0.1:39123" | "localhost:39123"))
}

fn is_allowed_origin(origin: &str) -> bool {
    matches!(origin, "http://127.0.0.1:39123" | "http://localhost:39123")
}

fn is_json_content_type(content_type: Option<&str>) -> bool {
    content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case("application/json"))
}

fn dispatch(command: ApiCommand, sender: &Sender<ApiMessage>) -> ApiReply {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    if sender
        .send(ApiMessage {
            command,
            reply: reply_tx,
        })
        .is_err()
    {
        return ApiReply::error(
            503,
            "game_unavailable",
            "game loop is not accepting API requests",
        );
    }
    match reply_rx.recv_timeout(REQUEST_TIMEOUT) {
        Ok(reply) => reply,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            ApiReply::error(503, "game_busy", "game did not process the request in time")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            ApiReply::error(503, "game_unavailable", "game loop stopped before replying")
        }
    }
}

fn parse_request(method: &str, url: &str, body: &[u8]) -> Result<ApiCommand, ParseError> {
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    if path == format!("{API_PREFIX}/screenshot") && method == "GET" {
        return Ok(ApiCommand::Screenshot);
    }
    if path == format!("{API_PREFIX}/scene/planet") && method == "PUT" {
        return Ok(ApiCommand::EnterPlanet);
    }
    if path == format!("{API_PREFIX}/status") && method == "GET" {
        return Ok(ApiCommand::Status);
    }
    if path == format!("{API_PREFIX}/cells") && method == "GET" {
        let (offset, limit) = parse_pagination(query)?;
        return Ok(ApiCommand::Cells { offset, limit });
    }
    if path == format!("{API_PREFIX}/buildings") && method == "GET" {
        return Ok(ApiCommand::Buildings);
    }
    if path == format!("{API_PREFIX}/networks") && method == "GET" {
        return Ok(ApiCommand::Networks);
    }
    if path == format!("{API_PREFIX}/units") && method == "GET" {
        return Ok(ApiCommand::Units);
    }
    if path == format!("{API_PREFIX}/buildings") && method == "POST" {
        let payload = parse_json_body::<PlaceBuildingBody>(body)?;
        return Ok(ApiCommand::PlaceBuilding {
            cell: payload.cell_index,
            triangle_index: payload.triangle_index,
            kind: payload.kind,
        });
    }
    if path == format!("{API_PREFIX}/networks") && method == "POST" {
        let payload = parse_json_body::<PlaceNetworkBody>(body)?;
        return Ok(ApiCommand::PlaceNetwork {
            kind: payload.kind,
            placement: payload.placement,
        });
    }
    if method == "DELETE" {
        if let Some(cell) = path.strip_prefix(&format!("{API_PREFIX}/buildings/")) {
            let mut parts = cell.split('/');
            let cell = parts
                .next()
                .and_then(|part| part.parse::<u32>().ok())
                .ok_or_else(|| {
                    parse_error(
                        400,
                        "invalid_cell",
                        "building path must begin with a cell index",
                    )
                })?;
            let triangle_index = parts
                .next()
                .map(str::parse::<u8>)
                .transpose()
                .map_err(|_| {
                    parse_error(
                        400,
                        "invalid_triangle_index",
                        "building path triangle index must be an integer",
                    )
                })?
                .unwrap_or(0);
            if parts.next().is_some() {
                return Err(parse_error(
                    400,
                    "invalid_building_path",
                    "building path must be /buildings/{cell_index}/{triangle_index}",
                ));
            }
            return Ok(ApiCommand::RemoveBuilding {
                cell,
                triangle_index,
            });
        }
        if let Some(id) = path.strip_prefix(&format!("{API_PREFIX}/networks/")) {
            let id = id.parse::<u32>().map_err(|_| {
                parse_error(
                    400,
                    "invalid_network_id",
                    "network path must end with an id",
                )
            })?;
            return Ok(ApiCommand::RemoveNetwork { id });
        }
    }
    if path == format!("{API_PREFIX}/units") && method == "POST" {
        let payload = parse_json_body::<SpawnUnitBody>(body)?;
        return Ok(ApiCommand::SpawnUnit {
            depot: payload.depot,
            preset: payload.preset,
            nodes: payload.nodes,
        });
    }
    if path == format!("{API_PREFIX}/cad/buildings") && method == "GET" {
        return Ok(ApiCommand::CadBuildings);
    }
    if let Some(rest) = path.strip_prefix(&format!("{API_PREFIX}/cad/buildings/")) {
        let mut parts = rest.split('/');
        let kind = parse_building_kind(parts.next().unwrap_or_default())?;
        match (parts.next(), method) {
            (None, "GET") => return Ok(ApiCommand::CadBuilding { kind }),
            (Some("primitives"), "POST") if parts.next().is_none() => {
                let payload = parse_json_body::<CadPrimitiveBody>(body)?;
                return Ok(ApiCommand::AddCadPrimitive {
                    kind,
                    primitive: payload.primitive,
                    center: payload.center,
                    dimensions: payload.dimensions,
                    rotation_degrees: payload.rotation_degrees,
                    sides: payload.sides,
                    color: payload.color,
                    replace: payload.replace,
                });
            }
            (Some("transform"), "PATCH") if parts.next().is_none() => {
                let payload = parse_json_body::<CadTransformBody>(body)?;
                return Ok(ApiCommand::TransformCadBuilding {
                    kind,
                    translation: payload.translation,
                    rotation_degrees: payload.rotation_degrees,
                    scale: payload.scale,
                });
            }
            (Some("save"), "PUT") if parts.next().is_none() => {
                return Ok(ApiCommand::SaveCadBuilding { kind });
            }
            _ => {}
        }
    }
    if let Some(command) = parse_interior_request(method, path, body)? {
        return Ok(command);
    }
    Err(parse_error(
        404,
        "not_found",
        format!("no API route for {method} {path}"),
    ))
}

fn parse_building_kind(value: &str) -> Result<ApiBuildingKind, ParseError> {
    match value {
        "base" => Ok(ApiBuildingKind::Base),
        "builder" => Ok(ApiBuildingKind::Builder),
        "housing" => Ok(ApiBuildingKind::Housing),
        "factory" => Ok(ApiBuildingKind::Factory),
        "mine" => Ok(ApiBuildingKind::Mine),
        "warehouse" => Ok(ApiBuildingKind::Warehouse),
        "refuel" => Ok(ApiBuildingKind::Refuel),
        "logistics" => Ok(ApiBuildingKind::Logistics),
        _ => Err(parse_error(
            400,
            "invalid_building_kind",
            format!("unknown CAD building kind '{value}'"),
        )),
    }
}

fn parse_interior_request(
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<Option<ApiCommand>, ParseError> {
    let prefix = format!("{API_PREFIX}/bases/");
    let Some(rest) = path.strip_prefix(&prefix) else {
        return Ok(None);
    };
    let Some((base_cell, resource_path)) = rest.split_once("/interior") else {
        return Ok(None);
    };
    if !resource_path.is_empty() && !resource_path.starts_with('/') {
        return Ok(None);
    }
    let base_cell = parse_u32(base_cell, "base cell")?;
    if resource_path.is_empty() && method == "GET" {
        return Ok(Some(ApiCommand::GetBaseInterior { base_cell }));
    }
    if resource_path == "/blocks" && method == "POST" {
        let payload = parse_json_body::<InteriorBlockBody>(body)?;
        return Ok(Some(ApiCommand::PlaceInteriorBlock {
            base_cell,
            hex: payload.hex,
            kind: payload.kind,
            rotation: payload.rotation,
        }));
    }

    let segments = resource_path
        .strip_prefix('/')
        .unwrap_or_default()
        .split('/')
        .collect::<Vec<_>>();
    match segments.as_slice() {
        ["blocks", q, r] if method == "PATCH" => {
            let payload = parse_json_body::<InteriorRotationBody>(body)?;
            Ok(Some(ApiCommand::RotateInteriorBlock {
                base_cell,
                hex: parse_axial(q, r)?,
                rotation: payload.rotation,
            }))
        }
        ["blocks", q, r] if method == "DELETE" => Ok(Some(ApiCommand::RemoveInteriorBlock {
            base_cell,
            hex: parse_axial(q, r)?,
        })),
        ["assemblers", q, r, "recipe"] if method == "PUT" => {
            let payload = parse_json_body::<AssemblerRecipeBody>(body)?;
            Ok(Some(ApiCommand::SetAssemblerRecipe {
                base_cell,
                hex: parse_axial(q, r)?,
                recipe: payload.recipe,
            }))
        }
        ["arms", q, r] if method == "PUT" => {
            let payload = parse_json_body::<MechanicArmConfigBody>(body)?;
            Ok(Some(ApiCommand::ConfigureMechanicArm {
                base_cell,
                hex: parse_axial(q, r)?,
                config: ApiMechanicArmConfig {
                    input_mode: payload.input_mode,
                    input_hexes: payload.input_hexes,
                    output_hexes: payload.output_hexes,
                    filters: payload.filters,
                },
            }))
        }
        _ => Ok(None),
    }
}

fn parse_u32(value: &str, label: &str) -> Result<u32, ParseError> {
    value.parse().map_err(|_| {
        parse_error(
            400,
            "invalid_coordinate",
            format!("{label} must be an integer"),
        )
    })
}

fn parse_axial(q: &str, r: &str) -> Result<Axial, ParseError> {
    let q = q
        .parse()
        .map_err(|_| parse_error(400, "invalid_coordinate", "q must be an integer"))?;
    let r = r
        .parse()
        .map_err(|_| parse_error(400, "invalid_coordinate", "r must be an integer"))?;
    Ok(Axial { q, r })
}

fn parse_pagination(query: &str) -> Result<(usize, usize), ParseError> {
    let mut offset = 0;
    let mut limit = 100;
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "offset" => {
                offset = value.parse().map_err(|_| {
                    parse_error(
                        400,
                        "invalid_offset",
                        "offset must be a non-negative integer",
                    )
                })?;
            }
            "limit" => {
                limit = value.parse().map_err(|_| {
                    parse_error(400, "invalid_limit", "limit must be a positive integer")
                })?;
            }
            _ => {
                return Err(parse_error(
                    400,
                    "invalid_query",
                    "unsupported query parameter",
                ));
            }
        }
    }
    if limit == 0 || limit > MAX_CELLS_PER_PAGE {
        return Err(parse_error(
            400,
            "invalid_limit",
            format!("limit must be between 1 and {MAX_CELLS_PER_PAGE}"),
        ));
    }
    Ok((offset, limit))
}

fn parse_json_body<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, ParseError> {
    serde_json::from_slice(body)
        .map_err(|error| parse_error(400, "invalid_json", format!("invalid JSON body: {error}")))
}

fn parse_error(status: u16, code: &'static str, message: impl Into<String>) -> ParseError {
    ParseError {
        status,
        code,
        message: message.into(),
    }
}

pub fn receive_request(receiver: &Receiver<ApiMessage>) -> Option<ApiMessage> {
    receiver.try_recv().ok()
}

#[cfg(test)]
mod tests {
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
}
