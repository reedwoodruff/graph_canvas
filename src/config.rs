use crate::graph::NodeTemplate;

#[derive(Clone, Debug)]
pub struct GraphCanvasConfig {
    // Visual settings
    pub context_menu_size: (f64, f64),
    pub default_node_width: f64,
    pub default_node_height: f64,
    pub connection_control_point_distance: f64,
    pub slot_radius: f64,

    // Templates and initial state
    pub node_templates: Vec<NodeTemplate>,
    pub initial_nodes: Vec<InitialNode>,

    // Behavioral settings
    pub show_default_toolbar: bool,
    pub snap_to_grid: bool,
    pub grid_size: f64,
    pub is_mutable: bool,
    pub is_movable: bool,
    // pub custom_toolbar: Option<HtmlElement>,
}
impl GraphCanvasConfig {
    pub fn new() -> Self {
        Self {
            context_menu_size: (400.0, 100.0),
            default_node_width: 150.0,
            default_node_height: 100.0,
            connection_control_point_distance: 75.0,
            slot_radius: 12.0,
            node_templates: Vec::new(),
            initial_nodes: Vec::new(),
            show_default_toolbar: true,
            snap_to_grid: false,
            grid_size: 20.0,
            is_mutable: true,
            is_movable: true,
        }
    }
}

impl Default for GraphCanvasConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct InitialNode {
    pub template_name: String,
    pub x: f64,
    pub y: f64,
    pub can_delete: bool,
    pub can_move: bool,
    pub initial_connections: Vec<InitialConnection>,
    pub id: Option<String>,
}
impl InitialNode {
    pub fn new(template_name: String) -> Self {
        Self {
            template_name,
            x: 10.0,
            y: 10.0,
            can_delete: true,
            can_move: true,
            initial_connections: vec![],
            id: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct InitialConnection {
    pub host_slot_name: String,
    pub target_instance_id: String,
    pub can_delete: bool,
}
