use wasm_bindgen::prelude::*;

use crate::{
    errors::{log_and_convert_error, GraphError, GraphResult},
    events::{EventSystem, SystemEvent},
    graph::{Connection, Graph, GraphCommand, NodeInstance, SlotInstance},
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
        self.interaction_state.click_initiated_on_node = None;
        self.interaction_state.click_initiated_on_slot = None;
    }
}

impl Drop for DragStateResetter<'_> {
    fn drop(&mut self) {
        self.reset_now();
    }
}

pub struct InteractionState {
    pub is_mouse_down: bool,
    pub click_initiated_on_node: Option<String>,
    pub click_initiated_on_slot: Option<(String, String)>,
    pub currently_selected_node_instance: Option<String>,
    pub is_dragging_node: bool,
    pub connection_drag: Option<ConnectionDragInfo>,
    pub context_menu: Option<ContextMenu>,
    pub mode: InteractionMode,
    pub actively_creating_node_template_id: String,
    pub is_panning: bool,
    pub view_transform: ViewTransform,
    pub hovered_node: Option<String>,
    pub hovered_slot: Option<(String, String)>, // (node_id, slot_template_id)
    pub hovered_connection: Option<Connection>,
}
pub struct ViewTransform {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}
impl ViewTransform {
    // Convert screen coordinates to graph coordinates
    pub fn screen_to_graph(&self, x: f64, y: f64) -> (f64, f64) {
        // First apply pan, then account for zoom
        let center_x = x - self.pan_x;
        let center_y = y - self.pan_y;

        // Convert from screen to graph space (divide by zoom factor)
        (center_x / self.zoom, center_y / self.zoom)
    }

    #[allow(dead_code)]
    // Convert graph coordinates to screen coordinates
    pub fn graph_to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        // First apply zoom, then add pan
        let zoomed_x = x * self.zoom;
        let zoomed_y = y * self.zoom;

