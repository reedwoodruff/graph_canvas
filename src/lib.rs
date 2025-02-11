use graph::Graph;
use graph::NodeInstance;
use graph::NodeTemplate;
use graph::SlotInstance;
use graph::SlotPosition;
use graph::SlotTemplate;
use graph::SlotType;
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;
use web_sys::{window, CanvasRenderingContext2d, HtmlCanvasElement};

mod graph;
mod utils;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

const SLOT_DRAW_RADIUS: f64 = 7.0;

#[derive(Debug, Clone)]
pub struct DragState {
    active: bool,
    from_node: Option<String>,
    from_slot: Option<String>,
    current_x: f64,
    current_y: f64,
}

impl DragState {
    pub fn new() -> Self {
        Self {
            active: false,
            from_node: None,
            from_slot: None,
            current_x: 0.0,
            current_y: 0.0,
        }
    }
}

#[derive(Clone)]
#[wasm_bindgen]
pub struct GraphCanvas {
    graph: Arc<Mutex<Graph>>,
    canvas: HtmlCanvasElement,
    drag_state: Arc<Mutex<DragState>>,
}

#[wasm_bindgen]
impl GraphCanvas {
    #[wasm_bindgen(constructor)]
    pub fn new(container: &HtmlElement) -> Result<GraphCanvas, JsValue> {
        console_error_panic_hook::set_once();
        // Set up the canvas
        let document = window().unwrap().document().unwrap();
        let canvas = document
            .create_element("canvas")?
            .dyn_into::<HtmlCanvasElement>()?;
        canvas.set_id("GraphCanvas");
        container.append_child(&canvas)?;
        let canvas_clone = canvas.clone();

        let resize_closure = Closure::wrap(Box::new(move || {
            let container = window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id("GraphCanvas")
                .unwrap()
                .parent_element()
                .unwrap();
            let width = container.client_width() as u32;
            let height = container.client_height() as u32;
            canvas_clone.set_width(width);
            canvas_clone.set_height(height);
        }) as Box<dyn FnMut()>);

        window()
            .unwrap()
            .add_event_listener_with_callback("resize", resize_closure.as_ref().unchecked_ref())?;
        resize_closure.forget();

        let canvas_clone = canvas.clone();
        // Initial resize to set the canvas size
        let width = container.client_width() as u32;
        let height = container.client_height() as u32;
        canvas_clone.set_width(width);
        canvas_clone.set_height(height);

        let mut graph = Graph::new();

        // Register a test template
        let template = NodeTemplate {
            template_id: "test_node".to_string(),
            name: "Test Node".to_string(),
            slot_templates: vec![
                // SlotTemplate {
                //     id: "input".to_string(),
                //     name: "Input".to_string(),
                //     position: SlotPosition::Left,
                //     slot_type: SlotType::Input,
                //     allowed_connections: vec!["test_node".to_string()],
                //     min_connections: 0,
                //     max_connections: 1,
                // },
                SlotTemplate {
                    id: "output".to_string(),
                    name: "Output".to_string(),
                    position: SlotPosition::Right,
                    slot_type: SlotType::Output,
                    allowed_connections: vec!["test_node".to_string()],
                    min_connections: 2,
                    max_connections: 3,
                },
            ],
            default_width: 100.0,
            default_height: 60.0,
        };
        graph.register_template(template);

        Ok(GraphCanvas {
            graph: Arc::new(Mutex::new(graph)),
            canvas: canvas_clone,
            drag_state: Arc::new(Mutex::new(DragState::new())),
        })
    }
}

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
}

