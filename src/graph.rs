use crate::{
    errors::{GraphError, GraphResult},
    log, InitialNode,
};
use std::collections::HashMap;

use crate::{
    common::generate_id,
    events::{EventSystem, SystemEvent},
};

pub trait NodeTemplateInfo {
    fn get_slot_template(&self, slot_id: &str) -> Option<&SlotTemplate>;
    fn get_slot_template_by_name(&self, slot_name: &str) -> Option<&SlotTemplate>;
}

// Template definitions
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    Boolean,
    Integer,
    String,
}

#[derive(Debug, Clone)]
pub struct FieldTemplate {
    pub id: String,
    pub name: String,
    pub field_type: FieldType,
    pub default_value: String,
}

impl FieldTemplate {
    pub fn new(name: &str, field_type: FieldType, default_value: &str) -> Self {
        Self {
            id: generate_id(),
            name: name.to_string(),
            field_type,
            default_value: default_value.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeTemplate {
    pub template_id: String,
    pub name: String,
    pub slot_templates: Vec<SlotTemplate>,
    pub field_templates: Vec<FieldTemplate>,
    pub max_instances: Option<usize>,
    pub min_instances: Option<usize>,
    pub can_delete: bool,
    pub can_create: bool,
    pub can_modify_slots: bool,
    pub can_modify_fields: bool,
    // Visual defaults could go here
    pub default_width: f64,
    pub default_height: f64,
}
impl NodeTemplate {
    pub fn new(name: &str) -> Self {
        Self {
            template_id: generate_id(),
            name: name.to_string(),
            slot_templates: vec![],
            field_templates: vec![],
            max_instances: None,
            min_instances: None,
            can_delete: true,
            can_create: true,
            default_width: 150.0,
            default_height: 100.0,
            can_modify_slots: true,
            can_modify_fields: true,
        }
    }
}

impl NodeTemplateInfo for NodeTemplate {
    fn get_slot_template(&self, slot_id: &str) -> Option<&SlotTemplate> {
        self.slot_templates.iter().find(|st| st.id == slot_id)
    }
    fn get_slot_template_by_name(&self, slot_name: &str) -> Option<&SlotTemplate> {
        self.slot_templates.iter().find(|st| st.name == slot_name)
    }
}
impl NodeTemplate {}

#[derive(Debug, Clone)]
pub struct SlotTemplate {
    pub id: String,
    pub name: String,
    pub position: SlotPosition,
    pub slot_type: SlotType,
    pub allowed_connections: Vec<String>, // template_ids that can connect here
    pub min_connections: usize,
    pub max_connections: Option<usize>,
    pub can_modify_connections: bool,
}
impl SlotTemplate {
    pub fn new(name: &str) -> Self {
        Self {
            id: generate_id(),
            name: name.to_string(),
            position: SlotPosition::Right,
            slot_type: SlotType::Outgoing,
            allowed_connections: vec![],
            min_connections: 0,
            max_connections: None,
            can_modify_connections: true,
        }
    }
}

// Instance definitions
#[derive(Debug, Clone)]
pub struct FieldInstance {
    pub node_instance_id: String,
    pub field_template_id: String,
    pub value: String,
    pub can_modify: bool,
}

#[derive(Debug, Clone)]
pub struct NodeInstance {
    pub instance_id: String,
    pub template_id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub slots: Vec<SlotInstance>,
    pub fields: Vec<FieldInstance>,
    pub can_delete: bool,
    pub can_move: bool,
    pub can_modify_connections: bool,
    pub can_modify_fields: bool,
}

impl NodeInstance {
    pub fn new(template: &NodeTemplate, instance_id: String, x: f64, y: f64) -> Self {
        let mut slots = template
            .slot_templates
            .iter()
            .map(|st| SlotInstance {
                node_instance_id: instance_id.clone(),
                slot_template_id: st.id.clone(),
                node_template_id: template.template_id.clone(),
                connections: Vec::new(),
                can_modify: true,
            })
            .collect::<Vec<_>>();

        // Add the standard incoming slot
        slots.push(SlotInstance {
            node_instance_id: instance_id.clone(),
            node_template_id: template.template_id.clone(),
            slot_template_id: "incoming".to_string(),
            connections: Vec::new(),
            can_modify: true,
        });

        // Create field instances from template
        let fields = template
            .field_templates
            .iter()
            .map(|ft| FieldInstance {
                node_instance_id: instance_id.clone(),
                field_template_id: ft.id.clone(),
                value: ft.default_value.clone(),
                can_modify: true,
            })
            .collect::<Vec<_>>();

        Self {
            instance_id,
            template_id: template.template_id.clone(),
            x,
            y,
            width: template.default_width,
            height: template.default_height,
            slots,
            fields,
            can_delete: true,
            can_move: true,
            can_modify_connections: true,
            can_modify_fields: true,
        }
    }
    pub fn capabilities<'a>(&'a self, graph: &'a Graph) -> NodeCapabilities<'a> {
        let template = graph.node_templates.get(&self.template_id).unwrap();
        NodeCapabilities {
            template,
            instance: self,
        }
    }
}

pub struct NodeCapabilities<'a> {
    pub template: &'a NodeTemplate,
    pub instance: &'a NodeInstance,
}

impl NodeTemplateInfo for NodeCapabilities<'_> {
    fn get_slot_template(&self, slot_id: &str) -> Option<&SlotTemplate> {
        self.template.get_slot_template(slot_id)
    }
    fn get_slot_template_by_name(&self, slot_name: &str) -> Option<&SlotTemplate> {
        self.template
            .slot_templates
            .iter()
            .find(|st| st.name == slot_name)
    }
}
impl<'a> NodeCapabilities<'a> {
    pub fn new(template: &'a NodeTemplate, instance: &'a NodeInstance) -> Self {
        Self { template, instance }
    }
}

#[derive(Debug, Clone)]
pub struct SlotInstance {
    // pub id: String,
    pub slot_template_id: String,
    pub node_template_id: String,
    pub node_instance_id: String,
    pub connections: Vec<Connection>,
    pub can_modify: bool,
}
impl SlotInstance {
    pub fn capabilities<'a>(&'a self, graph: &'a Graph) -> SlotCapabilities<'a> {
        let node_template = graph.node_templates.get(&self.node_template_id).unwrap();
        let slot_template = node_template
            .get_slot_template(&self.slot_template_id)
            .unwrap();
        SlotCapabilities {
            template: slot_template,
            instance: self,
        }
    }
}
#[derive(Debug, Clone)]
pub struct SlotCapabilities<'a> {
    pub template: &'a SlotTemplate,
    pub instance: &'a SlotInstance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub can_delete: bool,
    pub host_node_id: String,
    pub host_slot_template_id: String,
    pub target_node_id: String,
    pub target_slot_template_id: String,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "js",
    derive(serde::Serialize, serde::Deserialize, tsify::Tsify)
)]
#[cfg_attr(feature = "js", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SlotPosition {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "js",
    derive(serde::Serialize, serde::Deserialize, tsify::Tsify)
)]
#[cfg_attr(feature = "js", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SlotType {
    Incoming,
    Outgoing,
}

