use wasm_bindgen::prelude::*;

use crate::{
    draw::SLOT_DRAW_RADIUS,
    graph::{Graph, NodeInstance, SlotInstance},
    log, GraphCanvas,
};

struct DragStateResetter<'a> {
    interaction_state: &'a mut InteractionState,
    graph: &'a mut Graph,
}
impl<'a> DragStateResetter<'a> {
    // Create a new resetter
    pub fn new(ix_state: &'a mut InteractionState, graph: &'a mut Graph) -> Self {
        DragStateResetter {
            interaction_state: ix_state,
            graph,
        }
    }

    // Manually reset state (though Drop will do this automatically)
    pub fn reset_now(&mut self) {
        self.interaction_state.connection_drag = None;
        self.interaction_state.is_dragging_node = false;
    }
}

impl<'a> Drop for DragStateResetter<'a> {
    fn drop(&mut self) {
        self.reset_now();
    }
}

pub struct InteractionState {
    pub is_mouse_down: bool,
    pub mouse_down_on_node: Option<String>,
    pub is_dragging_node: bool,
    pub connection_drag: Option<ConnectionDragInfo>,
    pub context_menu: Option<ContextMenu>,
    pub selected_element: Option<SelectedElement>,
}
impl InteractionState {
    pub fn new() -> Self {
        Self {
            selected_element: None,
            is_mouse_down: false,
            mouse_down_on_node: None,
            is_dragging_node: false,
            context_menu: None,
            connection_drag: None,
        }
    }
}

#[derive(Clone)]
pub struct ConnectionDragInfo {
    pub from_node: String,
    pub from_slot: String,
    pub current_x: f64,
    pub current_y: f64,
}

pub enum SelectedElement {
    Node(String),
    Slot {
        node_id: String,
        slot_id: String,
    },
    Connection {
        from_node: String,
        from_slot: String,
        to_node: String,
        to_slot: String,
    },
}

pub struct ContextMenu {
    pub x: f64,
    pub y: f64,
    pub target_type: ContextMenuTarget,
}
pub enum ContextMenuTarget {
    Node(String),
    // from_node
    Connection {
        from_node: String,
        from_slot: String,
        to_node: String,
        to_slot: String,
    },
    Slot {
        node_id: String,
        slot_id: String,
    },
}
impl ContextMenuTarget {
    pub fn get_title(&self, graph: &Graph) -> String {
        match self {
            ContextMenuTarget::Node(node_id) => {
                if let Some(node) = graph.node_instances.get(node_id) {
                    if let Some(template) = graph.node_templates.get(&node.template_id) {
                        format!("Node: {}", template.name)
                    } else {
                        "Unknown Node".to_string()
                    }
                } else {
                    "Unknown Node".to_string()
                }
            }
            ContextMenuTarget::Connection { .. } => "Connection".to_string(),
            ContextMenuTarget::Slot { node_id, slot_id } => {
                if let Some(node) = graph.node_instances.get(node_id) {
                    if let Some(slot) = node.slots.iter().find(|s| s.id == *slot_id) {
                        if let Some(template) = graph.node_templates.get(&node.template_id) {
                            if let Some(slot_template) = template
                                .slot_templates
                                .iter()
                                .find(|t| t.id == slot.slot_template_id)
                            {
                                return format!("Slot: {}", slot_template.name);
                            }
                        }
                    }
                }
                "Unknown Slot".to_string()
            }
        }
    }
}
#[derive(Clone)]
pub struct ContextMenuItem {
    pub label: String,
    pub action: ContextMenuAction,
    pub color: String,
}

#[derive(Clone)]
pub enum ContextMenuAction {
    Delete,
    DeleteAllConnections,
}
#[wasm_bindgen]
impl GraphCanvas {
    pub fn handle_mouse_down(&self, x: f64, y: f64) -> Result<(), JsValue> {
        let graph = self.graph.lock().unwrap();
        let mut ix = self.interaction.lock().unwrap();
        ix.is_mouse_down = true;

        // Check if we clicked on a slot
        for (node_id, node) in &graph.node_instances {
            for slot in &node.slots {
                if self.is_point_in_slot(x, y, node, slot, &graph) {
                    ix.connection_drag = Some(ConnectionDragInfo {
                        from_node: node_id.clone(),
                        from_slot: slot.id.clone(),
                        current_x: x,
                        current_y: y,
                    });
                    return Ok(());
                }
            }
        }
        // Check if clicked on a node
        for (id, instance) in graph.node_instances.iter() {
            if x >= instance.x
                && x <= instance.x + instance.width
                && y >= instance.y
                && y <= instance.y + instance.height
            {
                ix.mouse_down_on_node = Some(id.clone());
                return Ok(());
            }
        }
        ix.mouse_down_on_node = None;
        Ok(())
    }

