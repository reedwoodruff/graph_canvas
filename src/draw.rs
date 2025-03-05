use std::{cell::RefCell, collections::HashMap, f64::consts::PI, rc::Rc};
use wasm_bindgen::prelude::*;
use web_sys::{window, CanvasRenderingContext2d};

use crate::{
    errors::GraphError,
    graph::{
        Connection, FieldType, Graph, NodeInstance, SlotInstance, SlotPosition, SlotTemplate,
        SlotType,
    },
    interaction::{
        ContextMenu, ContextMenuAction, ContextMenuItem, ContextMenuTarget, InteractionState,
        Rectangle,
    },
    GraphCanvas,
};

/// Rendering
/// There should be no locks except in the main `do_render` method.
impl GraphCanvas {
    fn draw_context_menu(
        &self,
        context: &CanvasRenderingContext2d,
        menu: &mut ContextMenu,
        graph: &Graph,
    ) -> Result<(), JsValue> {
        const PADDING: f64 = 10.0;
        const ITEM_HEIGHT: f64 = 30.0;
        const TITLE_HEIGHT: f64 = 25.0;

        // Get menu items based on target type
        let mut items = self.get_context_menu_items(&menu.target_type, graph)?;
        let title = menu.target_type.get_title(graph);

        let menu_height = TITLE_HEIGHT + (items.len() as f64 * ITEM_HEIGHT) + (PADDING * 2.0);

        // Draw menu background
        context.set_fill_style_str("#ffffff");
        context.set_stroke_style_str("#000000");
        context.begin_path();
        context.rect(menu.x, menu.y, self.config.context_menu_size.0, menu_height);
        context.fill();
        context.stroke();

        // Draw title
        context.set_fill_style_str("#000000");
        context.set_font("bold 14px Arial");
        context.set_text_align("left");
        context.fill_text(&title, menu.x + PADDING, menu.y + 20.0)?;

        // Draw separator line
        context.begin_path();
        context.move_to(menu.x, menu.y + TITLE_HEIGHT);
        context.line_to(
            menu.x + self.config.context_menu_size.0,
            menu.y + TITLE_HEIGHT,
        );
        context.stroke();

        // Draw menu items and store their bounds
        context.set_font("12px Arial");
        for (i, item) in items.iter_mut().enumerate() {
            let y_pos = menu.y + TITLE_HEIGHT + (i as f64 * ITEM_HEIGHT);

            // Store the bounds for this item
            item.bounds = Some(Rectangle {
                x: menu.x,
                y: y_pos,
                width: self.config.context_menu_size.0,
                height: ITEM_HEIGHT,
            });

            // Draw item background (maybe highlight if mouse is over)
            context.set_fill_style_str(&item.color);
            context.fill_text(&item.label, menu.x + PADDING, y_pos + 20.0)?;
        }

        // Update the menu's items with their bounds
        menu.items = items;

        Ok(())
    }

    fn draw_dragging_connection(
        &self,
        context: &CanvasRenderingContext2d,
        interaction: &InteractionState,
        graph: &Graph,
    ) {
        // Draw in-progress connection if dragging
        if let Some(connection_drag) = &interaction.connection_drag {
            if let Some(node) = graph.get_node_capabilities(&connection_drag.from_node) {
                if let Some(slot) = node
                    .instance
                    .slots
                    .iter()
                    .find(|s| s.slot_template_id == *connection_drag.from_slot)
                {
                    let slot_template = slot.capabilities(graph).template;
                    let (start_x, start_y) =
                        self.calculate_slot_position(slot_template, node.instance, graph);

                    // Calculate node center for control point
                    let node_center_x = node.instance.x + node.instance.radius;
                    let node_center_y = node.instance.y + node.instance.radius;

                    // Calculate angle from center to slot
                    let start_angle = (start_y - node_center_y).atan2(start_x - node_center_x);

                    // Calculate control point that follows the direction of the slot
                    let control_distance = self.config.connection_control_point_distance;
                    let cp_x = start_x + control_distance * start_angle.cos();
                    let cp_y = start_y + control_distance * start_angle.sin();

                    // Draw the in-progress connection as a bezier curve
                    context.begin_path();
                    context.move_to(start_x, start_y);
                    context.quadratic_curve_to(
                        cp_x,
                        cp_y,
                        connection_drag.current_x,
                        connection_drag.current_y,
                    );
                    context.set_stroke_style_str("#666666");
                    context.set_line_width(2.0);
                    context.stroke();
                    context.set_line_width(1.0);
                }
            }
        }
    }

