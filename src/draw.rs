use std::{cell::RefCell, f64::consts::PI, rc::Rc};
use wasm_bindgen::prelude::*;
use web_sys::{window, CanvasRenderingContext2d};

use crate::{
    common::get_bezier_control_points,
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

                    // Draw the in-progress connection
                    context.begin_path();
                    context.move_to(start_x, start_y);
                    context.line_to(connection_drag.current_x, connection_drag.current_y);
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
        // Hover effect
        {
            let is_hovered = ix.hovered_node.as_ref() == Some(&instance.instance_id);

            // Draw node rectangle with hover effect
            context.begin_path();
            context.set_fill_style_str("#ffffff");
            context.set_stroke_style_str("#000000");

            if is_hovered {
                // Add shadow effect when hovered
                context.set_shadow_color("#666666");
                context.set_shadow_blur(10.0);
                context.set_shadow_offset_x(0.0);
                context.set_shadow_offset_y(0.0);
            }

            context.rect(instance.x, instance.y, instance.width, instance.height);
            context.fill();
            context.stroke();

            // Reset shadow
            context.set_shadow_color("transparent");
            context.set_shadow_blur(0.0);
        }

        // Draw node rectangle
        context.begin_path();
        context.set_fill_style_str("#ffffff");
        context.set_stroke_style_str("#000000");
        context.rect(instance.x, instance.y, instance.width, instance.height);
        context.fill();
        context.stroke();

        // Draw node title
        context.set_font("16px Arial");
        context.set_text_align("center");
        context.set_fill_style_str("#000000");
        context.fill_text(
            &template.name,
            instance.x + instance.width / 2.0,
            instance.y + 25.0,
        )?;

        // Draw slots
        for (slot_instance, slot_template) in
            instance.slots.iter().zip(template.slot_templates.iter())
        {
            self.draw_slot(context, slot_instance, slot_template, instance, graph, ix)?;
        }

        Ok(())
    }

    fn draw_slot(
        &self,
        context: &CanvasRenderingContext2d,
        slot_instance: &SlotInstance,
        slot_template: &SlotTemplate,
        node: &NodeInstance,
        graph: &Graph,
        ix: &InteractionState,
    ) -> Result<(), JsValue> {
        // Hover effect
        {
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
        }
        // Triangle dimensions
        // let triangle_size = SLOT_DRAW_RADIUS * 1.5; // Make triangle slightly larger than circle hitbox
        let triangle_size = self.config.slot_radius;

        let (x, y) = self.calculate_slot_position(slot_template, node, graph);

        // Draw circle if it is incoming
        context.begin_path();
        if slot_template.slot_type == SlotType::Incoming {
            context.arc(x, y, self.config.slot_radius, 0.0, 2.0 * PI)?;
        } else {
            // Draw triangle based on position and type
            match &slot_template.position {
                SlotPosition::Left => {
                    // Outgoing triangle pointing right
                    context.move_to(x + triangle_size, y - triangle_size);
                    context.line_to(x - triangle_size, y);
                    context.line_to(x + triangle_size, y + triangle_size);
                }

                SlotPosition::Right => {
                    // Outgoing triangle pointing right
                    context.move_to(x - triangle_size, y - triangle_size);
                    context.line_to(x + triangle_size, y);
                    context.line_to(x - triangle_size, y + triangle_size);
                }
                SlotPosition::Top => {
                    // Outgoing triangle pointing up
                    context.move_to(x - triangle_size, y + triangle_size);
                    context.line_to(x + triangle_size, y + triangle_size);
                    context.line_to(x, y - triangle_size);
                }
                SlotPosition::Bottom => {
                    // Outgoing triangle pointing down
                    context.move_to(x - triangle_size, y - triangle_size);
                    context.line_to(x + triangle_size, y - triangle_size);
                    context.line_to(x, y + triangle_size);
                }
            }
        }
        context.close_path();

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

        // Draw slot label
        context.set_font("12px Arial");
        context.set_fill_style_str("#000000");
        let slot_radius = self.config.slot_radius;
        match slot_template.position {
            SlotPosition::Left => {
                context.set_text_align("left");
                context.fill_text(
                    &slot_template.name,
                    x + (slot_radius + 4.0),
                    y + (slot_radius / 2.0),
                )?;
            }
            SlotPosition::Right => {
                context.set_text_align("right");
                context.fill_text(
                    &slot_template.name,
                    x - (slot_radius + 4.0),
                    y + (slot_radius / 2.0),
                )?;
            }
            SlotPosition::Top => {
                context.set_text_align("center");
                context.fill_text(&slot_template.name, x, y + (slot_radius + 12.0))?;
            }
            SlotPosition::Bottom => {
                context.set_text_align("center");
                context.fill_text(&slot_template.name, x, y - (slot_radius + 4.0))?;
            }
        }

        // Reset shadow
        context.set_shadow_color("transparent");
        context.set_shadow_blur(0.0);

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

        // Draw curved connection line
        context.begin_path();
        context.move_to(start_x, start_y);

        // Calculate control points for curve
        let (cp1_x, cp1_y) = get_bezier_control_points(
            start_x,
            start_y,
            &from_slot_template.position,
            self.config.connection_control_point_distance,
        );
        let (cp2_x, cp2_y) = get_bezier_control_points(
            end_x,
            end_y,
            &to_slot_template.position,
            self.config.connection_control_point_distance,
        );

        context.bezier_curve_to(cp1_x, cp1_y, cp2_x, cp2_y, end_x, end_y);
        context.set_stroke_style_str("#666666");
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
    pub fn calculate_slot_position(
        &self,
        slot_template: &SlotTemplate,
        node: &NodeInstance,
        graph: &Graph,
    ) -> (f64, f64) {
        let node_template = node.capabilities(graph).template;
        let position = &slot_template.position;
        let direction = &slot_template.slot_type;

        let slots_on_side = node_template
            .slot_templates
            .iter()
            .filter(|s_t| s_t.position == slot_template.position)
            .collect::<Vec<_>>();
        let num_slots_on_side = slots_on_side.len();

        // Find this slot's index among slots on the same side
        let slot_index = slots_on_side
            .iter()
            .position(|slot_in_question| slot_in_question.id == slot_template.id)
            .unwrap_or(0);

        // Depending on the side and the directionality, push the slot in or out
        let offset = match position {
            SlotPosition::Left => match direction {
                SlotType::Incoming => 0.0,
                SlotType::Outgoing => -1.0,
            },
            SlotPosition::Right => match direction {
                SlotType::Incoming => 0.0,
                SlotType::Outgoing => 1.0,
            },
            SlotPosition::Top => match direction {
                SlotType::Incoming => 0.0,
                SlotType::Outgoing => -1.0,
            },
            SlotPosition::Bottom => match direction {
                SlotType::Incoming => 0.0,
                SlotType::Outgoing => 1.0,
            },
        } * self.config.slot_radius;

        // Calculate position based on slot index and total slots
        match position {
            SlotPosition::Left | SlotPosition::Right => {
                let x = if *position == SlotPosition::Left {
                    node.x
                } else {
                    node.x + node.width
                } + offset;
                let spacing = node.height / (num_slots_on_side as f64 + 1.0);
                let y = node.y + spacing * (slot_index as f64 + 1.0);
                (x, y)
            }
            SlotPosition::Top | SlotPosition::Bottom => {
                let y = if *position == SlotPosition::Top {
                    node.y
                } else {
                    node.y + node.height
                } + offset;
                let spacing = node.width / (num_slots_on_side as f64 + 1.0);
                let x = node.x + spacing * (slot_index as f64 + 1.0);
                (x, y)
            }
        }
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
        from_position: &SlotPosition,
        to_position: &SlotPosition,
    ) -> f64 {
        let (cp1_x, cp1_y) = get_bezier_control_points(
            start.0,
            start.1,
            from_position,
            self.config.connection_control_point_distance,
        );
        let (cp2_x, cp2_y) = get_bezier_control_points(
            end.0,
            end.1,
            to_position,
            self.config.connection_control_point_distance,
        );

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
