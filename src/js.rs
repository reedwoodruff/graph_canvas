use crate::common::generate_id;
use crate::config::{GraphCanvasConfig, InitialConnection, InitialNode, TemplateGroup};
use crate::graph::{NodeTemplate, SlotPosition, SlotTemplate, SlotType};
use serde::{Deserialize, Serialize};
use tsify::Tsify;

#[cfg(feature = "js")]
#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct JsTemplateGroup {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub templates: Vec<String>, // template IDs
}

impl From<JsTemplateGroup> for TemplateGroup {
    fn from(js_group: JsTemplateGroup) -> Self {
        Self {
            id: js_group.id,
            name: js_group.name,
            description: js_group.description,
            templates: js_group.templates,
        }
    }
}

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
    pub template_groups: Option<Vec<JsTemplateGroup>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_nodes: Option<Vec<JsPartialInitialNode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_default_toolbar: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snap_to_grid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_mutable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_movable: Option<bool>,
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
            template_groups: partial
                .template_groups
                .unwrap_or_default()
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
            is_mutable: partial.is_mutable.unwrap_or(default.is_mutable),
            is_movable: partial.is_movable.unwrap_or(default.is_movable),
        }
    }
}

// #[derive(Serialize, Deserialize, Tsify)]
// #[tsify(into_wasm_abi, from_wasm_abi)]
// pub struct JsSlotTemplate {
//     pub id: String,
//     pub name: String,
//     pub position: String,  // "left" | "right" | "top" | "bottom"
//     pub slot_type: String, // "incoming" | "outgoing"
//     pub allowed_connections: Vec<String>,
//     pub min_connections: usize,
//     pub max_connections: Option<usize>,
// }

// #[derive(Serialize, Deserialize, Tsify)]
// #[tsify(into_wasm_abi, from_wasm_abi)]
// pub struct JsInitialNode {
//     pub template_name: String,
//     pub x: f64,
//     pub y: f64,
//     pub can_delete: bool,
//     pub can_move: bool,
// }

// impl From<JsInitialNode> for InitialNode {
//     fn from(js_node: JsInitialNode) -> Self {
//         Self {
//             template_name: js_node.template_name,
//             x: js_node.x,
//             y: js_node.y,
//             can_delete: js_node.can_delete,
//             can_move: js_node.can_move,
//         }
//     }
// }

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_modify_slots: Option<bool>,
}

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct JsPartialSlotTemplate {
    // Required fields
    pub name: String,

    // Optional fields with defaults
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_type: Option<SlotType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<SlotPosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_connections: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_connections: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_modify_connections: Option<bool>,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_connections: Option<Vec<JsInitialConnection>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct JsInitialConnection {
    pub host_slot_name: String,
    pub target_instance_id: String,
    pub can_delete: bool,
}

// Implement conversions with defaults
impl From<JsPartialNodeTemplate> for NodeTemplate {
    fn from(partial: JsPartialNodeTemplate) -> Self {
        let default = NodeTemplate::new(&partial.name);
        let slot_templates = if let Some(partial_slot_templates) = partial.slot_templates {
            partial_slot_templates.into_iter().map(Into::into).collect()
        } else {
            default.slot_templates
        };
        Self {
            template_id: default.template_id,
            name: partial.name,
            slot_templates,
            min_instances: partial.min_instances.unwrap_or(default.min_instances),
            max_instances: partial.max_instances.unwrap_or(default.max_instances),
            can_delete: partial.can_delete.unwrap_or(default.can_delete),
            can_create: partial.can_create.unwrap_or(default.can_create),
            default_width: partial.default_width.unwrap_or(default.default_width),
            default_height: partial.default_height.unwrap_or(default.default_height),
            can_modify_slots: partial.can_modify_slots.unwrap_or(default.can_modify_slots),
        }
    }
}

impl From<JsPartialSlotTemplate> for SlotTemplate {
    fn from(partial: JsPartialSlotTemplate) -> Self {
        let default = SlotTemplate::new(&partial.name);
        let max_connections = if partial.max_connections.is_some() {
            partial.max_connections
        } else {
            default.max_connections
        };
        Self {
            id: generate_id(),
            name: partial.name,
            position: partial.position.unwrap_or(default.position),
            slot_type: partial.slot_type.unwrap_or(default.slot_type),
            allowed_connections: partial
                .allowed_connections
                .unwrap_or(default.allowed_connections),
            min_connections: partial.min_connections.unwrap_or(default.min_connections),
            max_connections,
            can_modify_connections: partial
                .can_modify_connections
                .unwrap_or(default.can_modify_connections),
        }
    }
}

impl From<JsPartialInitialNode> for InitialNode {
    fn from(partial: JsPartialInitialNode) -> Self {
        let initial_connections = match partial.initial_connections {
            Some(conns) => conns.into_iter().map(|conn| conn.into()).collect(),
            None => vec![],
        };
        Self {
            template_name: partial.template_name,
            x: partial.x,
            y: partial.y,
            can_delete: partial.can_delete.unwrap_or(true),
            can_move: partial.can_move.unwrap_or(true),
            initial_connections,
            id: partial.id,
        }
    }
}

impl From<JsInitialConnection> for InitialConnection {
    fn from(js_conn: JsInitialConnection) -> Self {
        Self {
            can_delete: js_conn.can_delete,
            host_slot_name: js_conn.host_slot_name,
            target_instance_id: js_conn.target_instance_id,
        }
    }
}
