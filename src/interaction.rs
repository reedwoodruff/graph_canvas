use wasm_bindgen::prelude::*;

use crate::{
    draw::SLOT_DRAW_RADIUS,
    errors::{log_and_convert_error, GraphError, GraphResult},
    events::{EventSystem, SystemEvent},
    graph::{Connection, Graph, GraphCommand, NodeInstance, SlotInstance},
    GraphCanvas,
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
        self.interaction_state.click_initiated_on_node = None;
        self.interaction_state.click_initiated_on_slot = None;
    }
}

impl<'a> Drop for DragStateResetter<'a> {
    fn drop(&mut self) {
        self.reset_now();
    }
}

pub struct InteractionState {
    pub is_mouse_down: bool,
    pub click_initiated_on_node: Option<String>,
    pub click_initiated_on_slot: Option<(String, String)>,
    pub is_dragging_node: bool,
    pub connection_drag: Option<ConnectionDragInfo>,
    pub context_menu: Option<ContextMenu>,
    pub mode: InteractionMode,
    pub current_node_template_name: String,
    pub is_panning: bool,
    pub view_transform: ViewTransform,
}
pub struct ViewTransform {
    pub pan_x: f64,
    pub pan_y: f64,
}
impl ViewTransform {
    // Convert screen coordinates to graph coordinates
    pub fn screen_to_graph(&self, x: f64, y: f64) -> (f64, f64) {
        (x - self.pan_x, y - self.pan_y)
    }

    #[allow(dead_code)]
    // Convert graph coordinates to screen coordinates
    pub fn graph_to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        (x + self.pan_x, y + self.pan_y)
    }
}
impl InteractionState {
    pub fn new(graph: &Graph) -> Self {
        Self {
            // selected_element: None,
            is_mouse_down: false,
            click_initiated_on_node: None,
            click_initiated_on_slot: None,
            is_dragging_node: false,
            context_menu: None,
            connection_drag: None,
            mode: InteractionMode::Default,
            current_node_template_name: graph.node_templates.values().next().unwrap().name.clone(),
            is_panning: false,
            view_transform: ViewTransform {
                pan_x: 0.0,
                pan_y: 0.0,
            },
        }
    }
}
pub enum InteractionMode {
    Default,
    Pan,
    AddNode,
}

#[derive(Clone)]
pub struct ConnectionDragInfo {
    pub from_node: String,
    pub from_slot: String,
    pub current_x: f64,
    pub current_y: f64,
}

pub struct ContextMenu {
    pub x: f64,
    pub y: f64,
    pub target_type: ContextMenuTarget,
    pub items: Vec<ContextMenuItem>,
}
#[derive(Clone, Debug)]
pub enum ContextMenuTarget {
    Node(String),
    // from_node
    Connection(Connection),
    Slot { node_id: String, slot_id: String },
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
    pub bounds: Option<Rectangle>,
}

#[derive(Clone)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rectangle {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

#[derive(Clone)]
pub enum ContextMenuAction {
    Delete,
    DeleteAllSlotConnections,
}
#[wasm_bindgen]
impl GraphCanvas {
    pub fn handle_mouse_down(&self, screen_x: f64, screen_y: f64) -> Result<(), JsValue> {
        let mut ix = self
            .interaction
            .lock()
            .map_err(|e| log_and_convert_error(e))?;
        let mut graph = self.graph.lock().map_err(|e| log_and_convert_error(e))?;
        let events = self.events.lock().map_err(|e| log_and_convert_error(e))?;

        let (graph_x, graph_y) = ix.view_transform.screen_to_graph(screen_x, screen_y);
        match ix.mode {
            InteractionMode::Default => self
                .internal_pointer_handle_mouse_down(graph_x, graph_y, &mut graph, &mut ix, &events)
                .map_err(|e| log_and_convert_error(e))?,
            InteractionMode::Pan => self
                .internal_pan_handle_mouse_down(graph_x, graph_y, &mut graph, &mut ix, &events)
                .map_err(|e| log_and_convert_error(e))?,
            InteractionMode::AddNode => self
                .internal_add_node_handle_mouse_down(graph_x, graph_y, &mut graph, &mut ix, &events)
                .map_err(|e| log_and_convert_error(e))?,
        }
        Ok(())
    }

