use std::{cell::RefCell, f64::consts::PI, rc::Rc};
use wasm_bindgen::prelude::*;
use web_sys::{window, CanvasRenderingContext2d};

pub const SLOT_DRAW_RADIUS: f64 = 7.0;

use crate::log;
use crate::{
    graph::{Graph, NodeInstance, SlotInstance, SlotPosition, SlotTemplate, SlotType},
    interaction::{
        ContextMenu, ContextMenuAction, ContextMenuItem, ContextMenuTarget, InteractionState,
    },
    GraphCanvas,
};

/// Rendering
/// There should be no locks except in the main `do_render` method.
#[wasm_bindgen]
impl GraphCanvas {
    pub fn start_render_loop(&self) -> Result<(), JsValue> {
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
            .request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;
        Ok(())
    }

    // Rendering - skips if locked
    pub fn render(&self) -> Result<(), JsValue> {
        let context = self
            .canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;

        if let (Ok(graph), Ok(interaction)) = (self.graph.try_lock(), self.interaction.try_lock()) {
            self.do_render(&context, &graph, &interaction)?;
        }

        Ok(())
    }

    // Helper method to separate rendering logic
    fn do_render(
        &self,
        context: &CanvasRenderingContext2d,
        graph: &Graph,
        interaction: &InteractionState,
    ) -> Result<(), JsValue> {
        context.clear_rect(
            0.0,
            0.0,
            self.canvas.width() as f64,
            self.canvas.height() as f64,
        );

        self.draw_connections(context, graph)?;

        for instance in graph.node_instances.values() {
            self.draw_node(context, instance, graph)?;
        }

        // Draw context menu if it exists
        if let Some(ref menu) = interaction.context_menu {
            self.draw_context_menu(context, menu, graph)?;
        }

        // Draw dragging connection if it exists
        self.draw_dragging_connection(context, interaction, graph);
        Ok(())
    }

    fn draw_context_menu(
        &self,
        context: &CanvasRenderingContext2d,
        menu: &ContextMenu,
        graph: &Graph,
    ) -> Result<(), JsValue> {
        const PADDING: f64 = 10.0;
        const ITEM_HEIGHT: f64 = 30.0;
        const TITLE_HEIGHT: f64 = 25.0;

        // Get menu items based on target type
        let items = self.get_context_menu_items(&menu.target_type)?;
        let title = menu.target_type.get_title(graph);

        let menu_height = TITLE_HEIGHT + (items.len() as f64 * ITEM_HEIGHT) + (PADDING * 2.0);

        // Draw menu background
        context.set_fill_style_str("#ffffff");
        context.set_stroke_style_str("#000000");
        context.begin_path();
        context.rect(
            menu.x,
            menu.y,
            self.settings.context_menu_size.0,
            menu_height,
        );
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
            menu.x + self.settings.context_menu_size.0,
            menu.y + TITLE_HEIGHT,
        );
        context.stroke();

        // Draw menu items
        context.set_font("12px Arial");
        for (i, item) in items.iter().enumerate() {
            let y_pos = menu.y + TITLE_HEIGHT + (i as f64 * ITEM_HEIGHT);

            // Draw item background
            context.set_fill_style_str(&item.color);
            context.fill_text(&item.label, menu.x + PADDING, y_pos + 20.0)?;
        }

