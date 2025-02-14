use crate::common::generate_id;
use crate::config::{GraphCanvasConfig, InitialNode};
use crate::graph::{NodeTemplate, SlotPosition, SlotTemplate, SlotType};
use serde::{Deserialize, Serialize};
use tsify::Tsify;

#[cfg(feature = "js")]
#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct JsPartialConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_menu_size: Option<(f64, f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_node_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_node_height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_control_point_distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_templates: Option<Vec<JsPartialNodeTemplate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_nodes: Option<Vec<JsPartialInitialNode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_default_toolbar: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snap_to_grid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_size: Option<f64>,
}

impl From<JsPartialConfig> for GraphCanvasConfig {
    fn from(partial: JsPartialConfig) -> Self {
        let default = GraphCanvasConfig::default();

        Self {
            context_menu_size: partial
                .context_menu_size
                .unwrap_or(default.context_menu_size),
            default_node_width: partial
                .default_node_width
                .unwrap_or(default.default_node_width),
            default_node_height: partial
                .default_node_height
                .unwrap_or(default.default_node_height),
            connection_control_point_distance: partial
                .connection_control_point_distance
                .unwrap_or(default.connection_control_point_distance),
            slot_radius: partial.slot_radius.unwrap_or(default.slot_radius),
            node_templates: partial
                .node_templates
                .unwrap_or(Default::default())
                .into_iter()
                .map(Into::into)
                .collect(),
            initial_nodes: partial
                .initial_nodes
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            show_default_toolbar: partial
                .show_default_toolbar
                .unwrap_or(default.show_default_toolbar),
            snap_to_grid: partial.snap_to_grid.unwrap_or(default.snap_to_grid),
            grid_size: partial.grid_size.unwrap_or(default.grid_size),
        }
    }
}

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct JsSlotTemplate {
    pub id: String,
    pub name: String,
    pub position: String,  // "left" | "right" | "top" | "bottom"
    pub slot_type: String, // "incoming" | "outgoing"
    pub allowed_connections: Vec<String>,
    pub min_connections: usize,
    pub max_connections: Option<usize>,
}

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct JsInitialNode {
    pub template_name: String,
    pub x: f64,
    pub y: f64,
    pub can_delete: bool,
    pub can_move: bool,
}

impl From<JsInitialNode> for InitialNode {
    fn from(js_node: JsInitialNode) -> Self {
        Self {
            template_name: js_node.template_name,
            x: js_node.x,
            y: js_node.y,
            can_delete: js_node.can_delete,
            can_move: js_node.can_move,
        }
    }
}

impl From<String> for SlotPosition {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Top" | "top" => SlotPosition::Top,
            "Bottom" | "bottom" => SlotPosition::Bottom,
            "Left" | "left" => SlotPosition::Left,
            "Right" | "right" => SlotPosition::Right,
            _ => SlotPosition::Top,
        }
    }
}
impl From<String> for SlotType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Input" => SlotType::Incoming,
            "Output" => SlotType::Outgoing,
            _ => SlotType::Incoming,
        }
    }
}

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct JsPartialNodeTemplate {
    // Required fields
    pub name: String,

    // Optional fields with defaults
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_templates: Option<Vec<JsPartialSlotTemplate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_instances: Option<Option<usize>>, // Double Option because min_instances itself is Optional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_instances: Option<Option<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_delete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_create: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_height: Option<f64>,
}

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct JsPartialSlotTemplate {
    // Required fields
    pub name: String,
    pub position: SlotPosition,
    pub slot_type: SlotType,

    // Optional fields with defaults
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_connections: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_connections: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<usize>,
}

#[cfg(feature = "js")]
#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct JsPartialInitialNode {
    // Required fields
    pub template_name: String,
    pub x: f64,
    pub y: f64,

    // Optional fields with defaults
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_delete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_move: Option<bool>,
}

// Implement conversions with defaults
impl From<JsPartialNodeTemplate> for NodeTemplate {
    fn from(partial: JsPartialNodeTemplate) -> Self {
        Self {
            template_id: generate_id(),
            name: partial.name,
            slot_templates: partial
                .slot_templates
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            min_instances: partial.min_instances.unwrap_or(None),
            max_instances: partial.max_instances.unwrap_or(None),
            can_delete: partial.can_delete.unwrap_or(true),
            can_create: partial.can_create.unwrap_or(true),
            default_width: partial.default_width.unwrap_or(150.0),
            default_height: partial.default_height.unwrap_or(100.0),
        }
    }
}

impl From<JsPartialSlotTemplate> for SlotTemplate {
    fn from(partial: JsPartialSlotTemplate) -> Self {
        Self {
            id: generate_id(),
            name: partial.name,
            position: partial.position,
            slot_type: partial.slot_type,
            allowed_connections: partial.allowed_connections.unwrap_or_default(),
            min_connections: partial.min_connections.unwrap_or(0),
            max_connections: partial.max_connections,
        }
    }
}

impl From<JsPartialInitialNode> for InitialNode {
    fn from(partial: JsPartialInitialNode) -> Self {
        Self {
            template_name: partial.template_name,
            x: partial.x,
            y: partial.y,
            can_delete: partial.can_delete.unwrap_or(true),
            can_move: partial.can_move.unwrap_or(true),
        }
    }
}