#[wasm_bindgen]
impl GraphCanvas {
    // Rendering - skips if locked
    pub fn render(&self) -> Result<(), JsValue> {
        let context = self
            .canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;

        if let (Ok(graph), Ok(drag_state)) = (self.graph.try_lock(), self.drag_state.try_lock()) {
            self.do_render(&context, &graph)?;

            // Draw in-progress connection if dragging
            if drag_state.active {
                if let (Some(from_node), Some(from_slot)) =
                    (&drag_state.from_node, &drag_state.from_slot)
                {
                    if let Some(node) = graph.get_node_capabilities(from_node) {
                        if let Some(slot) = node.instance.slots.iter().find(|s| s.id == *from_slot)
                        {
                            let slot_template = slot.capabilities(&graph).template;
                            let (start_x, start_y) = self
                                .calculate_slot_position(&slot_template.position, node.instance);

                            // Draw the in-progress connection
                            context.begin_path();
                            context.move_to(start_x, start_y);
                            context.line_to(drag_state.current_x, drag_state.current_y);
                            context.set_stroke_style_str("#666666");
                            context.set_line_width(2.0);
                            context.stroke();
                            context.set_line_width(1.0);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // User interactions - waits for lock
    pub fn add_node(&self, template_id: &str, x: f64, y: f64) -> Result<(), JsValue> {
        let mut graph = self.graph.lock().unwrap(); // This will wait
        graph.create_instance(template_id, x, y);
        // Optionally trigger a render here
        Ok(())
    }

    // Helper method to separate rendering logic
    fn do_render(&self, context: &CanvasRenderingContext2d, graph: &Graph) -> Result<(), JsValue> {
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

        Ok(())
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

    fn calculate_slot_position(&self, position: &SlotPosition, node: &NodeInstance) -> (f64, f64) {
        match position {
            SlotPosition::Left => (node.x, node.y + node.height / 2.0),
            SlotPosition::Right => (node.x + node.width, node.y + node.height / 2.0),
            SlotPosition::Top => (node.x + node.width / 2.0, node.y),
            SlotPosition::Bottom => (node.x + node.width / 2.0, node.y + node.height),
        }
    }
}
#[wasm_bindgen]
impl GraphCanvas {
    pub fn setup_events(&self) -> Result<(), JsValue> {
        // Mouse Down Handler
        let self_clone = self.clone();
        let canvas_clone = self.canvas.clone();
        let mouse_down = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            let rect = canvas_clone.get_bounding_client_rect();
            let x = event.client_x() as f64 - rect.left();
            let y = event.client_y() as f64 - rect.top();
            self_clone.handle_mouse_down(x, y).unwrap();
        }) as Box<dyn FnMut(_)>);

        // Mouse Move Handler
        let self_clone = self.clone();
        let canvas_clone = self.canvas.clone();
        let mouse_move = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            let rect = canvas_clone.get_bounding_client_rect();
            let x = event.client_x() as f64 - rect.left();
            let y = event.client_y() as f64 - rect.top();
            self_clone.handle_mouse_move(x, y).unwrap();
        }) as Box<dyn FnMut(_)>);

        // Mouse Up Handler
        let self_clone = self.clone();
        let canvas_clone = self.canvas.clone();
        let mouse_up = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            let rect = canvas_clone.get_bounding_client_rect();
            let x = event.client_x() as f64 - rect.left();
            let y = event.client_y() as f64 - rect.top();
            match self_clone.handle_mouse_up(x, y) {
                Ok(_) => {}
                Err(e) => log(&format!("{:?}", e.as_string())),
            }
        }) as Box<dyn FnMut(_)>);

        // Add event listeners
        self.canvas
            .add_event_listener_with_callback("mousedown", mouse_down.as_ref().unchecked_ref())?;
        self.canvas
            .add_event_listener_with_callback("mousemove", mouse_move.as_ref().unchecked_ref())?;
        self.canvas
            .add_event_listener_with_callback("mouseup", mouse_up.as_ref().unchecked_ref())?;

        // Prevent memory leaks
        mouse_down.forget();
        mouse_move.forget();
        mouse_up.forget();

        Ok(())
    }
}

#[wasm_bindgen]
impl GraphCanvas {
    pub fn handle_mouse_down(&self, x: f64, y: f64) -> Result<(), JsValue> {
        log("running mouse down");
        let mut graph = self
            .graph
            .lock()
            .map_err(|e| JsValue::from_str(&format!("Failed to lock graph: {}", e)))?;
        let mut drag_state = self
            .drag_state
            .lock()
            .map_err(|e| JsValue::from_str(&format!("Failed to lock drag_state: {}", e)))?;

        // Check if we clicked on a slot
        for (node_id, node) in &graph.node_instances {
            for slot in &node.slots {
                if self.is_point_in_slot(x, y, node, slot, &graph) {
                    drag_state.active = true;
                    drag_state.from_node = Some(node_id.clone());
                    drag_state.from_slot = Some(slot.id.clone());
                    drag_state.current_x = x;
                    drag_state.current_y = y;
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
                log("is in box");
                graph.selected_instance = Some(id.clone());
                graph.dragging = true;
                return Ok(());
            }
        }
        graph.selected_instance = None;
        Ok(())
    }

    pub fn handle_mouse_move(&self, x: f64, y: f64) -> Result<(), JsValue> {
        let mut drag_state = self
            .drag_state
            .lock()
            .map_err(|e| JsValue::from_str(&format!("Failed to lock drag_state: {}", e)))?;

        if drag_state.active {
            drag_state.current_x = x;
            drag_state.current_y = y;
        }

        if let Ok(mut graph) = self.graph.lock() {
            if graph.dragging {
                if let Some(ref selected_id) = graph.selected_instance.clone() {
                    if let Some(instance) = graph.node_instances.get_mut(selected_id) {
                        instance.x = x - instance.width / 2.0;
                        instance.y = y - instance.height / 2.0;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn handle_mouse_up(&self, x: f64, y: f64) -> Result<(), JsValue> {
        let mut graph = self
            .graph
            .lock()
            .map_err(|e| JsValue::from_str(&format!("Failed to lock graph: {}", e)))?;
        let mut drag_state = self
            .drag_state
            .lock()
            .map_err(|e| JsValue::from_str(&format!("Failed to lock drag_state: {}", e)))?;

        if drag_state.active {
            // Check if we're over another node
            for (target_node_id, target_node) in graph.node_instances.clone().into_iter() {
                // Don't connect to self
                if Some(target_node_id.clone()) != drag_state.from_node {
                    // Check if point is within node bounds
                    if x >= target_node.x
                        && x <= target_node.x + target_node.width
                        && y >= target_node.y
                        && y <= target_node.y + target_node.height
                    {
                        if let (Some(from_node), Some(from_slot)) =
                            (drag_state.from_node.clone(), drag_state.from_slot.clone())
                        {
                            graph.connect_slots(
                                &from_node,
                                &from_slot,
                                &target_node_id,
                                &"incoming",
                            )?;
                        }
                    }
                }
            }
        }

        // Reset drag state
        *drag_state = DragState::new();
        graph.dragging = false;

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
