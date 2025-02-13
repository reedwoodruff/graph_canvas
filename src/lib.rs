use graph::Graph;
use graph::NodeTemplate;
use graph::SlotPosition;
use graph::SlotTemplate;
use graph::SlotType;
use interaction::InteractionState;
use std::sync::Arc;
use std::sync::Mutex;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;
use web_sys::{window, HtmlCanvasElement};

mod common;
mod draw;
mod errors;
mod events;
mod graph;
mod interaction;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn warn(s: &str);
}

#[derive(Clone)]
#[wasm_bindgen]
pub struct GraphCanvas {
    settings: Arc<GraphCanvasSettings>,
    graph: Arc<Mutex<Graph>>,
    canvas: HtmlCanvasElement,
    interaction: Arc<Mutex<InteractionState>>,
    events: Arc<Mutex<events::EventSystem>>,
}

#[derive(Clone)]
pub struct GraphCanvasSettings {
    context_menu_size: (f64, f64),
}

#[wasm_bindgen]
impl GraphCanvas {
    // User interactions - waits for lock
    pub fn add_node(&self, template_id: &str, x: f64, y: f64) -> Result<(), JsValue> {
        let mut graph = self.graph.lock().unwrap();
        graph.create_instance(template_id, x, y);
        // Optionally trigger a render here
        Ok(())
    }
    #[wasm_bindgen(constructor)]
    pub fn new(container: &HtmlElement) -> Result<GraphCanvas, JsValue> {
        console_error_panic_hook::set_once();

        // Create canvas
        let document = window().unwrap().document().unwrap();
        let canvas = document
            .create_element("canvas")?
            .dyn_into::<HtmlCanvasElement>()?;

        // Set canvas style to fill container
        canvas.style().set_property("width", "100%")?;
        canvas.style().set_property("height", "100%")?;
        canvas.style().set_property("display", "block")?;

        // Add canvas to container
        container.append_child(&canvas)?;

        // Create resize observer
        let canvas_weak = canvas.clone();
        let resize_callback = Closure::wrap(Box::new(move |_: js_sys::Array| {
            let canvas = canvas_weak.clone();
            let parent = canvas.parent_element().unwrap();

            // Get parent's client dimensions
            let width = parent.client_width();
            let height = parent.client_height();

            // Update canvas dimensions
            canvas.set_width(width as u32);
            canvas.set_height(height as u32);
        }) as Box<dyn FnMut(js_sys::Array)>);

        let observer = web_sys::ResizeObserver::new(resize_callback.as_ref().unchecked_ref())?;
        observer.observe(container);

        // Keep the closure alive
        resize_callback.forget();

        // Initial size
        canvas.set_width(container.client_width() as u32);

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
                    id: "first".to_string(),
                    name: "First".to_string(),
                    position: SlotPosition::Right,
                    slot_type: SlotType::Outgoing,
                    allowed_connections: vec!["test_node".to_string()],
                    min_connections: 2,
                    max_connections: 3,
                },
                SlotTemplate {
                    id: "second".to_string(),
                    name: "Second".to_string(),
                    position: SlotPosition::Right,
                    slot_type: SlotType::Outgoing,
                    allowed_connections: vec!["test_node".to_string()],
                    min_connections: 2,
                    max_connections: 3,
                },
                SlotTemplate {
                    id: "third".to_string(),
                    name: "Third".to_string(),
                    position: SlotPosition::Top,
                    slot_type: SlotType::Outgoing,
                    allowed_connections: vec!["test_node".to_string()],
                    min_connections: 2,
                    max_connections: 3,
                },
                SlotTemplate {
                    id: "fourth".to_string(),
                    name: "Fourth".to_string(),
                    position: SlotPosition::Top,
                    slot_type: SlotType::Outgoing,
                    allowed_connections: vec!["test_node".to_string()],
                    min_connections: 2,
                    max_connections: 3,
                },
                SlotTemplate {
                    id: "fifth".to_string(),
                    name: "Fifth".to_string(),
                    position: SlotPosition::Bottom,
                    slot_type: SlotType::Outgoing,
                    allowed_connections: vec!["test_node".to_string()],
                    min_connections: 2,
                    max_connections: 3,
                },
                SlotTemplate {
                    id: "sixth".to_string(),
                    name: "Sixth".to_string(),
                    position: SlotPosition::Bottom,
                    slot_type: SlotType::Outgoing,
                    allowed_connections: vec!["test_node".to_string()],
                    min_connections: 2,
                    max_connections: 3,
                },
                SlotTemplate {
                    id: "seventh".to_string(),
                    name: "Seventh".to_string(),
                    position: SlotPosition::Left,
                    slot_type: SlotType::Outgoing,
                    allowed_connections: vec!["test_node".to_string()],
                    min_connections: 2,
                    max_connections: 3,
                },
                SlotTemplate {
                    id: "eigth".to_string(),
                    name: "Eigth".to_string(),
                    position: SlotPosition::Left,
                    slot_type: SlotType::Outgoing,
                    allowed_connections: vec!["test_node".to_string()],
                    min_connections: 2,
                    max_connections: 3,
                },
            ],
            default_width: 150.0,
            default_height: 100.0,
        };
        graph.register_template(template);

        let events = Arc::new(Mutex::new(events::EventSystem::new()));
        events.lock().unwrap().subscribe(Box::new(|event| {
            log(&format!("{:?}", event));
        }));

        let canvas_clone = canvas.clone();
        Ok(GraphCanvas {
            settings: Arc::new(GraphCanvasSettings {
                context_menu_size: (400.0, 100.0),
            }),
            graph: Arc::new(Mutex::new(graph)),
            canvas: canvas_clone,
            interaction: Arc::new(Mutex::new(InteractionState::new())),
            events,
        })
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