    fn draw_node(
        &self,
        context: &CanvasRenderingContext2d,
        instance: &NodeInstance,
        graph: &Graph,
        ix: &InteractionState,
    ) -> Result<(), JsValue> {
        // Get the template for this instance
        let template = match graph.node_templates.get(&instance.template_id) {
            Some(t) => t,
            None => return Ok(()), // Skip drawing if template not found
        };

        // Calculate node center and radius
        let center_x = instance.x + instance.radius;
        let center_y = instance.y + instance.radius;
        let radius = instance.radius - 2.0; // Slightly smaller to account for stroke

        // Selected Effect
        if ix.currently_selected_node_instance.as_ref() == Some(&instance.instance_id) {
            context.set_shadow_color("green");
            context.set_shadow_blur(20.0);
            context.set_shadow_offset_x(0.0);
            context.set_shadow_offset_y(0.0);
        }
        // Shadow Effect
        else if ix.hovered_node.as_ref() == Some(&instance.instance_id) {
            // Add shadow effect when hovered
            context.set_shadow_color("blue");
            context.set_shadow_blur(10.0);
            context.set_shadow_offset_x(0.0);
            context.set_shadow_offset_y(0.0);
        }

        // Draw node circle
        context.begin_path();
        context.set_fill_style_str(&instance.color);
        context.set_stroke_style_str("#000000");
        context.arc(center_x, center_y, radius, 0.0, 2.0 * std::f64::consts::PI)?;
        context.fill();
        context.stroke();

        // Reset shadow
        context.set_shadow_color("transparent");
        context.set_shadow_blur(0.0);

        // Draw node title - move it up to make room for fields
        context.set_font("16px Arial");
        context.set_text_align("center");
        context.set_fill_style_str("#000000");

        // Adjust y position based on number of fields (if any)
        let title_y = if !instance.fields.is_empty() {
            center_y - (instance.fields.len() as f64 * 15.0) / 2.0 - 10.0
        } else {
            center_y
        };

        context.fill_text(&template.name, center_x, title_y)?;

        // Draw fields below the title
        if !instance.fields.is_empty() {
            context.set_font("12px Arial");
            context.set_text_align("center");

            let mut y_offset = title_y + 20.0; // Start below the title

            for field_instance in &instance.fields {
                // Get the field template to access name and type
                if let Some(field_template) = template
                    .field_templates
                    .iter()
                    .find(|ft| ft.id == field_instance.field_template_id)
                {
                    let field_text = format!("{}: {}", field_template.name, field_instance.value);
                    context.fill_text(&field_text, center_x, y_offset)?;
                    y_offset += 15.0; // Move down for next field
                }
            }
        }

        // Calculate slot positions and draw them
        let slot_positions = self.calculate_slot_positions(instance, graph, false);

        for (slot_instance, slot_template) in
            instance.slots.iter().zip(template.slot_templates.iter())
        {
            if let Some(position) = slot_positions.get(&slot_template.id) {
                self.draw_slot_at_position(
                    context,
                    slot_instance,
                    slot_template,
                    instance,
                    &graph,
                    ix,
                    position.0,
                    position.1,
                )?;
            }
        }

        Ok(())
    }