        (zoomed_x + self.pan_x, zoomed_y + self.pan_y)
    }
}
impl InteractionState {
    pub fn new() -> Self {
        Self {
            // selected_element: None,
            is_mouse_down: false,
            click_initiated_on_node: None,
            click_initiated_on_slot: None,
            currently_selected_node_instance: None,
            is_dragging_node: false,
            context_menu: None,
            connection_drag: None,
            mode: InteractionMode::Default,
            actively_creating_node_template_id: "Unset".to_string(),
            is_panning: false,
            view_transform: ViewTransform {
                pan_x: 0.0,
                pan_y: 0.0,
                zoom: 1.0,
            },
            hovered_node: None,
            hovered_slot: None,
            hovered_connection: None,
        }
    }
}
pub enum InteractionMode {
    Default,
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
    pub field_edit_value: Option<String>, // For field editing
}
#[derive(Clone, Debug)]
pub enum ContextMenuTarget {
    Node(String),
    // from_node
    Connection(Connection),
    Slot {
        node_id: String,
        slot_template_id: String,
    },
    Field {
        node_id: String,
        field_template_id: String,
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
            ContextMenuTarget::Slot {
                node_id,
                slot_template_id,
            } => {
                if let Some(node) = graph.node_instances.get(node_id) {
                    if let Some(slot) = node
                        .slots
                        .iter()
                        .find(|s| s.slot_template_id == *slot_template_id)
                    {
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
            ContextMenuTarget::Field {
                node_id,
                field_template_id,
            } => {
                if let Some(node) = graph.node_instances.get(node_id) {
                    if let Some(field) = node
                        .fields
                        .iter()
                        .find(|f| f.field_template_id == *field_template_id)
                    {
                        if let Some(template) = graph.node_templates.get(&node.template_id) {
                            if let Some(field_template) = template
                                .field_templates
                                .iter()
                                .find(|t| t.id == field.field_template_id)
                            {
                                return format!("Field: {}", field_template.name);
                            }
                        }
                    }
                }
                "Unknown Field".to_string()
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
    EditField,
    SetBooleanField(bool),
    SetIntegerField(i32),
    SetStringField(String),
}
#[wasm_bindgen]
impl GraphCanvas {
    pub(crate) fn handle_mouse_down(&self, screen_x: f64, screen_y: f64) -> Result<(), JsValue> {
        let mut ix = self.interaction.lock().map_err(log_and_convert_error)?;
        let mut graph = self.graph.lock().map_err(log_and_convert_error)?;
        let events = self.events.lock().map_err(log_and_convert_error)?;

        let (graph_x, graph_y) = ix.view_transform.screen_to_graph(screen_x, screen_y);
        match ix.mode {
            InteractionMode::Default => self
                .internal_pointer_handle_mouse_down(graph_x, graph_y, &mut graph, &mut ix, &events)
                .map_err(log_and_convert_error)?,
            InteractionMode::AddNode => self
                .internal_add_node_handle_mouse_down(graph_x, graph_y, &mut graph, &mut ix, &events)
                .map_err(log_and_convert_error)?,
        }
        Ok(())
    }

    pub(crate) fn handle_mouse_move(
        &self,
        screen_x: f64,
        screen_y: f64,
        dx: f64,
        dy: f64,
    ) -> Result<(), JsValue> {
        let mut ix = self.interaction.lock().map_err(log_and_convert_error)?;
        let mut graph = self.graph.lock().map_err(log_and_convert_error)?;
        let events = self.events.lock().map_err(log_and_convert_error)?;

        let (graph_x, graph_y) = ix.view_transform.screen_to_graph(screen_x, screen_y);

        match ix.mode {
            InteractionMode::Default => self
                .internal_pointer_handle_mouse_move(
                    graph_x, graph_y, dx, dy, &mut graph, &mut ix, &events,
                )
                .map_err(log_and_convert_error)?,
            InteractionMode::AddNode => self
                .internal_add_node_handle_mouse_move(
                    graph_x, graph_y, dx, dy, &mut graph, &mut ix, &events,
                )
                .map_err(log_and_convert_error)?,
        }
        Ok(())
    }

    pub(crate) fn handle_mouse_up(&self, screen_x: f64, screen_y: f64) -> Result<(), JsValue> {
        let mut ix = self.interaction.lock().map_err(log_and_convert_error)?;
        let mut graph = self.graph.lock().map_err(log_and_convert_error)?;
        let events = self.events.lock().map_err(log_and_convert_error)?;

        let (graph_x, graph_y) = ix.view_transform.screen_to_graph(screen_x, screen_y);
        match ix.mode {
            InteractionMode::Default => self
                .internal_pointer_handle_mouse_up(graph_x, graph_y, &mut graph, &mut ix, &events)
                .map_err(log_and_convert_error)?,
            InteractionMode::AddNode => self
                .internal_add_node_handle_mouse_up(graph_x, graph_y, &mut graph, &mut ix, &events)
                .map_err(log_and_convert_error)?,
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
                ContextMenuTarget::Slot {
                    node_id,
                    slot_template_id,
                },
            ) => {
                graph.execute_command(
                    GraphCommand::DeleteSlotConnections {
                        node_id: node_id.clone(),
                        slot_template_id: slot_template_id.clone(),
                    },
                    events,
                )?;
            }
            (
                ContextMenuAction::SetBooleanField(value),
                ContextMenuTarget::Field {
                    node_id,
                    field_template_id,
                },
            ) => {
                // For boolean fields, set to true or false
                let value_str = if value {
                    "true".to_string()
                } else {
                    "false".to_string()
                };
                graph.execute_command(
                    GraphCommand::UpdateField {
                        node_id: node_id.clone(),
                        field_template_id: field_template_id.clone(),
                        new_value: value_str,
                    },
                    events,
                )?;
            }
            (
                ContextMenuAction::SetIntegerField(value),
                ContextMenuTarget::Field {
                    node_id,
                    field_template_id,
                },
            ) => {
                // For integer fields, set to the specified value
                graph.execute_command(
                    GraphCommand::UpdateField {
                        node_id: node_id.clone(),
                        field_template_id: field_template_id.clone(),
                        new_value: value.to_string(),
                    },
                    events,
                )?;
            }
            (
                ContextMenuAction::SetStringField(value),
                ContextMenuTarget::Field {
                    node_id,
                    field_template_id,
                },
            ) => {
                // For string fields, set to the specified value
                graph.execute_command(
                    GraphCommand::UpdateField {
                        node_id: node_id.clone(),
                        field_template_id: field_template_id.clone(),
                        new_value: value,
                    },
                    events,
                )?;
            }
            (ContextMenuAction::EditField, ContextMenuTarget::Field { .. }) => {
                // For now, we'll handle this in the JavaScript side
                // by providing a UI prompt for text entry
                log("Edit field action needs to be implemented in JS");
            }
            _ => {
                log("Unhandled context menu action");
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
        let (slot_x, slot_y) = self.calculate_slot_position(capa.template, node, graph);
        let radius = self.config.slot_radius; // Same as drawing radius

        let dx = x - slot_x;
        let dy = y - slot_y;
        dx * dx + dy * dy <= radius * radius
    }

    fn is_point_on_connection(
        &self,
        graph: &Graph,
        connection: &Connection,
        x: f64,
        y: f64,
    ) -> GraphResult<bool> {
        if let Some(host_instance) = graph.node_instances.get(&connection.host_node_id) {
            if let Some(host_slot) = host_instance
                .slots
                .iter()
                .find(|s| s.slot_template_id == connection.host_slot_template_id)
            {
                if let Some(target_instance) = graph.node_instances.get(&connection.target_node_id)
                {
                    if let Some(target_slot) = target_instance
                        .slots
                        .iter()
                        .find(|s| s.slot_template_id == connection.target_slot_template_id)
                    {
                        let start_node_template = graph
                            .node_templates
                            .get(&host_instance.template_id)
                            .unwrap();
                        let start_slot_template = start_node_template
                            .slot_templates
                            .iter()
                            .find(|t| t.id == host_slot.slot_template_id)
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
                        let (start_x, start_y) =
                            self.calculate_slot_position(start_slot_template, host_instance, graph);
                        let (end_x, end_y) =
                            self.calculate_slot_position(end_slot_template, target_instance, graph);

                        let distance = self.distance_to_bezier_curve(
                            (x, y),
                            (start_x, start_y),
                            (end_x, end_y),
                            host_instance,
                            target_instance,
                        );

                        if distance < 5.0 {
                            return Ok(true);
                        }
                    }
                }
            }
        }
        Ok(false)
    }
}

#[allow(unused_variables)]
impl GraphCanvas {
    /// A helper method to set the interaction mode.
    pub fn set_interaction_mode(&self, mode: InteractionMode) {
        self.interaction.lock().unwrap().mode = mode;
    }

    /// Update the node template that should be added in AddNode mode.
    pub(crate) fn set_current_node_template(&self, template_id: &str) {
        self.interaction
            .lock()
            .unwrap()
            .actively_creating_node_template_id = template_id.to_string();
    }

    /// Handle zooming - called when the mousewheel is used
    pub(crate) fn handle_zoom(
        &self,
        delta: f64,
        screen_x: f64,
        screen_y: f64,
    ) -> Result<(), JsValue> {
        let mut ix = self.interaction.lock().map_err(log_and_convert_error)?;

        // Calculate zoom factor change based on wheel delta
        let zoom_speed = 0.1; // Adjust for faster/slower zooming
        let zoom_delta = if delta < 0.0 {
            1.0 + zoom_speed
        } else {
            1.0 - zoom_speed
        };

        // Calculate new zoom level with min/max constraints
        let new_zoom = (ix.view_transform.zoom * zoom_delta).max(0.1).min(5.0);

        // Get the point under the cursor in graph coordinates before zoom
        let (graph_x, graph_y) = ix.view_transform.screen_to_graph(screen_x, screen_y);

        // Update zoom
        ix.view_transform.zoom = new_zoom;

        // Get the new screen position of the same graph point after zoom
        let (new_screen_x, new_screen_y) = ix.view_transform.graph_to_screen(graph_x, graph_y);

        // Calculate the offset to keep the point under the cursor
        let dx = screen_x - new_screen_x;
        let dy = screen_y - new_screen_y;

        // Adjust pan to keep the point under the cursor
        ix.view_transform.pan_x += dx;
        ix.view_transform.pan_y += dy;

        // Save the view transform to the current view
        if let Ok(mut layout_engine) = self.layout_engine.try_lock() {
            layout_engine.save_view_transform(&ix);
        }

        Ok(())
    }
    fn internal_pointer_handle_mouse_down(
        &self,
        x: f64,
        y: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        if !self.config.is_mutable {
            return Ok(());
        }

        ix.is_mouse_down = true;

        // Check if we clicked on a slot
        for (node_id, node) in &graph.node_instances {
            for slot in &node.slots {
                if self.is_point_in_slot(x, y, node, slot, graph) {
                    ix.click_initiated_on_slot =
                        Some((node_id.clone(), slot.slot_template_id.clone()));
                    return Ok(());
                }
            }
        }
        // Check if clicked on a node
        for (id, instance) in graph.node_instances.iter() {
            // For circular nodes, check distance from center
            let center_x = instance.x + instance.radius;
            let center_y = instance.y + instance.radius;
            let radius = instance.radius;

            // Calculate distance from center of node
            let dx = x - center_x;
            let dy = y - center_y;
            let distance = (dx * dx + dy * dy).sqrt();

            if distance <= radius {
                ix.click_initiated_on_node = Some(id.clone());
                ix.currently_selected_node_instance = Some(id.clone());
                return Ok(());
            }
        }

        // If context menu is open and the click was within the menu
        // Handle calling the action if it was clicked
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
                                events,
                            )?;
                            // Close menu after action
                            ix.context_menu = None;
                            events.emit(SystemEvent::ContextMenuClosed);
                            return Ok(());
                        }
                    }
                }
                // If it was not on a menu-item but is in a menu, do nothing
                return Ok(());
            }
        }

        // Check to see if the click was on a connection
        for (instance_id, instance) in graph.node_instances.iter() {
            for slot in &instance.slots {
                for connection in &slot.connections {
                    if self.is_point_on_connection(graph, connection, x, y)? {
                        return Ok(());
                    }
                }
            }
        }

        // If we didn't click on any slot, menu, connection, or node, start panning
        if self.config.is_movable {
            ix.is_panning = true;
        }

        ix.click_initiated_on_node = None;
        ix.click_initiated_on_slot = None;
        ix.currently_selected_node_instance = None;
        Ok(())
    }