#[derive(Debug)]
pub struct Graph {
    pub(crate) node_templates: HashMap<String, NodeTemplate>,
    pub(crate) node_instances: HashMap<String, NodeInstance>,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    pub fn new() -> Self {
        Graph {
            node_templates: HashMap::new(),
            node_instances: HashMap::new(),
        }
    }

    pub fn node_template_can_add_instance(&self, node_template_id: &str) -> bool {
        let instances_of_template = self.instances_of_node_template(node_template_id);
        let template = self.node_templates.get(node_template_id).unwrap();
        if let Some(max_instances) = template.max_instances {
            instances_of_template.len() < max_instances
        } else {
            true
        }
    }
    pub fn instances_of_node_template(&self, template_id: &str) -> Vec<String> {
        self.node_instances
            .iter()
            .filter(|(_, instance)| instance.template_id == template_id)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn register_template(&mut self, template: NodeTemplate) {
        let mut new_slots = template.slot_templates.clone();
        new_slots.push(SlotTemplate {
            id: "incoming".to_string(),
            name: "Incoming".to_string(),
            position: SlotPosition::Left,
            slot_type: SlotType::Incoming,
            allowed_connections: Vec::new(),
            min_connections: 0,
            max_connections: None,
            can_modify_connections: true,
        });
        let template_with_incoming_slot = NodeTemplate {
            slot_templates: new_slots,
            ..template
        };
        self.node_templates.insert(
            template_with_incoming_slot.template_id.clone(),
            template_with_incoming_slot,
        );
    }

    pub fn create_initial_nodes(&mut self, initial_nodes: &Vec<InitialNode>) -> GraphResult<()> {
        // Set up nodes before making connections
        let new_ids = initial_nodes
            .iter()
            .map(|node| {
                let template = self.get_node_template_by_name(&node.template_name).ok_or(
                    GraphError::ConfigurationError(
                        "Could not create initial node".to_string(),
                        Box::new(GraphError::TemplateNotFound(node.template_name.clone())),
                    ),
                )?;
                // let template_id = self
                //     .get_node_template_by_name(&node.template_name.clone())
                //     .ok_or(
                //         GraphError::ConfigurationError(
                //                                 "Could not create initial node".to_string(),
                //                                 Box::new(GraphError::TemplateNotFound(node.template_name.clone())),
                //                             ),

                //     );

                let instance_id = node.id.clone().unwrap_or(generate_id());
                let instance = NodeInstance {
                    instance_id: instance_id.clone(),
                    template_id: template.template_id.clone(),
                    x: node.x,
                    y: node.y,
                    width: template.default_width,
                    height: template.default_height,
                    can_move: true,
                    can_delete: true,
                    can_modify_connections: true,
                    can_modify_fields: true,
                    slots: template
                        .slot_templates
                        .iter()
                        .map(|slot_template| SlotInstance {
                            node_instance_id: instance_id.clone(),
                            node_template_id: template.template_id.clone(),
                            slot_template_id: slot_template.id.clone(),
                            connections: Vec::new(),
                            can_modify: true,
                        })
                        .collect(),
                    // Initialize fields from templates
                    fields: template
                        .field_templates
                        .iter()
                        .map(|field_template| {
                            let value =
                                match node.initial_field_values.iter().find(|initial_field| {
                                    initial_field.field_id == field_template.id
                                }) {
                                    Some(found_field) => found_field.value.clone(),
                                    None => field_template.default_value.clone(),
                                };
                            FieldInstance {
                                node_instance_id: instance_id.clone(),
                                field_template_id: field_template.id.clone(),
                                value,
                                can_modify: true,
                            }
                        })
                        .collect(),
                };

                self.node_instances.insert(instance_id.clone(), instance);
                // let new_instance =
                //     self.create_instance(&template.template_id, node.x, node.y, node.id.clone());
                // let new_instance = match new_instance {
                //     Ok(inner) => inner,
                //     Err(err) => {
                //         log(&format!("{:?}", err));
                //         return Err(err);
                //     }
                // };

                // Update instance properties if needed
                if let Some(instance) = self.node_instances.get_mut(&instance_id) {
                    instance.can_delete = node.can_delete;
                    instance.can_move = node.can_move;
                } else {
                    log("Instance wasn't created");
                }
                Ok::<(String, &InitialNode), GraphError>((instance_id, node))
            })
            .collect::<Vec<_>>();

        let mut connection_errors = vec![];
        // Make connections (not using `connect_slots` because we want to ignore mutability locks when setting up)
        for item in new_ids {
            if let Ok((host_node_id, node)) = item {
                for initial_connection in &node.initial_connections {
                    // log(&format!("{:#?}", self));
                    // log(&host_node_id);
                    let host_node_caps = self
                        .get_node_capabilities(&host_node_id)
                        .expect("Just created node, should exist");
                    let other_caps = self
                        .get_node_capabilities(&initial_connection.target_instance_id)
                        .ok_or(GraphError::NodeNotFound(
                            initial_connection.target_instance_id.clone(),
                        ));
                    if other_caps.is_err() {
                        connection_errors.push(other_caps.err().unwrap());
                        break;
                    }
                    let other_caps = other_caps.unwrap();

                    let host_node_slot = host_node_caps
                        .get_slot_template_by_name(&initial_connection.host_slot_name);
                    // host_node_caps.template.
                    if host_node_slot.is_none() {
                        connection_errors.push(GraphError::SlotNotFound {
                            node_id: "na".to_string(),
                            slot_id: initial_connection.host_slot_name.clone(),
                        });
                        break;
                    }
                    let host_node_slot_id = host_node_slot.unwrap().id.clone();
                    let target_node_id = other_caps.instance.instance_id.clone();
                    let target_slot_template_id = "incoming".to_string();

                    drop(host_node_caps);

                    self.node_instances
                        .get_mut(&host_node_id)
                        .expect("Just created node, should exist")
                        .slots
                        .iter_mut()
                        .find(|slot| slot.slot_template_id == host_node_slot_id)
                        .expect("Just created, should exist")
                        .connections
                        .push(Connection {
                            can_delete: initial_connection.can_delete,
                            host_node_id: host_node_id.clone(),
                            host_slot_template_id: host_node_slot_id,
                            target_node_id,
                            target_slot_template_id,
                        })
                }
            }
        }
        Ok(())
    }

    pub fn create_instance(
        &mut self,
        node_template_id: &str,
        x: f64,
        y: f64,
        id: Option<String>,
    ) -> GraphResult<String> {
        let template = {
            let result = self.node_templates.get(node_template_id).ok_or(Err(
                GraphError::NodeCreationFailed {
                    node_template_id: node_template_id.to_string(),
                    node_template_name: "?".to_string(),
                    reason: Box::new(GraphError::TemplateNotFound(node_template_id.to_string())),
                },
            ));
            match result {
                Ok(template) => template,
                Err(err) => return err,
            }
        };

        if !self.node_template_can_add_instance(node_template_id) {
            return Err(GraphError::NodeCreationFailed {
                node_template_id: node_template_id.to_string(),
                node_template_name: template.name.clone(),
                reason: Box::new(GraphError::Other("Maximum instances reached".to_string())),
            });
        }
        if !template.can_create {
            return Err(GraphError::NodeCreationFailed {
                node_template_id: node_template_id.to_string(),
                node_template_name: template.name.clone(),
                reason: Box::new(GraphError::Other("Template cannot be created".to_string())),
            });
        }

        let instance_id = id.unwrap_or(generate_id());
        let instance = NodeInstance {
            instance_id: instance_id.clone(),
            template_id: node_template_id.to_string(),
            x,
            y,
            width: template.default_width,
            height: template.default_height,
            can_move: true,
            can_delete: true,
            can_modify_connections: true,
            can_modify_fields: true,
            slots: template
                .slot_templates
                .iter()
                .map(|slot_template| SlotInstance {
                    node_instance_id: instance_id.clone(),
                    node_template_id: node_template_id.to_string(),
                    slot_template_id: slot_template.id.clone(),
                    connections: Vec::new(),
                    can_modify: true,
                })
                .collect(),
            // Initialize fields from templates
            fields: template
                .field_templates
                .iter()
                .map(|field_template| FieldInstance {
                    node_instance_id: instance_id.clone(),
                    field_template_id: field_template.id.clone(),
                    value: field_template.default_value.clone(),
                    can_modify: true,
                })
                .collect(),
        };

        self.node_instances.insert(instance_id.clone(), instance);
        Ok(instance_id)
    }

    pub fn get_node_capabilities(&self, instance_id: &str) -> Option<NodeCapabilities> {
        let instance = self.node_instances.get(instance_id)?;
        let template = self.node_templates.get(&instance.template_id)?;
        Some(NodeCapabilities { template, instance })
    }
    pub fn get_slot_capabilities(
        &self,
        node_instance_id: &str,
        slot_template_id: &str,
    ) -> Option<SlotCapabilities> {
        let instance = self.node_instances.get(node_instance_id)?;
        let slot = instance
            .slots
            .iter()
            .find(|s| s.slot_template_id == slot_template_id)?;
        let template = self.node_templates.get(&slot.node_template_id)?;
        let slot_template = template.get_slot_template(&slot.slot_template_id)?;
        Some(SlotCapabilities {
            template: slot_template,
            instance: slot,
        })
    }
    pub fn is_valid_connection(&self, connection: &Connection) -> GraphResult<()> {
        // Check if connection is valid based on templates
        // Implementation would check slot types, allowed connections, and current cardinality

        let Connection {
            host_node_id,
            host_slot_template_id,
            target_node_id,
            target_slot_template_id,
            ..
        } = connection;
        let from_slot_cap = self
            .get_slot_capabilities(&host_node_id, &host_slot_template_id)
            .unwrap();
        let target_node_cap = self.get_node_capabilities(&target_node_id).unwrap();
        let connection_allowed = if from_slot_cap
            .template
            .allowed_connections
            .contains(&target_node_cap.template.name)
        {
            Ok(())
        } else {
            Err("This slot type cannot connect to the attempted node template")
        };
        let max_len_reached = if from_slot_cap.instance.connections.len()
            < from_slot_cap.template.max_connections.unwrap_or(usize::MAX)
        {
            Ok(())
        } else {
            Err("Slot's max length is reached")
        };
        let connection_already_exists =
            if !from_slot_cap.instance.connections.iter().any(|connection| {
                connection.target_node_id == *target_node_id
                    && connection.target_slot_template_id == *target_slot_template_id
            }) {
                Ok(())
            } else {
                Err("Connection alrady exists")
            };
        if let Some(err) = max_len_reached.err() {
            return Err(GraphError::InvalidConnection {
                connection: connection.clone(),
                reason: err.to_string(),
            });
        }
        if let Some(err) = connection_already_exists.err() {
            return Err(GraphError::InvalidConnection {
                connection: connection.clone(),
                reason: err.to_string(),
            });
        }
        if let Some(err) = connection_allowed.err() {
            return Err(GraphError::InvalidConnection {
                connection: connection.clone(),
                reason: err.to_string(),
            });
        }
        Ok(())
    }

    pub fn connect_slots(
        &mut self,
        connection: Connection,
        events: &EventSystem,
    ) -> GraphResult<()> {
        let Connection {
            host_node_id,
            host_slot_template_id,
            ..
        } = connection.clone();
        let node_caps = self
            .get_node_capabilities(&host_node_id)
            .expect("Node should exist");
        let slot_template = node_caps
            .get_slot_template(&host_slot_template_id)
            .expect("Template should exist");

        self.is_valid_connection(&connection).map_err(|err| {
            GraphError::ConnectionCreationFailed {
                node_template_name: node_caps.template.name.clone(),
                slot_template_name: slot_template.name.clone(),
                reason: Box::new(err),
            }
        })?;

        self.check_node_modifiable_status(node_caps.instance)
            .map_err(|err| GraphError::ConnectionCreationFailed {
                node_template_name: node_caps.template.name.clone(),
                slot_template_name: slot_template.name.clone(),
                reason: Box::new(err),
            })?;
        if !slot_template.can_modify_connections {
            return Err(GraphError::ConnectionCreationFailed {
                node_template_name: node_caps.template.name.clone(),
                slot_template_name: slot_template.name.clone(),
                reason: Box::new(GraphError::SlotTemplateLocked {
                    name: slot_template.name.clone(),
                }),
            });
        }

        // Add connection to both slots
        if let Some(from_instance) = self.node_instances.get_mut(&host_node_id) {
            if let Some(slot) = from_instance
                .slots
                .iter_mut()
                .find(|s| s.slot_template_id == host_slot_template_id)
            {
                slot.connections.push(connection.clone());
            }
        }

        // if let Some(to_instance) = self.node_instances.get_mut(to_node) {
        //     if let Some(slot) = to_instance.slots.iter_mut().find(|s| s.id == to_slot) {
        //         slot.connections.push(Connection {
        //             host_node_id: to_node.to_string(),
        //             host_slot_id: to_slot.to_string(),
        //             target_node_id: from_node.to_string(),
        //             target_slot_id: from_slot.to_string(),
        //         });
        //     }
        // }

        events.emit(SystemEvent::ConnectionCompleted(connection));
        Ok(())
    }

    pub fn is_graph_valid(&self) -> bool {
        // Check if all nodes satisfy their template constraints
        for instance in self.node_instances.values() {
            if let Some(template) = self.node_templates.get(&instance.template_id) {
                for (slot, slot_template) in
                    instance.slots.iter().zip(template.slot_templates.iter())
                {
                    if slot.connections.len() < slot_template.min_connections
                        || slot.connections.len()
                            > slot_template.max_connections.unwrap_or(usize::MAX)
                    {
                        return false;
                    }
                    // Additional validation could go here
                }
            }
        }
        true
    }

    fn check_slot_modifiable_status(&self, slot: &SlotInstance) -> GraphResult<()> {
        let node_caps = self
            .get_node_capabilities(&slot.node_instance_id)
            .expect("Node should exist");
        let slot = node_caps
            .template
            .get_slot_template(&slot.slot_template_id)
            .expect("Slot should exist");
        let slot_instance = node_caps
            .instance
            .slots
            .iter()
            .find(|slot| slot.slot_template_id == *slot.slot_template_id)
            .expect("Slot should exist");
        if !slot.can_modify_connections {
            return Err(GraphError::SlotTemplateLocked {
                name: slot.name.clone(),
            });
        }
        if !slot_instance.can_modify {
            return Err(GraphError::SlotInstanceLocked);
        }

        self.check_node_modifiable_status(node_caps.instance)?;
        Ok(())
    }

    fn check_node_modifiable_status(&self, node: &NodeInstance) -> GraphResult<()> {
        let node_caps = self
            .get_node_capabilities(&node.instance_id)
            .expect("Node should exist");
        if !node_caps.template.can_modify_slots {
            return Err(GraphError::NodeTemplateLocked {
                name: node_caps.template.name.clone(),
            });
        }
        if !node_caps.instance.can_modify_connections {
            return Err(GraphError::NodeInstanceLocked);
        }
        Ok(())
    }

    fn check_connection_modifiable_status(&self, conn: &Connection) -> GraphResult<()> {
        let Connection {
            host_node_id,
            host_slot_template_id,
            ..
        } = conn;
        let node_caps = self
            .get_node_capabilities(host_node_id)
            .expect("Node should exist");
        // let slot = node_caps
        //     .template
        //     .get_slot_template(host_slot_template_id)
        //     .expect("Slot should exist");
        let slot_instance = node_caps
            .instance
            .slots
            .iter()
            .find(|slot| slot.slot_template_id == *host_slot_template_id)
            .expect("Slot should exist");
        if !conn.can_delete {
            return Err(GraphError::ConnectionDeletionFailed {
                connection: conn.clone(),
                reason: Box::new(GraphError::ConnectionLocked),
            });
        }
        self.check_slot_modifiable_status(slot_instance)
            .map_err(|err| GraphError::ConnectionDeletionFailed {
                connection: conn.clone(),
                reason: Box::new(err),
            })?;

        Ok(())
    }
    pub fn get_node_connections(&self, node_id: &str) -> Vec<Connection> {
        self.node_instances
            .get(node_id)
            .map(|instance| {
                instance
                    .slots
                    .iter()
                    .flat_map(|slot| slot.connections.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn delete_connection(&mut self, connection: &Connection) -> GraphResult<()> {
        let Connection {
            host_node_id,
            host_slot_template_id,
            target_node_id,
            target_slot_template_id,
            ..
        } = connection;
        self.check_connection_modifiable_status(connection)
            .map_err(|err| GraphError::ConnectionDeletionFailed {
                connection: connection.clone(),
                reason: Box::new(err),
            })?;

        if let Some(from_instance) = self.node_instances.get_mut(host_node_id) {
            if let Some(slot) = from_instance
                .slots
                .iter_mut()
                .find(|s| s.slot_template_id == *host_slot_template_id)
            {
                slot.connections.retain(|c| {
                    !(c.target_node_id == *target_node_id
                        && c.target_slot_template_id == *target_slot_template_id)
                });
            }
        }

        // if let Some(to_instance) = self.node_instances.get_mut(target_node_id) {
        //     if let Some(slot) = to_instance
        //         .slots
        //         .iter_mut()
        //         .find(|s| s.slot_template_id == *target_slot_template_id)
        //     {
        //         slot.connections.retain(|c| {
        //             !(c.target_node_id == *host_node_id
        //                 && c.target_slot_template_id == *host_slot_template_id)
        //         });
        //     }
        // }

        Ok(())
    }

    pub fn delete_node_instance(&mut self, node_id: &str) -> GraphResult<()> {
        let instance = self
            .node_instances
            .get(node_id)
            .ok_or(GraphError::NodeDeletionFailed {
                reason: Box::new(GraphError::NodeNotFound(node_id.to_string())),
                node_id: node_id.to_string(),
                node_template_name: "?".to_string(),
            })?
            .clone();
        let template = self
            .node_templates
            .get(&instance.template_id)
            .ok_or(GraphError::NodeDeletionFailed {
                reason: Box::new(GraphError::TemplateNotFound(
                    instance.template_id.to_string(),
                )),
                node_id: node_id.to_string(),
                node_template_name: "?".to_string(),
            })?
            .clone();
        let instances_of_template = self.instances_of_node_template(&template.template_id);

        self.check_node_modifiable_status(&instance)
            .map_err(|err| GraphError::NodeDeletionFailed {
                node_id: node_id.to_string(),
                node_template_name: template.name.clone(),
                reason: Box::new(err),
            })?;
        if template
            .min_instances
            .is_some_and(|min| instances_of_template.len() <= min)
        {
            return Err(GraphError::NodeDeletionFailed {
                node_id: node_id.to_string(),
                node_template_name: template.name.clone(),
                reason: Box::new(GraphError::Other("Minimum instances reached".to_string())),
            });
        }
        // First remove all connections from this node
        let slot_errors = instance
            .slots
            .iter()
            .map(|slot_instance| {
                self.delete_slot_connections(node_id, &slot_instance.slot_template_id)
            })
            .filter_map(|err| {
                if err.is_err() {
                    return Some(err.err().unwrap());
                }
                None
            })
            .collect::<Vec<_>>();
        if !slot_errors.is_empty() {
            return Err(GraphError::NodeDeletionFailed {
                node_id: node_id.to_string(),
                node_template_name: template.name.clone(),
                reason: Box::new(GraphError::SomeConnectionDeletionsFailed {
                    failures: slot_errors,
                }),
            });
        }

        // Then remove all connections to this node
        self.remove_all_incoming_connections(node_id)?;

        // Then remove the node
        self.node_instances.remove(node_id);
        Ok(())
    }
    pub fn remove_all_incoming_connections(&mut self, node_id: &str) -> GraphResult<()> {
        let connections_to_remove = self.node_instances.values().fold(vec![], |mut agg, inst| {
            agg.append(&mut inst.slots.iter().fold(vec![], |mut agg, slot| {
                agg.append(
                    &mut slot
                        .connections
                        .iter()
                        .filter(|conn| conn.target_node_id == node_id)
                        .cloned()
                        .collect::<Vec<_>>(),
                );
                agg
            }));
            agg
        });
        let errs = connections_to_remove
            .iter()
            .map(|conn| self.delete_connection(&conn))
            .fold(vec![], |mut agg, result| {
                if result.is_err() {
                    agg.push(result.err().unwrap());
                }
                agg
            });
        if !errs.is_empty() {
            return Err(GraphError::SomeConnectionDeletionsFailed { failures: errs });
        }
        Ok(())
    }
    pub fn delete_slot_connections(
        &mut self,
        node_id: &str,
        slot_template_id: &str,
    ) -> GraphResult<()> {
        let caps = self
            .get_slot_capabilities(node_id, slot_template_id)
            .ok_or(GraphError::SlotNotFound {
                node_id: node_id.to_string(),
                slot_id: slot_template_id.to_string(),
            })?;
        let slot_type = caps.template.slot_type.clone();

        self.check_slot_modifiable_status(caps.instance)
            .map_err(|err| GraphError::SlotDeletionFailed {
                slot_name: caps.template.name.clone(),
                reason: Box::new(err),
            })?;

        if let Some(instance) = self.node_instances.get(node_id).cloned() {
            if let Some(slot) = instance
                .slots
                .iter()
                .find(|s| s.slot_template_id == slot_template_id)
            {
                if slot_type == SlotType::Incoming {
                    self.remove_all_incoming_connections(node_id)?;
                } else {
                    let errs = slot
                        .connections
                        .iter()
                        .map(|conn| self.delete_connection(&conn))
                        .fold(vec![], |mut agg, result| {
                            if result.is_err() {
                                agg.push(result.err().unwrap());
                            }
                            agg
                        });
                    if !errs.is_empty() {
                        return Err(GraphError::SomeConnectionDeletionsFailed { failures: errs });
                    }
                }
                return Ok(());
            }
        }
        Err(GraphError::SlotNotFound {
            node_id: node_id.to_string(),
            slot_id: slot_template_id.to_string(),
        })
    }

    pub fn get_node_template_by_name(&self, name: &str) -> Option<NodeTemplate> {
        self.node_templates
            .values()
            .find(|t| t.name == name)
            .cloned()
    }
}

#[derive(Clone, Debug)]
pub enum GraphCommand {
    DeleteNode(String),
    DeleteConnection(Connection),
    DeleteSlotConnections {
        node_id: String,
        slot_template_id: String,
    },
    CreateConnection(Connection),
    CreateNode {
        template_id: String,
        x: f64,
        y: f64,
    },
    UpdateField {
        node_id: String,
        field_template_id: String,
        new_value: String,
    },
}

impl Graph {
    pub fn execute_command(
        &mut self,
        command: GraphCommand,
        events: &EventSystem,
    ) -> GraphResult<()> {
        let result = match command.clone() {
            GraphCommand::DeleteNode(node_id) => self.delete_node_instance(&node_id),
            GraphCommand::DeleteConnection(conn) => self.delete_connection(&conn),
            GraphCommand::DeleteSlotConnections {
                node_id,
                slot_template_id: slot_id,
            } => self.delete_slot_connections(&node_id, &slot_id),
            GraphCommand::CreateConnection(connection) => self.connect_slots(connection, events),
            GraphCommand::CreateNode { template_id, x, y } => {
                self.create_instance(&template_id, x, y, None).map(|_| ())
            }
            GraphCommand::UpdateField {
                node_id,
                field_template_id,
                new_value,
            } => self.update_field(&node_id, &field_template_id, new_value),
        };
        match &result {
            Ok(_) => {
                events.emit(SystemEvent::CommandExecuted(command));
            }
            Err(e) => {
                log(&format!("{:#?}", e));
            }
        }
        result
    }

    // Method to update a field's value
    pub fn update_field(
        &mut self,
        node_id: &str,
        field_template_id: &str,
        new_value: String,
    ) -> GraphResult<()> {
        if let Some(node) = self.node_instances.get_mut(node_id) {
            // Check if node allows field modifications
            if !node.can_modify_fields {
                return Err(GraphError::Other("Node fields are locked".to_string()));
            }

            // Find the field template to validate the value
            let template = self
                .node_templates
                .get(&node.template_id)
                .ok_or(GraphError::Other("Node template not found".to_string()))?;

            let field_template = template
                .field_templates
                .iter()
                .find(|ft| ft.id == field_template_id)
                .ok_or(GraphError::Other("Field template not found".to_string()))?;

            // Validate the value based on field type
            match field_template.field_type {
                FieldType::Boolean => {
                    if new_value != "true" && new_value != "false" {
                        return Err(GraphError::Other("Invalid boolean value".to_string()));
                    }
                }
                FieldType::Integer => {
                    if new_value.parse::<i32>().is_err() {
                        return Err(GraphError::Other("Invalid integer value".to_string()));
                    }
                }
                FieldType::String => {
                    // All string values are valid
                }
            }

            // Update the field value
            if let Some(field) = node
                .fields
                .iter_mut()
                .find(|f| f.field_template_id == field_template_id)
            {
                if !field.can_modify {
                    return Err(GraphError::Other("Field is locked".to_string()));
                }
                field.value = new_value;
                return Ok(());
            }

            return Err(GraphError::Other(
                "Field not found in node instance".to_string(),
            ));
        }

        Err(GraphError::Other("Node instance not found".to_string()))
    }
}
