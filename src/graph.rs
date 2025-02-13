use crate::{
    error,
    errors::{GraphError, GraphResult},
};
use std::collections::HashMap;

use crate::{
    common::generate_id,
    events::{EventSystem, SystemEvent},
};

pub trait NodeTemplateInfo {
    fn get_slot_template(&self, slot_id: &str) -> Option<&SlotTemplate>;
}

// Template definitions
#[derive(Debug, Clone)]
pub struct NodeTemplate {
    pub template_id: String,
    pub name: String,
    pub slot_templates: Vec<SlotTemplate>,
    // Visual defaults could go here
    pub default_width: f64,
    pub default_height: f64,
}

impl NodeTemplateInfo for NodeTemplate {
    fn get_slot_template(&self, slot_id: &str) -> Option<&SlotTemplate> {
        self.slot_templates.iter().find(|st| st.id == slot_id)
    }
}

#[derive(Debug, Clone)]
pub struct SlotTemplate {
    pub id: String,
    pub name: String,
    pub position: SlotPosition,
    pub slot_type: SlotType,
    pub allowed_connections: Vec<String>, // template_ids that can connect here
    pub min_connections: usize,
    pub max_connections: usize,
}

// Instance definitions
#[derive(Debug, Clone)]
pub struct NodeInstance {
    pub instance_id: String,
    pub template_id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub slots: Vec<SlotInstance>,
}