    fn internal_pointer_handle_mouse_move_hover(
        &self,
        x: f64,
        y: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
    ) -> GraphResult<()> {
        // Reset hover states
        ix.hovered_node = None;
        ix.hovered_slot = None;
        ix.hovered_connection = None;
        // Check for hovering over slots
        for (node_id, node) in &graph.node_instances {
            for slot in &node.slots {
                if self.is_point_in_slot(x, y, node, slot, graph) {
                    ix.hovered_slot = Some((node_id.clone(), slot.slot_template_id.clone()));
                    return Ok(());
                }
            }
        }

        // Check for hovering over nodes
        for (id, instance) in &graph.node_instances {
            // For circular nodes, check distance from center
            let center_x = instance.x + instance.radius;
            let center_y = instance.y + instance.radius;
            let radius = instance.radius;

            // Calculate distance from center of node
            let dx = x - center_x;
            let dy = y - center_y;
            let distance = (dx * dx + dy * dy).sqrt();

            if distance <= radius {
                ix.hovered_node = Some(id.clone());
                return Ok(());
            }
        }
        // Check for hovering over connections
        for (node_id, node) in &graph.node_instances {
            for slot in &node.slots {
                for connection in &slot.connections {
                    if self.is_point_on_connection(graph, connection, x, y)? {
                        ix.hovered_connection = Some(connection.clone());
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    fn internal_pointer_handle_mouse_move(
        &self,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        self.internal_pointer_handle_mouse_move_hover(x, y, graph, ix)?;

        if ix.is_mouse_down
            && ix.click_initiated_on_node.is_some()
            && ix.connection_drag.is_none()
            && !ix.is_dragging_node
        {
            if ix.context_menu.is_some() {
                ix.context_menu = None;
                events.emit(SystemEvent::ContextMenuClosed);
            }
            ix.is_dragging_node = true;

            // Start force simulation if the layout type is force directed
            if let Ok(mut layout_engine) = self.layout_engine.try_lock() {
                let node_id = ix.click_initiated_on_node.clone().unwrap();
                layout_engine.start_force_simulation(graph, &node_id);
            }
        }

        // Start connection drag
        if ix.is_mouse_down && ix.click_initiated_on_slot.is_some() && ix.connection_drag.is_none()
        {
            if ix.context_menu.is_some() {
                ix.context_menu = None;
                events.emit(SystemEvent::ContextMenuClosed);
            }
            let (node_id, slot_template_id) = ix.click_initiated_on_slot.clone().unwrap();
            let node_instance = graph.node_instances.get(&node_id);
            if let Some(node_instance) = node_instance {
                let slot = node_instance
                    .slots
                    .iter()
                    .find(|s| s.slot_template_id == slot_template_id)
                    .unwrap();
                ix.connection_drag = Some(ConnectionDragInfo {
                    from_node: node_id.clone(),
                    from_slot: slot_template_id,
                    current_x: x,
                    current_y: y,
                });
                events.emit(SystemEvent::ConnectionStarted {
                    node: node_id.clone(),
                    slot: slot.slot_template_id.clone(),
                });
            }
        }

        if let Some(connection_drag) = &mut ix.connection_drag {
            connection_drag.current_x = x;
            connection_drag.current_y = y;
        }
        if ix.is_dragging_node {
            if let Some(ref selected_id) = ix.click_initiated_on_node.clone() {
                if let Some(instance) = graph.node_instances.get_mut(selected_id) {
                    instance.x = x - instance.radius;
                    instance.y = y - instance.radius;

                    // Run a simulation step when in force directed mode
                    if let Ok(mut layout_engine) = self.layout_engine.try_lock() {
                        layout_engine.run_simulation_step(graph);
                    }
                }
            }
        }
        // Handle panning if enabled
        else if ix.is_panning {
            ix.view_transform.pan_x += dx;
            ix.view_transform.pan_y += dy;
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
        if !self.config.is_mutable {
            return Ok(());
        }
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
                        && x <= target_node.x + (target_node.radius * 2.0)
                        && y >= target_node.y
                        && y <= target_node.y + (target_node.radius * 2.0)
                    {
                        resetter.graph.connect_slots(
                            Connection {
                                host_node_id: connection_drag.from_node.clone(),
                                host_slot_template_id: connection_drag.from_slot.clone(),
                                target_node_id,
                                target_slot_template_id: "incoming".to_string(),
                                can_delete: true,
                            },
                            events,
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
                    x,
                    y,
                });
            }

            // Save current view transform to the view state
            if let Ok(mut layout_engine) = self.layout_engine.try_lock() {
                layout_engine.save_view_transform(ix);
            }

            ix.is_dragging_node = false;
            ix.click_initiated_on_node = None;
        } else if !ix.is_dragging_node {
            //
            for (instance_id, instance) in graph.node_instances.iter() {
                // Check Slots
                for slot in &instance.slots {
                    if self.is_point_in_slot(x, y, instance, slot, graph) {
                        let context_target = ContextMenuTarget::Slot {
                            node_id: instance_id.clone(),
                            slot_template_id: slot.slot_template_id.clone(),
                        };
                        ix.context_menu = Some(ContextMenu {
                            x,
                            y,
                            target_type: context_target.clone(),
                            items: vec![],
                            field_edit_value: None,
                        });
                        events.emit(SystemEvent::ContextMenuOpened(context_target));
                        return Ok(());
                    }
                }

                // Calculate node center and dimensions for later checks
                let center_x = instance.x + instance.radius;
                let center_y = instance.y + instance.radius;
                let radius = instance.radius;

                // Calculate distance from center of node
                let dx = x - center_x;
                let dy = y - center_y;
                let distance = (dx * dx + dy * dy).sqrt();

                // Check if we clicked within node radius
                if distance <= radius {
                    // Get the template to access field information
                    if let Some(template) = graph.node_templates.get(&instance.template_id) {
                        // If node has fields, check if we clicked on a field
                        if !instance.fields.is_empty() {
                            // Calculate field positions
                            let title_y = if !instance.fields.is_empty() {
                                center_y - (instance.fields.len() as f64 * 15.0) / 2.0 - 15.0
                            } else {
                                center_y
                            };

                            let mut y_offset = title_y + 20.0; // Start below the title

                            // Check each field to see if it was clicked
                            for field_instance in &instance.fields {
                                // Field click area is approx +/- 10px vertically from text center
                                if (y >= y_offset - 7.0) && (y <= y_offset + 7.0) {
                                    // Check horizontal distance - if within reasonable bounds of the text
                                    if distance <= radius * 0.8 {
                                        // Somewhat arbitrary, just to make sure we're near the field text
                                        // Get the field template for the menu title
                                        if let Some(field_template) = template
                                            .field_templates
                                            .iter()
                                            .find(|ft| ft.id == field_instance.field_template_id)
                                        {
                                            let context_target = ContextMenuTarget::Field {
                                                node_id: instance_id.clone(),
                                                field_template_id: field_instance
                                                    .field_template_id
                                                    .clone(),
                                            };
                                            ix.context_menu = Some(ContextMenu {
                                                x,
                                                y,
                                                target_type: context_target.clone(),
                                                items: vec![],
                                                field_edit_value: Some(
                                                    field_instance.value.clone(),
                                                ),
                                            });
                                            events.emit(SystemEvent::ContextMenuOpened(
                                                context_target,
                                            ));
                                            return Ok(());
                                        }
                                    }
                                }
                                y_offset += 15.0; // Move down for next field
                            }
                        }
                    }

                    // If we didn't click on a field but are within the node, open the node context menu
                    let context_target = ContextMenuTarget::Node(instance_id.clone());
                    ix.context_menu = Some(ContextMenu {
                        x,
                        y,
                        target_type: context_target.clone(),
                        items: vec![],
                        field_edit_value: None,
                    });
                    events.emit(SystemEvent::ContextMenuOpened(context_target));
                    return Ok(());
                }
            }
        }
        // Check to see if the click was on a connection
        for (instance_id, instance) in graph.node_instances.iter() {
            for slot in &instance.slots {
                for connection in &slot.connections {
                    if self.is_point_on_connection(graph, connection, x, y)? {
                        // let context_target = ContextMenuTarget::Connection(Connection {
                        //     host_node_id: instance.instance_id.clone(),
                        //     host_slot_template_id: slot.id.clone(),
                        //     target_node_id: connection.target_node_id.clone(),
                        //     target_slot_template_id: "incoming".to_string(),
                        // });
                        let context_target = ContextMenuTarget::Connection(connection.clone());
                        ix.context_menu = Some(ContextMenu {
                            x,
                            y,
                            target_type: context_target.clone(),
                            items: vec![],
                            field_edit_value: None,
                        });
                        events.emit(SystemEvent::ContextMenuOpened(context_target));
                        return Ok(());
                    }
                }
            }
        }
        // If we were dragging a node, stop any active simulation
        if ix.is_dragging_node {
            if let Ok(mut layout_engine) = self.layout_engine.try_lock() {
                layout_engine.stop_force_simulation();
            }
        }

        // If we were panning, save the view transform
        if ix.is_panning {
            if let Ok(mut layout_engine) = self.layout_engine.try_lock() {
                layout_engine.save_view_transform(ix);
            }
        }

        ix.is_dragging_node = false;
        ix.is_panning = false;

        if ix.context_menu.is_some() {
            ix.context_menu = None;
            events.emit(SystemEvent::ContextMenuClosed);
        }

        Ok(())
    }
    // Pan mode has been removed and integrated into Default mode
    fn internal_add_node_handle_mouse_down(
        &self,
        x: f64,
        y: f64,
        graph: &mut Graph,
        ix: &mut InteractionState,
        events: &EventSystem,
    ) -> GraphResult<()> {
        if !self.config.is_mutable {
            return Ok(());
        }
        let template_id = graph
            .node_templates
            .get(&ix.actively_creating_node_template_id)
            .ok_or(GraphError::TemplateNotFound(
                ix.actively_creating_node_template_id.clone(),
            ))?
            .template_id
            .clone();
        graph.execute_command(GraphCommand::CreateNode { template_id, x, y }, events)?;
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
