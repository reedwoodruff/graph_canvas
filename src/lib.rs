use graph::Graph;
use graph::NodeTemplate;
use graph::SlotPosition;
use graph::SlotTemplate;
use graph::SlotType;
use interaction::InteractionMode;
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
    fn group(s: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn groupEnd();
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
    #[wasm_bindgen(constructor)]
    pub fn new(
        container: &HtmlElement,
        user_toolbar_container: Option<HtmlElement>,
    ) -> Result<GraphCanvas, JsValue> {
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

        let graph_container = document.create_element("div")?;
        graph_container.set_id("graph_canvas_container");
        let graph_container = graph_container.dyn_into::<web_sys::HtmlDivElement>()?;
        graph_container.style().set_property("width", "100%")?;
        graph_container
            .style()
            .set_property("height", "calc(100% - 40px)")?;
        graph_container.style().set_property("display", "block")?;
        graph_container.append_child(&canvas)?;

        let toolbar_container = document.create_element("div")?;
        toolbar_container.set_id("graph_canvas_container");
        let toolbar_container = toolbar_container.dyn_into::<web_sys::HtmlDivElement>()?;
        toolbar_container.style().set_property("width", "100%")?;
        toolbar_container.style().set_property("height", "40px")?;
        toolbar_container.style().set_property("display", "block")?;

        // Add canvas to container
        container.append_child(&toolbar_container)?;
        container.append_child(&graph_container)?;

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

        // Setup toolbar: use user-provided container or create a default one.
        let toolbar = match &user_toolbar_container {
            Some(el) => el.clone(),
            None => {
                let document = window().unwrap().document().unwrap();
                let toolbar = document.create_element("div")?;
                toolbar.set_attribute("id", "graph-canvas-toolbar")?;
                toolbar.set_attribute("style", "display: flex; gap: 10px; padding: 8px;")?;

                // "Pan" button
                let pan_btn = document.create_element("button")?;
                pan_btn.set_inner_html("Pan");
                pan_btn.set_attribute("id", "btn-pan")?;
                toolbar.append_child(&pan_btn)?;

                // "Add Node" button
                let add_node_btn = document.create_element("button")?;
                add_node_btn.set_inner_html("Add Node");
                add_node_btn.set_attribute("id", "btn-add-node")?;
                toolbar.append_child(&add_node_btn)?;

                // Dropdown for selecting node template.
                let select_node = document.create_element("select")?;
                select_node.set_attribute("id", "select-node-template")?;
                // Populate the dropdown. Ideally, you’d look up the registered templates:
                let option = document.create_element("option")?;
                option.set_attribute("value", "test_node")?;
                option.set_inner_html("Test Node");
                select_node.append_child(&option)?;
                // You can add more options here if you have multiple templates.
                // Optionally, hide it initially.
                select_node.set_attribute("style", "display: none;")?;
                toolbar.append_child(&select_node)?;

                // "Cancel Add Node" button to return to default mode.
                let cancel_btn = document.create_element("button")?;
                cancel_btn.set_inner_html("Cancel");
                cancel_btn.set_attribute("id", "btn-cancel")?;
                // Hide initially.
                cancel_btn.set_attribute("style", "display: none;")?;
                toolbar.append_child(&cancel_btn)?;

                // Append toolbar above or below the canvas as needed.
                toolbar_container.append_child(&toolbar)?;
                toolbar.dyn_into::<HtmlElement>()?
            }
        };

        let mut graph = Graph::new();

        // Register a test template
        let template = NodeTemplate {
            min_instances: Some(1),
            max_instances: None,
            can_delete: true,
            can_create: true,
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

        // Create GraphCanvas...
        let canvas_clone = canvas.clone();
        let graph_canvas = GraphCanvas {
            settings: Arc::new(GraphCanvasSettings {
                context_menu_size: (400.0, 100.0),
            }),
            interaction: Arc::new(Mutex::new(InteractionState::new(&graph))),
            graph: Arc::new(Mutex::new(graph)),
            canvas: canvas_clone,
            events,
        };

        // Wire up default toolbar events if we created the toolbar
        if user_toolbar_container.is_none() {
            let document = window().unwrap().document().unwrap();

            // Pointer button
            {
                let pointer_btn = document
                    .create_element("button")?
                    .dyn_into::<HtmlElement>()?;
                pointer_btn.set_inner_html("Pointer");
                pointer_btn.set_attribute("id", "btn-pointer")?;
                // (Optionally) add styling such as margin or padding
                toolbar.append_child(&pointer_btn)?;
            }
            {
                let graph_canvas_clone = graph_canvas.clone();
                let pointer_btn = document
                    .get_element_by_id("btn-pointer")
                    .unwrap()
                    .dyn_into::<HtmlElement>()?;
                let pointer_click = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                    // This resets the interaction state back to default (Select) mode.
                    graph_canvas_clone.set_interaction_mode(InteractionMode::Default);
                    // You could also hide any extra UI related to other modes here if needed.
                    if let Some(select) = window()
                        .unwrap()
                        .document()
                        .unwrap()
                        .get_element_by_id("select-node-template")
                    {
                        select.set_attribute("style", "display: none;").unwrap();
                    }
                    // Similarly, if the UI for canceling or other features is showing, hide them.
                    if let Some(cancel) = window()
                        .unwrap()
                        .document()
                        .unwrap()
                        .get_element_by_id("btn-cancel")
                    {
                        cancel.set_attribute("style", "display: none;").unwrap();
                    }
                }) as Box<dyn FnMut(_)>);
                pointer_btn.add_event_listener_with_callback(
                    "click",
                    pointer_click.as_ref().unchecked_ref(),
                )?;
                pointer_click.forget();
            }

            // Closure for entering Pan mode.
            {
                let graph_canvas_clone = graph_canvas.clone();
                let pan_btn = document
                    .get_element_by_id("btn-pan")
                    .unwrap()
                    .dyn_into::<HtmlElement>()?;
                let pan_click = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                    // Switch the interaction mode to Pan.
                    graph_canvas_clone.set_interaction_mode(InteractionMode::Pan);
                    // Optionally hide the node select dropdown and cancel button.
                    if let Some(select) = window()
                        .unwrap()
                        .document()
                        .unwrap()
                        .get_element_by_id("select-node-template")
                    {
                        select.set_attribute("style", "display: none;").unwrap();
                    }
                    if let Some(cancel) = window()
                        .unwrap()
                        .document()
                        .unwrap()
                        .get_element_by_id("btn-cancel")
                    {
                        cancel.set_attribute("style", "display: none;").unwrap();
                    }
                }) as Box<dyn FnMut(_)>);
                pan_btn.add_event_listener_with_callback(
                    "click",
                    pan_click.as_ref().unchecked_ref(),
                )?;
                pan_click.forget();
            }

            // Closure for entering AddNode mode.
            {
                let graph_canvas_clone = graph_canvas.clone();
                let add_node_btn = document
                    .get_element_by_id("btn-add-node")
                    .unwrap()
                    .dyn_into::<HtmlElement>()?;
                let add_node_click = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                    // Switch to AddNode mode.
                    graph_canvas_clone.set_interaction_mode(InteractionMode::AddNode);
                    // Show the dropdown and cancel button so the user can choose the node template and exit AddNode mode.
                    if let Some(select) = window()
                        .unwrap()
                        .document()
                        .unwrap()
                        .get_element_by_id("select-node-template")
                    {
                        select.set_attribute("style", "display: block;").unwrap();
                    }
                    if let Some(cancel) = window()
                        .unwrap()
                        .document()
                        .unwrap()
                        .get_element_by_id("btn-cancel")
                    {
                        cancel.set_attribute("style", "display: block;").unwrap();
                    }
                }) as Box<dyn FnMut(_)>);
                add_node_btn.add_event_listener_with_callback(
                    "click",
                    add_node_click.as_ref().unchecked_ref(),
                )?;
                add_node_click.forget();
            }

            // Closure for canceling AddNode mode and returning to Select mode.
            {
                let graph_canvas_clone = graph_canvas.clone();
                let cancel_btn = document
                    .get_element_by_id("btn-cancel")
                    .unwrap()
                    .dyn_into::<HtmlElement>()?;
                let cancel_click = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                    // Switch back to default, e.g., select mode.
                    graph_canvas_clone.set_interaction_mode(InteractionMode::Default);
                    // Hide the dropdown and cancel button again.
                    if let Some(select) = window()
                        .unwrap()
                        .document()
                        .unwrap()
                        .get_element_by_id("select-node-template")
                    {
                        select.set_attribute("style", "display: none;").unwrap();
                    }
                    if let Some(cancel) = window()
                        .unwrap()
                        .document()
                        .unwrap()
                        .get_element_by_id("btn-cancel")
                    {
                        cancel.set_attribute("style", "display: none;").unwrap();
                    }
                }) as Box<dyn FnMut(_)>);
                cancel_btn.add_event_listener_with_callback(
                    "click",
                    cancel_click.as_ref().unchecked_ref(),
                )?;
                cancel_click.forget();
            }

            // Optionally: You can also attach an event listener on the dropdown to update the current node selection
            {
                let graph_canvas_clone = graph_canvas.clone();
                let select_node = document
                    .get_element_by_id("select-node-template")
                    .unwrap()
                    .dyn_into::<web_sys::HtmlSelectElement>()?;
                let select_node_clone = select_node.clone();
                let select_change = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                    let value = select_node_clone.value();
                    // Update the interaction state with the selected node template.
                    graph_canvas_clone.set_current_node_template(&value);
                }) as Box<dyn FnMut(_)>);
                select_node.add_event_listener_with_callback(
                    "change",
                    select_change.as_ref().unchecked_ref(),
                )?;
                select_change.forget();
            }
        }

        Ok(graph_canvas)
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
            let dx = event.movement_x();
            let dy = event.movement_y();
            self_clone
                .handle_mouse_move(x, y, dx as f64, dy as f64)
                .unwrap();
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
