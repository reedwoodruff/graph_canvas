use derivative::Derivative;

use crate::graph::NodeTemplate;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct TemplateGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub templates: Vec<String>, // template_ids
}

impl TemplateGroup {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            templates: Vec::new(),
        }
    }
}

#[derive(Derivative)]
#[derivative(Clone, Debug)]
pub struct GraphCanvasConfig {
    // Visual settings
    pub context_menu_size: (f64, f64),
    pub default_node_width: f64,
    pub default_node_height: f64,
    pub connection_control_point_distance: f64,
    pub slot_radius: f64,

    // Templates and initial state
    pub node_templates: Vec<NodeTemplate>,
    pub template_groups: Vec<TemplateGroup>,
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
            template_groups: Vec::new(),
            initial_nodes: Vec::new(),
            show_default_toolbar: true,
            snap_to_grid: false,
            grid_size: 20.0,
            is_mutable: true,
            is_movable: true,
        }
    }

    pub fn add_template_to_group(&mut self, template_id: &str, group_id: &str) -> bool {
        if let Some(group) = self.template_groups.iter_mut().find(|g| g.id == group_id) {
            if !group.templates.contains(&template_id.to_string()) {
                group.templates.push(template_id.to_string());
                return true;
            }
        }
        false
    }

    pub fn get_templates_by_group(&self, group_id: &str) -> Vec<&NodeTemplate> {
        if let Some(group) = self.template_groups.iter().find(|g| g.id == group_id) {
            return self
                .node_templates
                .iter()
                .filter(|template| group.templates.contains(&template.template_id))
                .collect();
        }
        Vec::new()
    }

    pub fn get_template_group_map(&self) -> Vec<(String, Vec<&NodeTemplate>)> {
        let mut result = Vec::new();

        // Create a "default" group for unassigned templates
        let mut assigned_template_ids = HashSet::new();

        // First, populate all explicitly defined groups
        for group in &self.template_groups {
            let templates = group.templates.iter().map(|template_name| {
                self.node_templates
                    .iter()
                    .find(|t| t.name == *template_name)
                    .expect(&format!(
                        "Configuration incorrect. Template not found: {}. Template group which holds unfound template: {}",
                        template_name, group.name
                    ))
            }).collect::<Vec<_>>();

            for template in &templates {
                assigned_template_ids.insert(template.template_id.clone());
            }

            if !templates.is_empty() {
                result.push((group.id.clone(), templates));
            }
        }

        // Then add "Other" group with unassigned templates
        let other_templates = self
            .node_templates
            .iter()
            .filter(|template| {
                !assigned_template_ids.contains(&template.template_id) && template.can_create
            })
            .collect::<Vec<_>>();

        if !other_templates.is_empty() {
            result.push(("other".to_string(), other_templates));
        }

        result
    }
}

impl Default for GraphCanvasConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(
    feature = "js",
    derive(serde::Serialize, serde::Deserialize, tsify::Tsify)
)]
#[cfg_attr(feature = "js", tsify(into_wasm_abi, from_wasm_abi))]
#[derive(Clone, Debug)]
pub enum TemplateIdentifier {
    Name(String),
    Id(String),
}

impl From<u128> for TemplateIdentifier {
    fn from(id: u128) -> Self {
        TemplateIdentifier::Id(uuid::Uuid::from_u128(id).to_string())
    }
}
impl ToString for TemplateIdentifier {
    fn to_string(&self) -> String {
        match self {
            TemplateIdentifier::Name(name) => name.clone(),
            TemplateIdentifier::Id(id) => id.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct InitialNode {
    pub template_identifier: TemplateIdentifier,
    pub x: f64,
    pub y: f64,
    pub can_delete: bool,
    pub can_move: bool,
    pub initial_connections: Vec<InitialConnection>,
    pub id: Option<String>,
    pub initial_field_values: Vec<InitialFieldValue>,
}
impl InitialNode {
    pub fn new(template_identifier: TemplateIdentifier) -> Self {
        Self {
            template_identifier,
            x: 10.0,
            y: 10.0,
            can_delete: true,
            can_move: true,
            initial_connections: vec![],
            id: None,
            initial_field_values: vec![],
        }
    }
}

#[derive(Clone, Debug)]
pub struct InitialConnection {
    pub host_slot_name: String,
    pub target_instance_id: String,
    pub can_delete: bool,
}

#[derive(Clone, Debug)]
pub struct InitialFieldValue {
    pub field_template_id: String,
    pub value: String,
}
