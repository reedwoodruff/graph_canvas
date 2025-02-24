use std::{cell::RefCell, collections::HashMap, f64::consts::PI, rc::Rc};
use wasm_bindgen::prelude::*;
use web_sys::{window, CanvasRenderingContext2d};

use crate::{
    errors::GraphError,
    graph::{Connection, Graph, NodeInstance, SlotInstance, SlotPosition, SlotTemplate, SlotType},
    interaction::{
        ContextMenu, ContextMenuAction, ContextMenuItem, ContextMenuTarget, InteractionState,
        Rectangle,
    },
    GraphCanvas,
};

/// Rendering
/// There should be no locks except in the main `do_render` method.
impl GraphCanvas {
    pub fn start_render_loop(&self) -> Result<(), GraphError> {
        let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
        let g = f.clone();
        let canvas = self.clone(); // Add Clone derive to GraphCanvas

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
        let context = self
            .canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;

        if let (Ok(graph), Ok(interaction)) =
            (self.graph.try_lock(), &mut self.interaction.try_lock())
        {
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
        context.clear_rect(
            0.0,
            0.0,
            self.canvas.width() as f64,
            self.canvas.height() as f64,
        );

        // Save the current transform
        context.save();

        // Apply pan transform
        context.translate(
            interaction.view_transform.pan_x,
            interaction.view_transform.pan_y,
        )?;

        self.draw_connections(context, graph, interaction)?;

        for instance in graph.node_instances.values() {
            self.draw_node(context, instance, graph, interaction)?;
        }

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
        let mut items = self.get_context_menu_items(&menu.target_type)?;
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
                    let node_center_x = node.instance.x + node.instance.width / 2.0;
                    let node_center_y = node.instance.y + node.instance.height / 2.0;

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
        let center_x = instance.x + instance.width / 2.0;
        let center_y = instance.y + instance.height / 2.0;
        let radius = (instance.width.min(instance.height) / 2.0) - 2.0; // Slightly smaller to account for stroke

        // Hover effect
        let is_hovered = ix.hovered_node.as_ref() == Some(&instance.instance_id);
        if is_hovered {
            // Add shadow effect when hovered
            context.set_shadow_color("#666666");
            context.set_shadow_blur(10.0);
            context.set_shadow_offset_x(0.0);
            context.set_shadow_offset_y(0.0);
        }

        // Draw node circle
        context.begin_path();
        context.set_fill_style_str("#ffffff");
        context.set_stroke_style_str("#000000");
        context.arc(center_x, center_y, radius, 0.0, 2.0 * std::f64::consts::PI)?;
        context.fill();
        context.stroke();

        // Reset shadow
        context.set_shadow_color("transparent");
        context.set_shadow_blur(0.0);

        // Draw node title
        context.set_font("16px Arial");
        context.set_text_align("center");
        context.set_fill_style_str("#000000");
        context.fill_text(&template.name, center_x, center_y)?;

        // Calculate slot positions and draw them
        let slot_positions = self.calculate_slot_positions(instance, graph);

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

    // For backward compatibility with existing code
    fn draw_slot(
        &self,
        context: &CanvasRenderingContext2d,
        slot_instance: &SlotInstance,
        slot_template: &SlotTemplate,
        node: &NodeInstance,
        graph: &Graph,
        ix: &InteractionState,
    ) -> Result<(), JsValue> {
        let (x, y) = self.calculate_slot_position(slot_template, node, graph);
        self.draw_slot_at_position(context, slot_instance, slot_template, node, graph, ix, x, y)
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

        // Calculate start and end points
        let (start_x, start_y) = self.calculate_slot_position(from_slot_template, from_node, graph);
        let (end_x, end_y) = self.calculate_slot_position(to_slot_template, to_node, graph);

        // Calculate centers of nodes for control points
        let from_center_x = from_node.x + from_node.width / 2.0;
        let from_center_y = from_node.y + from_node.height / 2.0;
        let to_center_x = to_node.x + to_node.width / 2.0;
        let to_center_y = to_node.y + to_node.height / 2.0;

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
        let center_x = node.x + node.width / 2.0;
        let center_y = node.y + node.height / 2.0;
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

    // Calculate dynamic slot positions based on connections
    fn calculate_slot_positions(
        &self,
        node: &NodeInstance,
        graph: &Graph,
    ) -> HashMap<String, (f64, f64)> {
        let node_template = node.capabilities(graph).template;
        let center_x = node.x + node.width / 2.0;
        let center_y = node.y + node.height / 2.0;
        let radius = node.width.min(node.height) / 2.0 - 2.0;

        let mut slot_positions = HashMap::new();
        let mut slot_angles = HashMap::new();
        let mut slot_weights = HashMap::new(); // For tracking slot importance (number of connections)

        // First, determine optimal angles for slots with connections (including incoming slot)
        for slot in &node.slots {
            if !slot.connections.is_empty() {
                let mut connection_angles = Vec::new();

                // Calculate the angle to each connected node
                for connection in &slot.connections {
                    if let Some(target_node) = graph.node_instances.get(&connection.target_node_id)
                    {
                        let target_x = target_node.x + target_node.width / 2.0;
                        let target_y = target_node.y + target_node.height / 2.0;
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

                    slot_angles.insert(slot.slot_template_id.clone(), mean_angle);
                    slot_weights.insert(slot.slot_template_id.clone(), slot.connections.len());
                }
            }
        }

        // Check for incoming connections from other nodes and position the incoming slot accordingly
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
                        let other_x = other_node.x + other_node.width / 2.0;
                        let other_y = other_node.y + other_node.height / 2.0;
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

            // Position it opposite from the source nodes for better visual layout
            // let opposite_angle = (mean_angle + std::f64::consts::PI) % (2.0 * std::f64::consts::PI);
            slot_angles.insert(incoming_slot_id.to_string(), mean_angle);
            slot_weights.insert(incoming_slot_id.to_string(), incoming_connections.len());
        }

        // For slots without connections, distribute them evenly around the circle
        let num_slots_without_positions = node_template.slot_templates.len() - slot_angles.len(); // +1 for the incoming slot

        if num_slots_without_positions > 0 {
            // Find available spaces (avoiding positions too close to existing slots)
            let mut unassigned_slots = Vec::new();

            // Include the incoming slot if it's not already assigned
            if !slot_angles.contains_key(incoming_slot_id) {
                unassigned_slots.push(incoming_slot_id);
            }

            // Add unassigned template slots
            for template in &node_template.slot_templates {
                if !slot_angles.contains_key(&template.id) {
                    unassigned_slots.push(&template.id);
                }
            }

            // Sort by consistent criteria for stable layout
            unassigned_slots.sort();

            // Distribute remaining slots evenly
            let angle_step = 2.0 * std::f64::consts::PI / (num_slots_without_positions as f64);

            // Start from a position avoiding existing slots if possible
            let mut start_angle = 0.0;
            if !slot_angles.is_empty() {
                // Find the largest gap between assigned angles
                let mut assigned_angles: Vec<f64> = slot_angles.values().cloned().collect();
                assigned_angles.sort_by(|a, b| a.partial_cmp(b).unwrap());

                let mut max_gap = 0.0;
                let mut max_gap_start = 0.0;

                for i in 0..assigned_angles.len() {
                    let next_idx = (i + 1) % assigned_angles.len();
                    let mut gap = assigned_angles[next_idx] - assigned_angles[i];
                    if gap < 0.0 {
                        gap += 2.0 * std::f64::consts::PI;
                    }

                    if gap > max_gap {
                        max_gap = gap;
                        max_gap_start = assigned_angles[i];
                    }
                }

                // Start in the middle of the largest gap
                start_angle = max_gap_start + (max_gap / 2.0);
            }

            // Assign slots to evenly distributed positions
            for (i, slot_id) in unassigned_slots.iter().enumerate() {
                let angle = start_angle + (i as f64 * angle_step);
                slot_angles.insert((*slot_id).to_string(), angle);
                slot_weights.insert((*slot_id).to_string(), 0); // No connections for these slots
            }
        }

        // Resolve slot overlaps
        self.resolve_slot_overlaps(&mut slot_angles, &slot_weights);

        // Now convert angles to positions
        for (slot_id, angle) in slot_angles {
            let x = center_x + radius * angle.cos();
            let y = center_y + radius * angle.sin();
            slot_positions.insert(slot_id, (x, y));
        }

        slot_positions
    }

    // Helper method to resolve slot overlaps, giving preference to slots with more connections
    fn resolve_slot_overlaps(
        &self,
        slot_angles: &mut HashMap<String, f64>,
        slot_weights: &HashMap<String, usize>,
    ) {
        // Minimum angle separation between slots (in radians)
        let min_angle_separation = 0.15; // About 8.6 degrees

        // First, sort slots by weight (descending) so higher-weight slots have priority
        let mut slots: Vec<(String, f64, usize)> = slot_angles
            .iter()
            .map(|(id, angle)| (id.clone(), *angle, *slot_weights.get(id).unwrap_or(&0)))
            .collect();

        // Sort by weight (connections) in descending order
        slots.sort_by(|a, b| b.2.cmp(&a.2));

        // Clear the map before rebuilding
        slot_angles.clear();

        // Start with the highest weighted slot
        if !slots.is_empty() {
            slot_angles.insert(slots[0].0.clone(), slots[0].1);

            // Now place remaining slots, adjusting their positions if they're too close to existing ones
            for i in 1..slots.len() {
                let (id, target_angle, _) = &slots[i];
                let mut final_angle = *target_angle;
                let mut retry_count = 0;
                let max_retries = 10;

                loop {
                    let mut too_close = false;

                    // Check against already placed slots
                    for (_, placed_angle) in slot_angles.iter() {
                        let angle_diff = (final_angle - placed_angle).abs();
                        let normalized_diff =
                            angle_diff.min(2.0 * std::f64::consts::PI - angle_diff);

                        if normalized_diff < min_angle_separation {
                            too_close = true;
                            break;
                        }
                    }

                    if !too_close || retry_count >= max_retries {
                        break;
                    }

                    // If too close, nudge away from the closest slot
                    let mut closest_angle = 0.0;
                    let mut min_diff = std::f64::consts::PI * 2.0;

                    for (_, placed_angle) in slot_angles.iter() {
                        let angle_diff = (final_angle - placed_angle).abs();
                        let normalized_diff =
                            angle_diff.min(2.0 * std::f64::consts::PI - angle_diff);

                        if normalized_diff < min_diff {
                            min_diff = normalized_diff;
                            closest_angle = *placed_angle;
                        }
                    }

                    // Move away from closest slot
                    let direction = if (final_angle - closest_angle + 2.0 * std::f64::consts::PI)
                        % (2.0 * std::f64::consts::PI)
                        < std::f64::consts::PI
                    {
                        1.0 // Move clockwise
                    } else {
                        -1.0 // Move counter-clockwise
                    };

                    // Increase nudge amount with retry count to escape local minima
                    let nudge = min_angle_separation * (1.0 + 0.5 * retry_count as f64) * direction;
                    final_angle = (final_angle + nudge + 2.0 * std::f64::consts::PI)
                        % (2.0 * std::f64::consts::PI);

                    retry_count += 1;
                }

                slot_angles.insert(id.clone(), final_angle);
            }
        }
    }

    // Legacy method kept for backward compatibility with existing code
    pub fn calculate_slot_position(
        &self,
        slot_template: &SlotTemplate,
        node: &NodeInstance,
        graph: &Graph,
    ) -> (f64, f64) {
        // Calculate positions dynamically
        let positions = self.calculate_slot_positions(node, graph);

        // If we have a calculated position, use it
        if let Some(position) = positions.get(&slot_template.id) {
            return *position;
        }

        // Special case for the "incoming" slot
        if slot_template.id == "incoming" {
            if let Some(position) = positions.get("incoming") {
                return *position;
            }
        }

        // Fallback to old-style calculation for compatibility
        let center_x = node.x + node.width / 2.0;
        let center_y = node.y + node.height / 2.0;
        let radius = node.width.min(node.height) / 2.0 - 2.0;

        // Place at a default position based on original position property
        let angle = match slot_template.position {
            SlotPosition::Right => 0.0,
            SlotPosition::Bottom => std::f64::consts::PI / 2.0,
            SlotPosition::Left => std::f64::consts::PI,
            SlotPosition::Top => 3.0 * std::f64::consts::PI / 2.0,
        };

        (
            center_x + radius * angle.cos(),
            center_y + radius * angle.sin(),
        )
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
        let from_center_x = from_node.x + from_node.width / 2.0;
        let from_center_y = from_node.y + from_node.height / 2.0;
        let to_center_x = to_node.x + to_node.width / 2.0;
        let to_center_y = to_node.y + to_node.height / 2.0;

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
