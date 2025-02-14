use web_sys::HtmlElement;

use crate::graph::NodeTemplate;

#[derive(Clone)]
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
    // pub custom_toolbar: Option<HtmlElement>,
}

impl Default for GraphCanvasConfig {
    fn default() -> Self {
        Self {
            context_menu_size: (400.0, 100.0),
            default_node_width: 150.0,
            default_node_height: 100.0,
            connection_control_point_distance: 75.0,
            slot_radius: 7.0,
            node_templates: Vec::new(),
            initial_nodes: Vec::new(),
            show_default_toolbar: true,
            snap_to_grid: false,
            grid_size: 20.0,
            // custom_toolbar: None,
        }
    }
}

#[derive(Clone)]
pub struct InitialNode {
    pub template_name: String,
    pub x: f64,
    pub y: f64,
    pub can_delete: bool,
    pub can_move: bool,
}