    fn draw_connections(
        &self,
        context: &CanvasRenderingContext2d,
        graph: &Graph,
        ix: &InteractionState,
    ) -> Result<(), JsValue> {
        for instance in graph.node_instances.values() {
            for slot in &instance.slots {
                for connection in &slot.connections {
                    if let Some(target_instance) =
                        graph.node_instances.get(&connection.target_node_id)
                    {
                        if let Some(target_slot) = target_instance
                            .slots
                            .iter()
                            .find(|s| s.slot_template_id == connection.target_slot_template_id)
                        {
                            self.draw_connection(
                                context,
                                instance,
                                slot,
                                target_instance,
                                target_slot,
                                graph,
                                ix,
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl GraphCanvas {
    fn draw_slot_at_position(
        &self,
        context: &CanvasRenderingContext2d,
        slot_instance: &SlotInstance,
        slot_template: &SlotTemplate,
        node: &NodeInstance,
        graph: &Graph,
        ix: &InteractionState,
        x: f64,
        y: f64,
    ) -> Result<(), JsValue> {
        // Hover effect
        let is_hovered = ix.hovered_slot.as_ref()
            == Some(&(
                node.instance_id.clone(),
                slot_instance.slot_template_id.clone(),
            ));

        if is_hovered {
            // Add glow effect for hovered slots
            context.set_shadow_color("#4444ff");
            context.set_shadow_blur(8.0);
            context.set_shadow_offset_x(0.0);
            context.set_shadow_offset_y(0.0);
        }

        // Draw circle
        context.begin_path();
        context.arc(x, y, self.config.slot_radius, 0.0, 2.0 * PI)?;

        // Color based on slot type and connection status
        let fill_color = match (
            &slot_template.slot_type,
            slot_instance.connections.is_empty(),
            slot_instance.connections.len() < slot_template.min_connections,
            slot_instance.connections.len() < slot_template.max_connections.unwrap_or(usize::MAX),
        ) {
            (SlotType::Incoming, _, _, _) => "#fff",
            (_, true, true, true) => "red",
            (_, false, true, true) => "orange",
            (_, _, false, true) => "lightgreen",
            (_, _, false, false) => "green",
            _ => "purple",
        };
        context.set_fill_style_str(fill_color);
        context.fill();
        context.stroke();

        // Reset shadow
        context.set_shadow_color("transparent");
        context.set_shadow_blur(0.0);

        // Draw slot label (dynamically positioned based on slot angle from center)
        context.set_font("12px Arial");
        context.set_fill_style_str("#000000");

        // Calculate angle from node center to slot
        let center_x = node.x + node.radius;
        let center_y = node.y + node.radius;
        let angle = (y - center_y).atan2(x - center_x);

        // Text position is outside the slot
        let text_radius = self.config.slot_radius * 1.5;
        let text_x = x + text_radius * angle.cos();
        let text_y = y + text_radius * angle.sin();

        // Adjust text alignment based on which quadrant we're in
        if angle.abs() < PI / 2.0 {
            context.set_text_align("left");
        } else {
            context.set_text_align("right");
        }

        context.fill_text(&slot_template.name, text_x, text_y)?;

        Ok(())
    }

    // Storage for slot position history to reduce flickering
    thread_local! {
        static POSITION_HISTORY: std::cell::RefCell<HashMap<String, HashMap<String, f64>>> =
            std::cell::RefCell::new(HashMap::new());
    }

    pub(crate) fn get_bezier_point(
        &self,
        t: f64,
        p0: (f64, f64),
        p1: (f64, f64),
        p2: (f64, f64),
        p3: (f64, f64),
    ) -> (f64, f64) {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        let x = p0.0 * mt3 + 3.0 * p1.0 * mt2 * t + 3.0 * p2.0 * mt * t2 + p3.0 * t3;
        let y = p0.1 * mt3 + 3.0 * p1.1 * mt2 * t + 3.0 * p2.1 * mt * t2 + p3.1 * t3;

        (x, y)
    }

    pub fn get_context_menu_items(
        &self,
        target: &ContextMenuTarget,
        graph: &Graph,
    ) -> Result<Vec<ContextMenuItem>, JsValue> {
        match target {
            ContextMenuTarget::Node(_) => Ok(vec![ContextMenuItem {
                label: "Delete Node".to_string(),
                action: ContextMenuAction::Delete,
                color: "#ff0000".to_string(),
                bounds: None,
            }]),
            ContextMenuTarget::Connection { .. } => Ok(vec![ContextMenuItem {
                label: "Delete Connection".to_string(),
                action: ContextMenuAction::Delete,
                color: "#ff0000".to_string(),
                bounds: None,
            }]),
            ContextMenuTarget::Slot { .. } => Ok(vec![ContextMenuItem {
                label: "Delete All Connections".to_string(),
                action: ContextMenuAction::DeleteAllSlotConnections,
                color: "#ff0000".to_string(),
                bounds: None,
            }]),
            ContextMenuTarget::Field {
                node_id,
                field_template_id,
            } => {
                // Get the field template to determine its type
                let node = graph
                    .node_instances
                    .get(node_id)
                    .ok_or_else(|| JsValue::from_str("Node not found"))?;
                let template = graph
                    .node_templates
                    .get(&node.template_id)
                    .ok_or_else(|| JsValue::from_str("Template not found"))?;
                let field_template = template
                    .field_templates
                    .iter()
                    .find(|ft| ft.id == *field_template_id)
                    .ok_or_else(|| JsValue::from_str("Field template not found"))?;

                let field_instance = node
                    .fields
                    .iter()
                    .find(|f| f.field_template_id == *field_template_id)
                    .ok_or_else(|| JsValue::from_str("Field not found"))?;

                // Return different menu items based on field type
                match field_template.field_type {
                    FieldType::Boolean => Ok(vec![
                        ContextMenuItem {
                            label: "Set True".to_string(),
                            action: ContextMenuAction::SetBooleanField(true),
                            color: "#0077ff".to_string(),
                            bounds: None,
                        },
                        ContextMenuItem {
                            label: "Set False".to_string(),
                            action: ContextMenuAction::SetBooleanField(false),
                            color: "#0077ff".to_string(),
                            bounds: None,
                        },
                    ]),
                    FieldType::Integer => {
                        // For integers, we provide some increment/decrement options
                        let current_value = field_instance.value.parse::<i32>().unwrap_or(0);
                        Ok(vec![
                            ContextMenuItem {
                                label: format!("Current: {}", current_value),
                                action: ContextMenuAction::EditField,
                                color: "#444444".to_string(),
                                bounds: None,
                            },
                            ContextMenuItem {
                                label: format!("Increment (+1)"),
                                action: ContextMenuAction::SetIntegerField(current_value + 1),
                                color: "#0077ff".to_string(),
                                bounds: None,
                            },
                            ContextMenuItem {
                                label: format!("Decrement (-1)"),
                                action: ContextMenuAction::SetIntegerField(current_value - 1),
                                color: "#0077ff".to_string(),
                                bounds: None,
                            },
                        ])
                    }
                    FieldType::String => {
                        // For strings we just show the current value and edit option
                        Ok(vec![
                            ContextMenuItem {
                                label: format!("Current: {}", field_instance.value),
                                action: ContextMenuAction::EditField,
                                color: "#444444".to_string(),
                                bounds: None,
                            },
                            // ContextMenuItem {
                            //     label: "Edit Text".to_string(),
                            //     action: ContextMenuAction::EditField,
                            //     color: "#0077ff".to_string(),
                            //     bounds: None,
                            // },
                        ])
                    }
                }
            }
        }
    }

    pub(crate) fn distance_to_bezier_curve(
        &self,
        point: (f64, f64),
        start: (f64, f64),
        end: (f64, f64),
        from_node: &NodeInstance,
        to_node: &NodeInstance,
    ) -> f64 {
        // Calculate node centers
        let from_center_x = from_node.x + from_node.radius;
        let from_center_y = from_node.y + from_node.radius;
        let to_center_x = to_node.x + to_node.radius;
        let to_center_y = to_node.y + to_node.radius;

        // Calculate angles from center to slot
        let from_angle = (start.1 - from_center_y).atan2(start.0 - from_center_x);
        let to_angle = (end.1 - to_center_y).atan2(end.0 - to_center_x);

        // Calculate control points
        let control_distance = self.config.connection_control_point_distance;
        let cp1_x = start.0 + control_distance * from_angle.cos();
        let cp1_y = start.1 + control_distance * from_angle.sin();
        let cp2_x = end.0 + control_distance * to_angle.cos();
        let cp2_y = end.1 + control_distance * to_angle.sin();

        // Sample points along the curve
        let samples = 50;
        let mut min_distance = f64::MAX;

        for i in 0..=samples {
            let t = i as f64 / samples as f64;
            let curve_point = self.get_bezier_point(t, start, (cp1_x, cp1_y), (cp2_x, cp2_y), end);

            let distance =
                ((point.0 - curve_point.0).powi(2) + (point.1 - curve_point.1).powi(2)).sqrt();
            min_distance = min_distance.min(distance);
        }

        min_distance
    }
}

// New struct to store slot position data with explicit initialization state
#[derive(Clone, Debug)]
struct SlotPositionCache {
    // Tracks whether positions have been initialized
    initialized: bool,
    // Stores slot angles for each node
    node_angles: HashMap<String, HashMap<String, f64>>,
    // Stores calculated positions (x,y) for quick lookup
    node_positions: HashMap<String, HashMap<String, (f64, f64)>>,
}

impl SlotPositionCache {
    fn new() -> Self {
        SlotPositionCache {
            initialized: false,
            node_angles: HashMap::new(),
            node_positions: HashMap::new(),
        }
    }

    // fn get_angle(&self, node_id: &str, slot_id: &str) -> Option<f64> {
    //     self.node_angles
    //         .get(node_id)
    //         .and_then(|angles| angles.get(slot_id).copied())
    // }

    fn set_angle(&mut self, node_id: &str, slot_id: &str, angle: f64) {
        self.node_angles
            .entry(node_id.to_string())
            .or_insert_with(HashMap::new)
            .insert(slot_id.to_string(), angle);
    }

    fn get_position(&self, node_id: &str, slot_id: &str) -> Option<(f64, f64)> {
        self.node_positions
            .get(node_id)
            .and_then(|positions| positions.get(slot_id).copied())
    }

    fn set_position(&mut self, node_id: &str, slot_id: &str, position: (f64, f64)) {
        self.node_positions
            .entry(node_id.to_string())
            .or_insert_with(HashMap::new)
            .insert(slot_id.to_string(), position);
    }

    fn mark_initialized(&mut self) {
        self.initialized = true;
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn clear_node(&mut self, node_id: &str) {
        self.node_angles.remove(node_id);
        self.node_positions.remove(node_id);
    }
}

/// Rendering
/// There should be no locks except in the main `do_render` method.
impl GraphCanvas {
    // Create a new initialization method to pre-calculate all slot positions
    pub fn initialize_slot_positions(&self, graph: &Graph) {
        // Check initialization status first
        let already_initialized =
            Self::POSITION_CACHE.with(|cache| cache.borrow().is_initialized());

        if already_initialized {
            return;
        }

        // Pre-warm the position cache by running slot stabilization
        for node in graph.node_instances.values() {
            // Run multiple iterations of position calculation to stabilize
            for _ in 0..3 {
                // Calculate positions - these are automatically stored in cache
                self.calculate_slot_positions(node, graph, true);
            }
        }

        // Mark as initialized - separate operation to avoid borrow conflicts
        Self::POSITION_CACHE.with(|cache| {
            let mut cache_ref = cache.borrow_mut();
            cache_ref.mark_initialized();
        });
    }

    pub fn start_render_loop(&self) -> Result<(), GraphError> {
        // Initialize positions first if needed
        if let Ok(graph) = self.graph.try_lock() {
            self.initialize_slot_positions(&graph);
        }

        let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
        let g = f.clone();
        let canvas = self.clone();

        *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
            // Render the graph using the cloned self
            canvas.render().unwrap();

            // Schedule next frame
            window()
                .unwrap()
                .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
                .unwrap();
        }) as Box<dyn FnMut()>));

        window()
            .unwrap()
            .request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .map_err(GraphError::SetupFailed)?;
        Ok(())
    }

    // Rendering - skips if locked
    pub fn render(&self) -> Result<(), JsValue> {
        let element = window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(&self.canvas_id);
        if element.is_none() {
            return Err(JsValue::from_str("Element not found"));
        }
        let canvas = element
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        let context = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;

        if let (Ok(graph), Ok(interaction)) =
            (self.graph.try_lock(), &mut self.interaction.try_lock())
        {
            // Check if we need to initialize positions
            Self::POSITION_CACHE.with(|cache| {
                if !cache.borrow().is_initialized() {
                    self.initialize_slot_positions(&graph);
                }
            });

            self.do_render(&context, &graph, interaction)?;
        }

        Ok(())
    }

    // Helper method to separate rendering logic
    fn do_render(
        &self,
        context: &CanvasRenderingContext2d,
        graph: &Graph,
        interaction: &mut InteractionState,
    ) -> Result<(), JsValue> {
        let canvas = window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(&self.canvas_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();

        context.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);

        // Save the current transform
        context.save();

        // Apply pan transform
        context.translate(
            interaction.view_transform.pan_x,
            interaction.view_transform.pan_y,
        )?;

        // CHANGED ORDER: Draw nodes first, then connections
        // This ensures slot positions are calculated before drawing connections
        for instance in graph.node_instances.values() {
            self.draw_node(context, instance, graph, interaction)?;
        }

        // Now that nodes and slots are drawn, draw connections with updated positions
        self.draw_connections(context, graph, interaction)?;

        // Draw context menu if it exists
        if let Some(menu) = &mut interaction.context_menu {
            self.draw_context_menu(context, menu, graph)?;
        }

        // Draw dragging connection if it exists
        self.draw_dragging_connection(context, interaction, graph);

        // Restore the original transform
        context.restore();
        Ok(())
    }

    // ... [other methods remain the same] ...

    // Replace thread_local with a more robust structure
    thread_local! {
        static POSITION_CACHE: RefCell<SlotPositionCache> = RefCell::new(SlotPositionCache::new());
    }

    // Calculate dynamic slot positions based on connections
    fn calculate_slot_positions(
        &self,
        node: &NodeInstance,
        graph: &Graph,
        is_initialization: bool,
    ) -> HashMap<String, (f64, f64)> {
        let node_template = node.capabilities(graph).template;
        let center_x = node.x + node.radius;
        let center_y = node.y + node.radius;
        let radius = node.radius;

        let mut slot_positions = HashMap::new();
        let mut slot_angles = HashMap::new();
        let mut slot_weights = HashMap::new(); // For tracking slot importance (number of connections)

        // Get previous angles for stability to reduce flickering
        let node_id = node.instance_id.clone();

        // IMPORTANT: Safely get previous angles without holding the borrow
        // Copy all data we need to local variables to avoid borrow issues
        let prev_angles: HashMap<String, f64> = {
            let mut angles = HashMap::new();
            Self::POSITION_CACHE.with(|cache| {
                // Use a limited scope for the borrow to ensure it's dropped
                let cache_ref = cache.borrow();
                if let Some(node_angles) = cache_ref.node_angles.get(&node_id) {
                    for (slot_id, angle) in node_angles {
                        angles.insert(slot_id.clone(), *angle);
                    }
                }
            });
            angles
        };

        // First, determine optimal angles for slots with connections
        for slot in &node.slots {
            if !slot.connections.is_empty() {
                let mut connection_angles = Vec::new();

                // Calculate the angle to each connected node
                for connection in &slot.connections {
                    if let Some(target_node) = graph.node_instances.get(&connection.target_node_id)
                    {
                        let target_x = target_node.x + target_node.radius;
                        let target_y = target_node.y + target_node.radius;
                        let angle = (target_y - center_y).atan2(target_x - center_x);
                        connection_angles.push(angle);
                    }
                }

                // If we have connections, use the mean angle for this slot
                if !connection_angles.is_empty() {
                    // Calculate mean angle (careful with the cyclic nature of angles)
                    let sin_sum: f64 = connection_angles.iter().map(|a| a.sin()).sum();
                    let cos_sum: f64 = connection_angles.iter().map(|a| a.cos()).sum();
                    let mean_angle = sin_sum.atan2(cos_sum);

                    // Apply smoothing if we have a previous position to reduce flickering
                    let final_angle = if let Some(prev_angle) =
                        prev_angles.get(&slot.slot_template_id)
                    {
                        // Interpolate between previous and new angle (exponential smoothing)
                        // Increase alpha value for faster stabilization during initialization
                        let alpha = if is_initialization { 0.6 } else { 0.3 }; // Higher = faster response
                        let mut angle_diff = mean_angle - prev_angle;

                        // Handle angle wrapping
                        if angle_diff > std::f64::consts::PI {
                            angle_diff -= 2.0 * std::f64::consts::PI;
                        } else if angle_diff < -std::f64::consts::PI {
                            angle_diff += 2.0 * std::f64::consts::PI;
                        }

                        let smoothed_angle = prev_angle + alpha * angle_diff;
                        (smoothed_angle + 2.0 * std::f64::consts::PI) % (2.0 * std::f64::consts::PI)
                    } else {
                        mean_angle
                    };

                    slot_angles.insert(slot.slot_template_id.clone(), final_angle);
                    // Give connected slots much higher weight
                    slot_weights.insert(slot.slot_template_id.clone(), slot.connections.len() * 10);
                }
            }
        }

        // Check for incoming connections from other nodes
        let incoming_slot_id = "incoming";
        let mut incoming_connections = Vec::new();

        // Find all nodes that connect to this node
        for (other_id, other_node) in &graph.node_instances {
            if other_id == &node.instance_id {
                continue; // Skip self
            }

            for other_slot in &other_node.slots {
                for conn in &other_slot.connections {
                    if conn.target_node_id == node.instance_id {
                        let other_x = other_node.x + other_node.radius;
                        let other_y = other_node.y + other_node.radius;
                        let angle = (other_y - center_y).atan2(other_x - center_x);
                        incoming_connections.push(angle);
                    }
                }
            }
        }

        // If there are incoming connections, position the incoming slot
        if !incoming_connections.is_empty() {
            let sin_sum: f64 = incoming_connections.iter().map(|a| a.sin()).sum();
            let cos_sum: f64 = incoming_connections.iter().map(|a| a.cos()).sum();
            let mean_angle = sin_sum.atan2(cos_sum);

            // Apply smoothing if we have a previous position
            let final_angle = if let Some(prev_angle) = prev_angles.get(incoming_slot_id) {
                // Increased alpha for faster stabilization during initialization
                let alpha = if is_initialization { 0.6 } else { 0.3 };
                let mut angle_diff = mean_angle - prev_angle;

                // Handle angle wrapping
                if angle_diff > std::f64::consts::PI {
                    angle_diff -= 2.0 * std::f64::consts::PI;
                } else if angle_diff < -std::f64::consts::PI {
                    angle_diff += 2.0 * std::f64::consts::PI;
                }

                let smoothed_angle = prev_angle + alpha * angle_diff;
                (smoothed_angle + 2.0 * std::f64::consts::PI) % (2.0 * std::f64::consts::PI)
            } else {
                mean_angle
            };

            slot_angles.insert(incoming_slot_id.to_string(), final_angle);
            slot_weights.insert(
                incoming_slot_id.to_string(),
                incoming_connections.len() * 10,
            );
        }

        // Create a list of unassigned slots (those without connections)
        let mut unassigned_slots = Vec::new();

        // Include the incoming slot if it's not already assigned
        if !slot_angles.contains_key(incoming_slot_id) {
            unassigned_slots.push(incoming_slot_id);
        }

        // Add all unassigned template slots
        for template in &node_template.slot_templates {
            if !slot_angles.contains_key(&template.id) {
                unassigned_slots.push(&template.id);
            }
        }

        // Assign initial positions to unassigned slots
        if !unassigned_slots.is_empty() {
            // For each unassigned slot, generate initial positions
            for slot_id in &unassigned_slots {
                // If we have a previous position, use it for stability
                // Otherwise use a default based on its original position
                let initial_angle = if let Some(prev_angle) = prev_angles.get(*slot_id) {
                    *prev_angle
                } else {
                    // Generate an initial angle based on the slot template's position
                    match *slot_id {
                        "incoming" => std::f64::consts::PI, // Left side
                        _ => {
                            // Find the template for this slot
                            if let Some(template) = node_template
                                .slot_templates
                                .iter()
                                .find(|t| &t.id == slot_id)
                            {
                                match template.position {
                                    SlotPosition::Right => 0.0,
                                    SlotPosition::Bottom => std::f64::consts::PI / 2.0,
                                    SlotPosition::Left => std::f64::consts::PI,
                                    SlotPosition::Top => 3.0 * std::f64::consts::PI / 2.0,
                                }
                            } else {
                                // Default to right side if template not found (shouldn't happen)
                                0.0
                            }
                        }
                    }
                };

                slot_angles.insert((*slot_id).to_string(), initial_angle);
                slot_weights.insert((*slot_id).to_string(), 1); // Minimal weight for empty slots
            }
        }

        // Use our gas-like model algorithm to distribute slots
        // Increase stabilization iterations during initialization
        let iterations = if is_initialization { 50 } else { 30 };
        self.distribute_slots_gas_model(
            &mut slot_angles,
            &slot_weights,
            unassigned_slots.len(),
            iterations,
        );

        // Now convert angles to positions
        for (slot_id, angle) in &slot_angles {
            let x = center_x + radius * angle.cos();
            let y = center_y + radius * angle.sin();
            slot_positions.insert(slot_id.clone(), (x, y));
        }

        // Store all data at once - AFTER all calculations are complete
        // This avoids recursive borrowing issues
        Self::POSITION_CACHE.with(|cache| {
            let mut cache_ref = cache.borrow_mut();
            // Clear old data for this node
            cache_ref.clear_node(&node_id);

            // Save all current angles
            for (id, angle) in &slot_angles {
                cache_ref.set_angle(&node_id, id, *angle);
            }

            // Save all positions
            for (id, (x, y)) in &slot_positions {
                cache_ref.set_position(&node_id, id, (*x, *y));
            }
        });

        slot_positions
    }

    // Modified gas model with iteration count parameter
    fn distribute_slots_gas_model(
        &self,
        slot_angles: &mut HashMap<String, f64>,
        slot_weights: &HashMap<String, usize>,
        empty_slot_count: usize,
        iterations: usize, // Allow customization of iteration count
    ) {
        // Parameters for simulation
        let repulsion_strength = 0.01;
        let min_angle_separation = 0.4;
        let incoming_slot_id = "incoming";

        // Empty slots should get more evenly distributed
        let empty_slot_base_spacing = if empty_slot_count > 0 {
            2.0 * std::f64::consts::PI / (empty_slot_count as f64).max(1.0)
        } else {
            0.0
        };

        // Prepare slots for processing
        let mut slots: Vec<(String, f64, usize)> = slot_angles
            .iter()
            .map(|(id, angle)| (id.clone(), *angle, *slot_weights.get(id).unwrap_or(&1)))
            .collect();

        // Handle special case where the incoming slot needs special treatment
        let incoming_idx = slots.iter().position(|(id, _, _)| id == incoming_slot_id);
        let _has_active_incoming = incoming_idx.is_some()
            && slots
                .get(incoming_idx.unwrap())
                .map_or(false, |(_, _, w)| *w > 1);

        // Check if all slots are empty (no connections)
        let all_slots_empty = slots.iter().all(|(_, _, weight)| *weight <= 1);

        // If all slots are empty, don't run the simulation - just evenly distribute them
        if all_slots_empty {
            // Sort slots by ID to ensure consistent ordering across frames
            slots.sort_by(|(id_a, _, _), (id_b, _, _)| id_a.cmp(id_b));

            // For empty nodes, just place slots at fixed positions evenly around the circle
            let slot_count = slots.len();
            let angle_step = 2.0 * std::f64::consts::PI / slot_count as f64;

            // Position the slots evenly, sorting by ID for stability
            slot_angles.clear();

            for (i, (id, _, _)) in slots.iter().enumerate() {
                // Place the slots evenly around the circle
                let angle = i as f64 * angle_step;

                // Store the final position
                slot_angles.insert(id.clone(), angle);
            }

            return;
        }

        // Sort by weight (heaviest/most connected slots first)
        slots.sort_by(|a, b| {
            // Give special priority to incoming slot with connections
            if a.0 == incoming_slot_id && a.2 > 1 {
                std::cmp::Ordering::Less
            } else if b.0 == incoming_slot_id && b.2 > 1 {
                std::cmp::Ordering::Greater
            } else {
                // Normal weight-based comparison
                b.2.cmp(&a.2)
            }
        });

        // Run simulation iterations - using the provided iteration count
        for iter_count in 0..iterations {
            // Calculate forces between all pairs of slots
            let mut forces: Vec<f64> = vec![0.0; slots.len()];

            for i in 0..slots.len() {
                let (id_i, angle_i, weight_i) = &slots[i];
                let is_empty_i = *weight_i <= 1; // No connections = empty
                let is_incoming_i = id_i == incoming_slot_id;

                // Each slot tries to maintain a preferred distance from others
                for j in 0..slots.len() {
                    if i == j {
                        continue;
                    }

                    let (id_j, angle_j, weight_j) = &slots[j];
                    let is_empty_j = *weight_j <= 1; // No connections = empty
                    let is_incoming_j = id_j == incoming_slot_id;

                    // Calculate circular distance between angles
                    let mut diff = angle_i - angle_j;
                    if diff > std::f64::consts::PI {
                        diff -= 2.0 * std::f64::consts::PI;
                    }
                    if diff < -std::f64::consts::PI {
                        diff += 2.0 * std::f64::consts::PI;
                    }

                    // Calculate ideal separation based on slot types
                    let ideal_separation = if is_empty_i && is_empty_j {
                        // Empty slots try to maintain even spacing
                        empty_slot_base_spacing.max(min_angle_separation)
                    } else if is_incoming_i || is_incoming_j {
                        // Incoming slots need more separation
                        min_angle_separation * 1.5
                    } else {
                        // Connected slots need at least minimum separation
                        min_angle_separation
                    };

                    // Skip if already far enough apart
                    if diff.abs() >= ideal_separation {
                        continue;
                    }

                    // Force direction - positive = clockwise, negative = counterclockwise
                    let direction = if diff > 0.0 { 1.0 } else { -1.0 };

                    // Force magnitude based on distance - closer = stronger repulsion
                    let distance = diff.abs().max(0.01); // Prevent division by zero
                    let distance_factor = 1.0 - distance / ideal_separation;

                    // Calculate base repulsion strength
                    let base_repulsion = if is_incoming_i || is_incoming_j {
                        // Stronger repulsion for incoming slots
                        repulsion_strength * 1.5
                    } else {
                        repulsion_strength
                    };

                    // Use stronger repulsion in early iterations for faster stabilization
                    let iteration_factor =
                        1.0 + 0.5 * (1.0 - iter_count as f64 / iterations as f64);
                    let adjusted_repulsion = base_repulsion * iteration_factor;

                    let repulsion = adjusted_repulsion * distance_factor;

                    // Weight affects how much a slot resists movement
                    let weight_factor =
                        if (is_incoming_i && !is_empty_j) || (is_incoming_j && !is_empty_i) {
                            // Incoming slots strongly push empty slots
                            1.3
                        } else {
                            (*weight_i as f64) / (*weight_j as f64).max(1.0)
                        };

                    // Calculate final force
                    let force = direction * repulsion * weight_factor;
                    forces[i] += force;
                }

                // Special case for slots - attract to appropriate positions
                if !is_incoming_i || is_empty_i {
                    // Find angles of other slots - include IDs for stable sorting
                    let mut other_angles: Vec<(f64, bool, bool, String)> = slots
                        .iter()
                        .filter(|(other_id, _, other_weight)| {
                            *other_id != *id_i &&
                            // Only consider populated slots and the incoming slot as barriers
                            (*other_weight > 1 || *other_id == incoming_slot_id)
                        })
                        .map(|(other_id, a, w)| {
                            (*a, *w > 1, other_id == incoming_slot_id, other_id.clone())
                        })
                        .collect();

                    // Sort by angle first, then by ID for stable ordering
                    other_angles.sort_by(|a, b| {
                        a.0.partial_cmp(&b.0)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| a.3.cmp(&b.3))
                    });

                    if !other_angles.is_empty() {
                        // Find the largest gap
                        let mut angles: Vec<f64> =
                            other_angles.iter().map(|(a, _, _, _)| *a).collect();
                        angles.sort_by(|a, b| a.partial_cmp(b).unwrap());

                        let mut max_gap = 0.0;
                        let mut gap_middle = 0.0;

                        for i in 0..angles.len() {
                            let next_idx = (i + 1) % angles.len();
                            let mut gap = angles[next_idx] - angles[i];
                            if gap < 0.0 {
                                gap += 2.0 * std::f64::consts::PI;
                            }

                            if gap > max_gap {
                                max_gap = gap;
                                gap_middle = (angles[i] + gap / 2.0) % (2.0 * std::f64::consts::PI);
                            }
                        }

                        // For empty slots, find an appropriate gap
                        if is_empty_i && max_gap > min_angle_separation * 2.0 {
                            // Calculate the middle point of the gap
                            let mut attraction_diff = gap_middle - angle_i;
                            if attraction_diff > std::f64::consts::PI {
                                attraction_diff -= 2.0 * std::f64::consts::PI;
                            }
                            if attraction_diff < -std::f64::consts::PI {
                                attraction_diff += 2.0 * std::f64::consts::PI;
                            }

                            // Stronger attraction in later iterations
                            let attraction_strength =
                                0.001 * (1.0 + iter_count as f64 / iterations as f64);
                            let attraction_force = attraction_diff * attraction_strength;
                            forces[i] += attraction_force;
                        }
                    }
                }
            }

            // Apply forces to update positions
            for i in 0..slots.len() {
                let (id, ref mut angle, weight) = &mut slots[i];

                // Special handling for incoming slot with connections
                let resistance = if id == incoming_slot_id && *weight > 1 {
                    // Connected incoming slots are harder to move (2x resistance)
                    1.0 + (*weight as f64 * 0.4)
                } else {
                    // Normal resistance
                    1.0 + (*weight as f64 * 0.2)
                };

                let movement = forces[i] / resistance;

                // Update position
                *angle =
                    (*angle + movement + 2.0 * std::f64::consts::PI) % (2.0 * std::f64::consts::PI);
            }

            // Early termination check - if slots are well separated, we can stop
            if iter_count >= iterations - 5 {
                let mut all_separated = true;

                for i in 0..slots.len() {
                    for j in (i + 1)..slots.len() {
                        let (id_i, angle_i, _) = &slots[i];
                        let (id_j, angle_j, _) = &slots[j];

                        // Calculate angular distance
                        let mut diff = (angle_i - angle_j).abs();
                        diff = diff.min(2.0 * std::f64::consts::PI - diff);

                        // Required separation depends on slot types
                        let required_sep = if id_i == incoming_slot_id || id_j == incoming_slot_id {
                            min_angle_separation * 1.2 // Incoming slots need more space
                        } else {
                            min_angle_separation * 0.9 // Regular slots
                        };

                        if diff < required_sep {
                            all_separated = false;
                            break;
                        }
                    }
                    if !all_separated {
                        break;
                    }
                }

                // If all slots are well-separated, we can stop early
                if all_separated {
                    break;
                }
            }
        }

        // Update the slot_angles map with final positions
        slot_angles.clear();
        for (id, angle, _) in slots {
            slot_angles.insert(id, angle);
        }
    }

    // Modified to use the position cache for consistency
    pub fn calculate_slot_position(
        &self,
        slot_template: &SlotTemplate,
        node: &NodeInstance,
        graph: &Graph,
    ) -> (f64, f64) {
        // Check position cache first for consistent positions
        let node_id = &node.instance_id;
        let slot_id = &slot_template.id;

        // Get cached position without holding a borrow
        let cached_position = {
            let mut position = None;
            Self::POSITION_CACHE.with(|cache| {
                position = cache.borrow().get_position(node_id, slot_id);
            });
            position
        };

        if let Some(position) = cached_position {
            return position;
        }

        // If not in cache, calculate dynamically
        let positions = self.calculate_slot_positions(node, graph, false);

        // If we have a calculated position, use it
        if let Some(position) = positions.get(&slot_template.id) {
            return *position;
        }

        // Fallback to old-style calculation for compatibility
        let center_x = node.x + node.radius;
        let center_y = node.y + node.radius;
        let radius = node.radius;

        // Place at a default position based on original position property
        let angle = match slot_template.position {
            SlotPosition::Right => 0.0,
            SlotPosition::Bottom => std::f64::consts::PI / 2.0,
            SlotPosition::Left => std::f64::consts::PI,
            SlotPosition::Top => 3.0 * std::f64::consts::PI / 2.0,
        };

        let position = (
            center_x + radius * angle.cos(),
            center_y + radius * angle.sin(),
        );

        // Cache the calculated position for future use - but don't borrow during recursion
        {
            Self::POSITION_CACHE.with(|cache| {
                let mut cache_ref = cache.borrow_mut();
                cache_ref.set_position(node_id, slot_id, position);
            });
        }

        position
    }

    fn draw_connection(
        &self,
        context: &CanvasRenderingContext2d,
        from_node: &NodeInstance,
        from_slot: &SlotInstance,
        to_node: &NodeInstance,
        to_slot: &SlotInstance,
        graph: &Graph,
        ix: &InteractionState,
    ) -> Result<(), JsValue> {
        let current_connection = Connection {
            host_node_id: from_node.instance_id.clone(),
            host_slot_template_id: from_slot.slot_template_id.clone(),
            target_node_id: to_node.instance_id.clone(),
            target_slot_template_id: to_slot.slot_template_id.clone(),
            can_delete: true,
        };
        let is_hovered = ix.hovered_connection.as_ref() == Some(&current_connection);

        // Get templates
        let from_template = graph.node_templates.get(&from_node.template_id).unwrap();
        let to_template = graph.node_templates.get(&to_node.template_id).unwrap();

        // Find slot templates
        let from_slot_template = from_template
            .slot_templates
            .iter()
            .find(|t| t.id == from_slot.slot_template_id)
            .unwrap();
        let to_slot_template = to_template
            .slot_templates
            .iter()
            .find(|t| t.id == to_slot.slot_template_id)
            .unwrap();

        // Calculate start and end points - use cached positions for consistency
        let (start_x, start_y) = self.calculate_slot_position(from_slot_template, from_node, graph);
        let (end_x, end_y) = self.calculate_slot_position(to_slot_template, to_node, graph);

        // Calculate centers of nodes for control points
        let from_center_x = from_node.x + from_node.radius;
        let from_center_y = from_node.y + from_node.radius;
        let to_center_x = to_node.x + to_node.radius;
        let to_center_y = to_node.y + to_node.radius;

        // Calculate angles from center to slot
        let from_angle = (start_y - from_center_y).atan2(start_x - from_center_x);
        let to_angle = (end_y - to_center_y).atan2(end_x - to_center_x);

        // Draw curved connection line
        context.begin_path();
        context.move_to(start_x, start_y);

        // Calculate control points that follow the direction of the slots
        let control_point_distance = self.config.connection_control_point_distance;
        let cp1_x = start_x + control_point_distance * from_angle.cos();
        let cp1_y = start_y + control_point_distance * from_angle.sin();
        let cp2_x = end_x + control_point_distance * to_angle.cos();
        let cp2_y = end_y + control_point_distance * to_angle.sin();

        context.bezier_curve_to(cp1_x, cp1_y, cp2_x, cp2_y, end_x, end_y);

        if is_hovered {
            context.set_line_width(3.0);
            context.set_stroke_style_str("#4444ff");
        } else {
            context.set_line_width(2.0);
            context.set_stroke_style_str("#666666");
        }
        context.stroke();
        context.set_line_width(1.0);

        Ok(())
    }
}