    pub fn handle_mouse_move(
        &self,
        screen_x: f64,
        screen_y: f64,
        dx: f64,
        dy: f64,
    ) -> Result<(), JsValue> {
        let mut ix = self
            .interaction
            .lock()
            .map_err(|e| log_and_convert_error(e))?;
        let mut graph = self.graph.lock().map_err(|e| log_and_convert_error(e))?;
        let events = self.events.lock().map_err(|e| log_and_convert_error(e))?;

        let (graph_x, graph_y) = ix.view_transform.screen_to_graph(screen_x, screen_y);

        match ix.mode {
            InteractionMode::Default => self
                .internal_pointer_handle_mouse_move(
                    graph_x, graph_y, dx, dy, &mut graph, &mut ix, &events,
                )
                .map_err(|e| log_and_convert_error(e))?,
            InteractionMode::Pan => self
                .internal_pan_handle_mouse_move(
                    graph_x, graph_y, dx, dy, &mut graph, &mut ix, &events,
                )
                .map_err(|e| log_and_convert_error(e))?,
            InteractionMode::AddNode => self
                .internal_add_node_handle_mouse_move(
                    graph_x, graph_y, dx, dy, &mut graph, &mut ix, &events,
                )
                .map_err(|e| log_and_convert_error(e))?,
        }
        Ok(())
    }

    pub fn handle_mouse_up(&self, screen_x: f64, screen_y: f64) -> Result<(), JsValue> {
        let mut ix = self
            .interaction
            .lock()
            .map_err(|e| log_and_convert_error(e))?;
        let mut graph = self.graph.lock().map_err(|e| log_and_convert_error(e))?;
        let events = self.events.lock().map_err(|e| log_and_convert_error(e))?;

        let (graph_x, graph_y) = ix.view_transform.screen_to_graph(screen_x, screen_y);
        match ix.mode {
            InteractionMode::Default => self
                .internal_pointer_handle_mouse_up(graph_x, graph_y, &mut graph, &mut ix, &events)
                .map_err(|e| log_and_convert_error(e))?,
            InteractionMode::Pan => self
                .internal_pan_handle_mouse_up(graph_x, graph_y, &mut graph, &mut ix, &events)
                .map_err(|e| log_and_convert_error(e))?,
            InteractionMode::AddNode => self
                .internal_add_node_handle_mouse_up(graph_x, graph_y, &mut graph, &mut ix, &events)
                .map_err(|e| log_and_convert_error(e))?,
        }
        Ok(())
    }

    fn handle_context_menu_action(
        &self,
        action: ContextMenuAction,
        target: &ContextMenuTarget,
        graph: &mut Graph,
        events: &EventSystem,
    ) -> GraphResult<()> {
        match (action, target) {
            (ContextMenuAction::Delete, ContextMenuTarget::Node(node_id)) => {
                graph.execute_command(GraphCommand::DeleteNode(node_id.clone()), events)?;
            }
            (ContextMenuAction::Delete, ContextMenuTarget::Connection(connection)) => {
                graph
                    .execute_command(GraphCommand::DeleteConnection(connection.clone()), events)?;
            }
            (
                ContextMenuAction::DeleteAllSlotConnections,
                ContextMenuTarget::Slot { node_id, slot_id },
            ) => {
                graph.execute_command(
                    GraphCommand::DeleteSlotConnections {
                        node_id: node_id.clone(),
                        slot_id: slot_id.clone(),
                    },
                    events,
                )?;
            }
            _ => {
                todo!();
            }
        }
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
        let (slot_x, slot_y) = self.calculate_slot_position(&capa.template, node, graph);
        let radius = SLOT_DRAW_RADIUS; // Same as drawing radius

        let dx = x - slot_x;
        let dy = y - slot_y;
        dx * dx + dy * dy <= radius * radius
    }
}

#[allow(unused_variables)]
impl GraphCanvas {
    /// A helper method to set the interaction mode.
    pub fn set_interaction_mode(&self, mode: InteractionMode) {
        self.interaction.lock().unwrap().mode = mode;
    }