        Ok(())
    }

    fn get_context_menu_items(
        &self,
        target: &ContextMenuTarget,
    ) -> Result<Vec<ContextMenuItem>, JsValue> {
        match target {
            ContextMenuTarget::Node(_) => Ok(vec![ContextMenuItem {
                label: "Delete Node".to_string(),
                action: ContextMenuAction::Delete,
                color: "#ff0000".to_string(),
            }]),
            ContextMenuTarget::Connection { .. } => Ok(vec![ContextMenuItem {
                label: "Delete Connection".to_string(),
                action: ContextMenuAction::Delete,
                color: "#ff0000".to_string(),
            }]),
            ContextMenuTarget::Slot { .. } => Ok(vec![ContextMenuItem {
                label: "Delete All Connections".to_string(),
                action: ContextMenuAction::DeleteAllConnections,
                color: "#ff0000".to_string(),
            }]),
        }
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
                    .find(|s| s.id == *connection_drag.from_slot)
                {
                    let slot_template = slot.capabilities(&graph).template;
                    let (start_x, start_y) =
                        self.calculate_slot_position(&slot_template.position, node.instance);

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
    ) -> Result<(), JsValue> {
        // Get the template for this instance
        let template = match graph.node_templates.get(&instance.template_id) {
            Some(t) => t,
            None => return Ok(()), // Skip drawing if template not found
        };

        // Draw node rectangle
        context.begin_path();
        context.set_fill_style_str("#ffffff");
        context.set_stroke_style_str("#000000");
        context.rect(instance.x, instance.y, instance.width, instance.height);
        context.fill();
        context.stroke();

        // Draw node title
        context.set_font("14px Arial");
        context.set_text_align("center");
        context.set_fill_style_str("#000000");
        context.fill_text(
            &template.name,
            instance.x + instance.width / 2.0,
            instance.y + 20.0,
        )?;

        // Draw slots
        for (slot_instance, slot_template) in
            instance.slots.iter().zip(template.slot_templates.iter())
        {
            self.draw_slot(context, slot_instance, slot_template, instance)?;
        }

        Ok(())
    }

    fn draw_slot(
        &self,
        context: &CanvasRenderingContext2d,
        slot_instance: &SlotInstance,
        slot_template: &SlotTemplate,
        node: &NodeInstance,
    ) -> Result<(), JsValue> {
        let (x, y) = self.calculate_slot_position(&slot_template.position, node);

        // Draw slot circle
        context.begin_path();
        context.arc(x, y, SLOT_DRAW_RADIUS, 0.0, 2.0 * PI)?;

        // Color based on slot type and connection status
        let fill_color = match (
            &slot_template.slot_type,
            slot_instance.connections.is_empty(),
            slot_instance.connections.len() < slot_template.min_connections,
            slot_instance.connections.len() < slot_template.max_connections,
        ) {
            (SlotType::Input, _, _, _) => "#888888",
            (_, true, true, true) => "red",
            (_, false, true, true) => "orange",
            (_, _, false, true) => "lightgreen",
            (_, _, false, false) => "green",
            _ => "purple",
            // (true) => "#888888",
            // (false) => "#44aa44",
        };
        context.set_fill_style_str(fill_color);
        context.fill();

        // Draw slot label
        context.set_font("12px Arial");
        context.set_fill_style_str("#000000");
        match slot_template.position {
            SlotPosition::Left => {
                context.set_text_align("left");
                context.fill_text(&slot_template.name, x + 10.0, y + 4.0)?;
            }
            SlotPosition::Right => {
                context.set_text_align("right");
                context.fill_text(&slot_template.name, x - 10.0, y + 4.0)?;
            }
            SlotPosition::Top => {
                context.set_text_align("center");
                context.fill_text(&slot_template.name, x, y - 10.0)?;
            }
            SlotPosition::Bottom => {
                context.set_text_align("center");
                context.fill_text(&slot_template.name, x, y + 20.0)?;
            }
        }

        Ok(())
    }

    fn draw_connections(
        &self,
        context: &CanvasRenderingContext2d,
        graph: &Graph,
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
                            .find(|s| s.slot_template_id == connection.target_slot_id)
                        {
                            self.draw_connection(
                                context,
                                instance,
                                slot,
                                target_instance,
                                target_slot,
                                graph,
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
    ) -> Result<(), JsValue> {
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
        let (start_x, start_y) =
            self.calculate_slot_position(&from_slot_template.position, from_node);
        let (end_x, end_y) = self.calculate_slot_position(&to_slot_template.position, to_node);

        // Draw curved connection line
        context.begin_path();
        context.move_to(start_x, start_y);

        // Calculate control points for curve
        let control_distance = 50.0; // Distance of control points from endpoints
        let (cp1_x, cp1_y, cp2_x, cp2_y) =
            match (&from_slot_template.position, &to_slot_template.position) {
                (SlotPosition::Right, SlotPosition::Left) => (
                    start_x + control_distance,
                    start_y,
                    end_x - control_distance,
                    end_y,
                ),
                // Add other cases as needed
                _ => (
                    start_x + control_distance,
                    start_y,
                    end_x - control_distance,
                    end_y,
                ),
            };

        context.bezier_curve_to(cp1_x, cp1_y, cp2_x, cp2_y, end_x, end_y);
        context.set_stroke_style_str("#666666");
        context.set_line_width(2.0);
        context.stroke();
        context.set_line_width(1.0);

        Ok(())
    }
}

impl GraphCanvas {
    pub fn calculate_slot_position(
        &self,
        position: &SlotPosition,
        node: &NodeInstance,
    ) -> (f64, f64) {
        match position {
            SlotPosition::Left => (node.x, node.y + node.height / 2.0),
            SlotPosition::Right => (node.x + node.width, node.y + node.height / 2.0),
            SlotPosition::Top => (node.x + node.width / 2.0, node.y),
            SlotPosition::Bottom => (node.x + node.width / 2.0, node.y + node.height),
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

    pub(crate) fn distance_to_bezier_curve(
        &self,
        point: (f64, f64),
        start: (f64, f64),
        end: (f64, f64),
        control_distance: f64,
    ) -> f64 {
        let (cp1_x, cp1_y) = (start.0 + control_distance, start.1);
        let (cp2_x, cp2_y) = (end.0 - control_distance, end.1);

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