impl NodeInstance {
    pub fn new(template: &NodeTemplate, instance_id: String, x: f64, y: f64) -> Self {
        let mut slots = template
            .slot_templates
            .iter()
            .map(|st| SlotInstance {
                id: st.id.clone(),
                slot_template_id: st.id.clone(),
                node_template_id: template.template_id.clone(),
                connections: Vec::new(),
            })
            .collect::<Vec<_>>();

        // Add the standard incoming slot
        slots.push(SlotInstance {
            id: "incoming".to_string(),
            node_template_id: template.template_id.clone(),
            slot_template_id: "incoming".to_string(),
            connections: Vec::new(),
        });

        Self {
            instance_id,
            template_id: template.template_id.clone(),
            x,
            y,
            width: template.default_width,
            height: template.default_height,
            slots,
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
}
impl<'a> NodeCapabilities<'a> {
    pub fn new(template: &'a NodeTemplate, instance: &'a NodeInstance) -> Self {
        Self { template, instance }
    }
}

#[derive(Debug, Clone)]
pub struct SlotInstance {
    pub id: String,
    pub slot_template_id: String,
    pub node_template_id: String,
    pub connections: Vec<Connection>,
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
pub struct SlotCapabilities<'a> {
    pub template: &'a SlotTemplate,
    pub instance: &'a SlotInstance,
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub host_node_id: String,
    pub host_slot_id: String,
    pub target_node_id: String,
    pub target_slot_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SlotPosition {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SlotType {
    Incoming,
    Outgoing,
}

pub struct Graph {
    pub(crate) node_templates: HashMap<String, NodeTemplate>,
    pub(crate) node_instances: HashMap<String, NodeInstance>,
}

impl Graph {
    pub fn new() -> Self {
        Graph {
            node_templates: HashMap::new(),
            node_instances: HashMap::new(),
        }
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
            max_connections: 10000,
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

    pub fn create_instance(&mut self, node_template_id: &str, x: f64, y: f64) -> Option<String> {
        let template = self.node_templates.get(node_template_id)?;

        let instance_id = generate_id();
        let instance = NodeInstance {
            instance_id: instance_id.clone(),
            template_id: node_template_id.to_string(),
            x,
            y,
            width: template.default_width,
            height: template.default_height,
            slots: template
                .slot_templates
                .iter()
                .map(|slot_template| SlotInstance {
                    id: generate_id(),
                    node_template_id: node_template_id.to_string(),
                    slot_template_id: slot_template.id.clone(),
                    connections: Vec::new(),
                })
                .collect(),
        };

        self.node_instances.insert(instance_id.clone(), instance);
        Some(instance_id)
    }

    pub fn get_node_capabilities(&self, instance_id: &str) -> Option<NodeCapabilities> {
        let instance = self.node_instances.get(instance_id)?;
        let template = self.node_templates.get(&instance.template_id)?;
        Some(NodeCapabilities { template, instance })
    }
    pub fn get_slot_capabilities(
        &self,
        node_instance_id: &str,
        slot_id: &str,
    ) -> Option<SlotCapabilities> {
        let instance = self.node_instances.get(node_instance_id)?;
        let slot = instance.slots.iter().find(|s| s.id == slot_id)?;
        let template = self.node_templates.get(&slot.node_template_id)?;
        let slot_template = template.get_slot_template(&slot.slot_template_id)?;
        Some(SlotCapabilities {
            template: slot_template,
            instance: slot,
        })
    }
    pub fn is_valid_connection(&self, connection: Connection) -> bool {
        // Check if connection is valid based on templates
        // Implementation would check slot types, allowed connections, and current cardinality

        let Connection {
            host_node_id,
            host_slot_id,
            target_node_id,
            target_slot_id,
        } = connection;
        let from_slot_cap = self
            .get_slot_capabilities(&host_node_id, &host_slot_id)
            .unwrap();
        let target_node_cap = self.get_node_capabilities(&target_node_id).unwrap();
        if from_slot_cap
            .template
            .allowed_connections
            .contains(&target_node_cap.template.template_id)
            && from_slot_cap.instance.connections.len() < from_slot_cap.template.max_connections
            && !from_slot_cap.instance.connections.iter().any(|connection| {
                connection.target_node_id == target_node_id
                    && connection.target_slot_id == target_slot_id
            })
        {
            return true;
        }
        false
    }

    pub fn connect_slots(
        &mut self,
        connection: Connection,
        events: &EventSystem,
    ) -> GraphResult<()> {
        let Connection {
            host_node_id,
            host_slot_id,
            ..
        } = connection.clone();
        if !self.is_valid_connection(connection.clone()) {
            return Err(GraphError::InvalidConnection {
                connection,
                reason: "Connection Validation Failed".to_string(),
            });
        }

        // Add connection to both slots
        if let Some(from_instance) = self.node_instances.get_mut(&host_node_id) {
            if let Some(slot) = from_instance
                .slots
                .iter_mut()
                .find(|s| s.id == host_slot_id)
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
                        || slot.connections.len() > slot_template.max_connections
                    {
                        return false;
                    }
                    // Additional validation could go here
                }
            }
        }
        true
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
            host_slot_id,
            target_node_id,
            target_slot_id,
        } = connection;
        if let Some(from_instance) = self.node_instances.get_mut(host_node_id) {
            if let Some(slot) = from_instance
                .slots
                .iter_mut()
                .find(|s| s.id == *host_slot_id)
            {
                slot.connections.retain(|c| {
                    !(c.target_node_id == *target_node_id && c.target_slot_id == *target_slot_id)
                });
            }
        }

        if let Some(to_instance) = self.node_instances.get_mut(target_node_id) {
            if let Some(slot) = to_instance
                .slots
                .iter_mut()
                .find(|s| s.id == *target_slot_id)
            {
                slot.connections.retain(|c| {
                    !(c.target_node_id == *host_node_id && c.target_slot_id == *host_slot_id)
                });
            }
        }

        Ok(())
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
    DeleteSlotConnections { node_id: String, slot_id: String },
    CreateConnection(Connection),
    CreateNode { template_id: String, x: f64, y: f64 },
}

impl Graph {
    pub fn execute_command(
        &mut self,
        command: GraphCommand,
        events: &EventSystem,
    ) -> GraphResult<()> {
        let result = match command.clone() {
            GraphCommand::DeleteNode(node_id) => {
                // First remove all connections from this node
                let connections_to_remove = self.get_node_connections(&node_id);
                for conn in connections_to_remove {
                    self.delete_connection(&conn)?;
                }
                // Then remove all connections to this node
                let connections_to_remove =
                    self.node_instances.values().fold(vec![], |mut agg, inst| {
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
                for conn in connections_to_remove {
                    self.delete_connection(&conn)?;
                }

                // Then remove the node
                self.node_instances.remove(&node_id);
                events.emit(SystemEvent::CommandExecuted(command));
                Ok(())
            }
            GraphCommand::DeleteConnection(conn) => {
                self.delete_connection(&conn)?;
                events.emit(SystemEvent::CommandExecuted(command));
                Ok(())
            }
            GraphCommand::DeleteSlotConnections { node_id, slot_id } => {
                if let Some(instance) = self.node_instances.get_mut(&node_id) {
                    if let Some(slot) = instance.slots.iter_mut().find(|s| s.id == slot_id) {
                        slot.connections.clear();
                        events.emit(SystemEvent::CommandExecuted(command));
                        return Ok(());
                    }
                }
                Err(GraphError::SlotNotFound {
                    node_id: node_id.clone(),
                    slot_id: slot_id.clone(),
                })
            }
            GraphCommand::CreateConnection(connection) => {
                self.connect_slots(connection, events)?;
                events.emit(SystemEvent::CommandExecuted(command));
                Ok(())
            }
            GraphCommand::CreateNode { template_id, x, y } => {
                let result = self.create_instance(&template_id, x, y);
                match result {
                    Some(node_id) => {
                        events.emit(SystemEvent::CommandExecuted(command));
                        Ok(())
                    }
                    None => Err(GraphError::NodeCreationFailed(template_id)),
                }
            }
        };
        match &result {
            Ok(_) => {}
            Err(e) => {
                error(&format!("Command failed: {:#}", e));
            }
        }
        result
    }
}