    /// Update the node template that should be added in AddNode mode.
    pub fn set_current_node_template(&self, template: &str) {
        self.interaction.lock().unwrap().current_node_template_name = template.to_string();
    }
    fn internal_pointer_handle_mouse_down(
        &self,
        x: f64,
        y: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        _events: &EventSystem,
    ) -> GraphResult<()> {
        ix.is_mouse_down = true;

        // Check if we clicked on a slot
        for (node_id, node) in &graph.node_instances {
            for slot in &node.slots {
                if self.is_point_in_slot(x, y, node, slot, &graph) {
                    ix.click_initiated_on_slot = Some((node_id.clone(), slot.id.clone()));
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
                ix.click_initiated_on_node = Some(id.clone());
                return Ok(());
            }
        }
        ix.click_initiated_on_node = None;
        ix.click_initiated_on_slot = None;
        Ok(())
    }

    fn internal_pointer_handle_mouse_move(
        &self,
        x: f64,
        y: f64,
        _dx: f64,
        _dy: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        if ix.is_mouse_down
            && ix.click_initiated_on_node.is_some()
            && ix.connection_drag.is_none()
            && ix.is_dragging_node == false
        {
            if ix.context_menu.is_some() {
                ix.context_menu = None;
                events.emit(SystemEvent::ContextMenuClosed);
            }
            ix.is_dragging_node = true;
        }
        if ix.is_mouse_down && ix.click_initiated_on_slot.is_some() && ix.connection_drag.is_none()
        {
            if ix.context_menu.is_some() {
                ix.context_menu = None;
                events.emit(SystemEvent::ContextMenuClosed);
            }
            let (node_id, slot_id) = ix.click_initiated_on_slot.clone().unwrap();
            let slot = graph
                .node_instances
                .get(&node_id)
                .unwrap()
                .slots
                .iter()
                .find(|s| s.id == slot_id)
                .unwrap();
            ix.connection_drag = Some(ConnectionDragInfo {
                from_node: node_id.clone(),
                from_slot: slot_id,
                current_x: x,
                current_y: y,
            });
            events.emit(SystemEvent::ConnectionStarted {
                node: node_id.clone(),
                slot: slot.id.clone(),
            });
        }

        if let Some(connection_drag) = &mut ix.connection_drag {
            connection_drag.current_x = x;
            connection_drag.current_y = y;
        }
        if ix.is_dragging_node {
            if let Some(ref selected_id) = ix.click_initiated_on_node.clone() {
                if let Some(instance) = graph.node_instances.get_mut(selected_id) {
                    instance.x = x - instance.width / 2.0;
                    instance.y = y - instance.height / 2.0;
                }
            }
        }

        Ok(())
    }
    fn internal_pointer_handle_mouse_up(
        &self,
        x: f64,
        y: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        ix.is_mouse_down = false;

        // If we were creating a connection
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
                            Connection {
                                host_node_id: connection_drag.from_node.clone(),
                                host_slot_id: connection_drag.from_slot.clone(),
                                target_node_id,
                                target_slot_id: "incoming".to_string(),
                            },
                            &events,
                        )?;
                    }
                }
            }
        }
        // if we were dragging a node
        else if ix.is_dragging_node && ix.click_initiated_on_node.is_some() {
            if let Some(moved_node) = &ix.click_initiated_on_node {
                events.emit(SystemEvent::NodeMoved {
                    node: moved_node.clone(),
                    x: x.clone(),
                    y: y.clone(),
                });
            }
            ix.is_dragging_node = false;
            ix.click_initiated_on_node = None;
        } else if !ix.is_dragging_node {
            // If context menu is open and the click was within the menu
            if let Some(menu) = &ix.context_menu {
                if x >= menu.x
                    && x <= menu.x + self.config.context_menu_size.0
                    && y >= menu.y
                    && y <= menu.y + self.config.context_menu_size.1
                {
                    // if the click was on a menu item, handle the action
                    for item in &menu.items {
                        if let Some(bounds) = &item.bounds {
                            if bounds.contains(x, y) {
                                // Handle the action
                                self.handle_context_menu_action(
                                    item.action.clone(),
                                    &menu.target_type,
                                    graph,
                                    &events,
                                )?;
                                // Close menu after action
                                ix.context_menu = None;
                                events.emit(SystemEvent::ContextMenuClosed);
                                return Ok(());
                            }
                        }
                    }

                    // If it was not on a menu-item, do nothing
                    return Ok(());
                }
            }

            //
            for (instance_id, instance) in graph.node_instances.iter() {
                // Check Slots
                for slot in &instance.slots {
                    if self.is_point_in_slot(x, y, instance, slot, &graph) {
                        let context_target = ContextMenuTarget::Slot {
                            node_id: instance_id.clone(),
                            slot_id: slot.id.clone(),
                        };
                        ix.context_menu = Some(ContextMenu {
                            x,
                            y,
                            target_type: context_target.clone(),
                            items: vec![],
                        });
                        events.emit(SystemEvent::ContextMenuOpened(context_target));
                        return Ok(());
                    }
                }

                // Check Nodes
                if x >= instance.x
                    && x <= instance.x + instance.width
                    && y >= instance.y
                    && y <= instance.y + instance.height
                {
                    let context_target = ContextMenuTarget::Node(instance_id.clone());
                    ix.context_menu = Some(ContextMenu {
                        x,
                        y,
                        target_type: context_target.clone(),
                        items: vec![],
                    });
                    events.emit(SystemEvent::ContextMenuOpened(context_target));
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
                                let start_node_template =
                                    graph.node_templates.get(&instance.template_id).unwrap();
                                let start_slot_template = start_node_template
                                    .slot_templates
                                    .iter()
                                    .find(|t| t.id == slot.slot_template_id)
                                    .unwrap();
                                let end_node_template = graph
                                    .node_templates
                                    .get(&target_instance.template_id)
                                    .unwrap();
                                let end_slot_template = end_node_template
                                    .slot_templates
                                    .iter()
                                    .find(|t| t.id == target_slot.slot_template_id)
                                    .unwrap();
                                let (start_x, start_y) = self.calculate_slot_position(
                                    &start_slot_template,
                                    instance,
                                    graph,
                                );
                                let (end_x, end_y) = self.calculate_slot_position(
                                    &end_slot_template,
                                    target_instance,
                                    graph,
                                );

                                let distance = self.distance_to_bezier_curve(
                                    (x, y),
                                    (start_x, start_y),
                                    (end_x, end_y),
                                    50.0, // control_distance, same as used in draw_connection
                                    &start_slot_template.position,
                                    &end_slot_template.position,
                                );

                                if distance < 5.0 {
                                    let context_target =
                                        ContextMenuTarget::Connection(Connection {
                                            host_node_id: instance.instance_id.clone(),
                                            host_slot_id: slot.id.clone(),
                                            target_node_id: target_instance.instance_id.clone(),
                                            target_slot_id: "incoming".to_string(),
                                        });
                                    ix.context_menu = Some(ContextMenu {
                                        x,
                                        y,
                                        target_type: context_target.clone(),
                                        items: vec![],
                                    });
                                    events.emit(SystemEvent::ContextMenuOpened(context_target));
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
        ix.is_dragging_node = false;

        if ix.context_menu.is_some() {
            ix.context_menu = None;
            events.emit(SystemEvent::ContextMenuClosed);
        }

        Ok(())
    }
    fn internal_pan_handle_mouse_down(
        &self,
        x: f64,
        y: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        ix.is_panning = true;
        Ok(())
    }
    fn internal_pan_handle_mouse_move(
        &self,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        if ix.is_panning {
            ix.view_transform.pan_x += dx;
            ix.view_transform.pan_y += dy;
        }
        Ok(())
    }
    fn internal_pan_handle_mouse_up(
        &self,
        x: f64,
        y: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        ix.is_panning = false;
        Ok(())
    }
    fn internal_add_node_handle_mouse_down(
        &self,
        x: f64,
        y: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        let template_id = graph
            .get_node_template_by_name(&ix.current_node_template_name)
            .ok_or(GraphError::TemplateNotFound(
                ix.current_node_template_name.clone(),
            ))?
            .template_id;
        graph.execute_command(
            GraphCommand::CreateNode {
                template_id: template_id,
                x,
                y,
            },
            events,
        )?;
        Ok(())
    }
    fn internal_add_node_handle_mouse_move(
        &self,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,

        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        Ok(())
    }
    fn internal_add_node_handle_mouse_up(
        &self,
        x: f64,
        y: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        Ok(())
    }
}