    pub fn handle_mouse_move(&self, x: f64, y: f64) -> Result<(), JsValue> {
        // let mut drag_state = self
        //     .connection_drag_state
        //     .lock()
        //     .map_err(|e| JsValue::from_str(&format!("Failed to lock drag_state: {}", e)))?;

        let mut ix = self.interaction.lock().unwrap();
        let mut graph = self.graph.lock().unwrap();
        if ix.is_mouse_down
            && ix.mouse_down_on_node.is_some()
            && ix.connection_drag.is_none()
            && ix.is_dragging_node == false
        {
            ix.context_menu = None;
            ix.is_dragging_node = true;
        }
        if ix.connection_drag.is_some() && ix.context_menu.is_some() {
            ix.context_menu = None;
        }

        if let Some(connection_drag) = &mut ix.connection_drag {
            connection_drag.current_x = x;
            connection_drag.current_y = y;
        }
        if ix.is_dragging_node {
            if let Some(ref selected_id) = ix.mouse_down_on_node.clone() {
                if let Some(instance) = graph.node_instances.get_mut(selected_id) {
                    instance.x = x - instance.width / 2.0;
                    instance.y = y - instance.height / 2.0;
                }
            }
        }

        Ok(())
    }

    pub fn handle_mouse_up(&self, x: f64, y: f64) -> Result<(), JsValue> {
        let mut ix = self.interaction.lock().unwrap();
        let mut graph = self.graph.lock().unwrap();

        ix.is_mouse_down = false;

        if ix.connection_drag.is_some() {
            let resetter = DragStateResetter::new(&mut *ix, &mut *graph);
            let connection_drag = resetter.interaction_state.connection_drag.clone().unwrap();
            // Check if we're over another node
            for (target_node_id, target_node) in resetter.graph.node_instances.clone().into_iter() {
                // Don't connect to self
                if target_node_id != connection_drag.from_node {
                    // Check if point is within node bounds
                    if x >= target_node.x
                        && x <= target_node.x + target_node.width
                        && y >= target_node.y
                        && y <= target_node.y + target_node.height
                    {
                        resetter.graph.connect_slots(
                            &connection_drag.from_node,
                            &connection_drag.from_slot,
                            &target_node_id,
                            &"incoming",
                        )?;
                    }
                }
            }
        } else if !ix.is_dragging_node {
            if let Some(context_menu) = &ix.context_menu {
                // If context menu is open and the click was within the menu, do nothing and return
                if x >= context_menu.x
                    && x <= context_menu.x + self.settings.context_menu_size.0
                    && y >= context_menu.y
                    && y <= context_menu.y + self.settings.context_menu_size.1
                {
                    return Ok(());
                }
            }
            for (instance_id, instance) in graph.node_instances.iter() {
                // Check Slots
                for slot in &instance.slots {
                    if self.is_point_in_slot(x, y, instance, slot, &graph) {
                        ix.context_menu = Some(ContextMenu {
                            x,
                            y,
                            target_type: ContextMenuTarget::Slot {
                                node_id: instance_id.clone(),
                                slot_id: slot.id.clone(),
                            },
                        });
                        return Ok(());
                    }
                }

                // Check Nodes
                if x >= instance.x
                    && x <= instance.x + instance.width
                    && y >= instance.y
                    && y <= instance.y + instance.height
                {
                    ix.context_menu = Some(ContextMenu {
                        x,
                        y,
                        target_type: ContextMenuTarget::Node(instance_id.clone()),
                    });
                    return Ok(());
                }
                // Check to see if the click was on a connection
                for slot in &instance.slots {
                    for connection in &slot.connections {
                        if let Some(target_instance) =
                            graph.node_instances.get(&connection.target_node_id)
                        {
                            if let Some(target_slot) = target_instance
                                .slots
                                .iter()
                                .find(|s| s.slot_template_id == connection.target_slot_id)
                            {
                                let (start_x, start_y) = self.calculate_slot_position(
                                    &graph
                                        .node_templates
                                        .get(&instance.template_id)
                                        .unwrap()
                                        .slot_templates
                                        .iter()
                                        .find(|t| t.id == slot.slot_template_id)
                                        .unwrap()
                                        .position,
                                    instance,
                                );
                                let (end_x, end_y) = self.calculate_slot_position(
                                    &graph
                                        .node_templates
                                        .get(&target_instance.template_id)
                                        .unwrap()
                                        .slot_templates
                                        .iter()
                                        .find(|t| t.id == target_slot.slot_template_id)
                                        .unwrap()
                                        .position,
                                    target_instance,
                                );

                                let distance = self.distance_to_bezier_curve(
                                    (x, y),
                                    (start_x, start_y),
                                    (end_x, end_y),
                                    50.0, // control_distance, same as used in draw_connection
                                );

                                if distance < 5.0 {
                                    ix.context_menu = Some(ContextMenu {
                                        x,
                                        y,
                                        target_type: ContextMenuTarget::Connection {
                                            from_node: instance.instance_id.clone(),
                                            from_slot: slot.id.clone(),
                                            to_node: target_instance.instance_id.clone(),
                                            to_slot: target_slot.id.clone(),
                                        },
                                    });
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
        ix.is_dragging_node = false;
        ix.context_menu = None;

        Ok(())
    }

    fn is_point_in_slot(
        &self,
        x: f64,
        y: f64,
        node: &NodeInstance,
        slot: &SlotInstance,
        graph: &Graph,
    ) -> bool {
        let capa = slot.capabilities(graph);
        let (slot_x, slot_y) = self.calculate_slot_position(&capa.template.position, node);
        let radius = SLOT_DRAW_RADIUS; // Same as drawing radius

        let dx = x - slot_x;
        let dy = y - slot_y;
        dx * dx + dy * dy <= radius * radius
    }
}
