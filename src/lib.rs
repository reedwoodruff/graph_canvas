#[cfg(feature = "js")]
use errors::IntoJsError;
use errors::{GraphError, GraphResult};
use interaction::InteractionState;
use layout::LayoutEngine;
use std::sync::{Arc, Mutex};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{window, HtmlCanvasElement, HtmlDivElement};

mod common;
mod config;
mod draw;
mod errors;
mod events;
mod graph;
mod interaction;
#[cfg(feature = "js")]
mod js;
mod layout;
pub mod prelude;
mod toolbar_ui;

pub use config::GraphCanvasConfig;
pub use config::InitialConnection;
pub use config::InitialFieldValue;
pub use config::InitialNode;
pub use config::TemplateGroup;
pub use config::TemplateIdentifier;
pub use graph::Connection;
pub use graph::FieldTemplate;
pub use graph::FieldType;
pub use graph::Graph;
pub use graph::NodeInstance;
pub use graph::NodeTemplate;
pub use graph::SlotInstance;
pub use graph::SlotPosition;
pub use graph::SlotTemplate;
pub use graph::SlotType;
#[cfg(feature = "js")]
pub use js::JsInitialConnection;
#[cfg(feature = "js")]
pub use js::JsInitialFieldValue;
#[cfg(feature = "js")]
pub use js::JsPartialConfig;
#[cfg(feature = "js")]
pub use js::JsPartialInitialNode;
#[cfg(feature = "js")]
pub use js::JsPartialNodeTemplate;
#[cfg(feature = "js")]
pub use js::JsPartialSlotTemplate;
#[cfg(feature = "js")]
pub use js::JsTemplateGroup;
pub use layout::LayoutType;

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
    // settings: Arc<GraphCanvasSettings>,
    config: Arc<GraphCanvasConfig>,
    graph: Arc<Mutex<Graph>>,
    canvas_id: String,
    interaction: Arc<Mutex<InteractionState>>,
    events: Arc<Mutex<events::EventSystem>>,
    layout_engine: Arc<Mutex<LayoutEngine>>,
}
impl std::fmt::Debug for GraphCanvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphCanvas")
            .field("config", &self.config)
            .field("graph", &self.graph)
            .finish()
    }
}

#[wasm_bindgen]
impl GraphCanvas {
    #[cfg(feature = "js")]
    #[wasm_bindgen(constructor)]
    pub fn new_js(
        container: &HtmlDivElement,
        js_config: JsPartialConfig,
    ) -> Result<GraphCanvas, JsValue> {
        let config: GraphCanvasConfig = js_config.into();
        log("GraphCanvas initializing");
        let graph = Self::new_rust(container, config).map_err(|e| e.into_js_error());
        log("GraphCanvas initialized");
        graph
    }
}
impl GraphCanvas {
    pub fn new_rust(
        container: &HtmlDivElement,
        config: GraphCanvasConfig,
        // user_toolbar_container: Option<HtmlElement>,
    ) -> GraphResult<GraphCanvas> {
        console_error_panic_hook::set_once();

        let mut graph = Graph::new();

        // Register a test template
        graph.register_template(GraphCanvas::get_test_template());
        // Register templates
        for template in &config.node_templates {
            graph.register_template(template.clone());
        }

        // Create initial nodes
        graph.create_initial_nodes(&config.initial_nodes)?;

        let events = Arc::new(Mutex::new(events::EventSystem::new()));
        events.lock().unwrap().subscribe(Box::new(|event| {
            log(&format!("{:?}", event));
        }));

        let (canvas, toolbar_container) =
            GraphCanvas::create_canvas(container).map_err(GraphError::SetupFailed)?;

        // Create GraphCanvas...
        let canvas_clone = canvas.clone();
        let graph_canvas = GraphCanvas {
            config: Arc::new(config.clone()),
            interaction: Arc::new(Mutex::new(InteractionState::new())),
            graph: Arc::new(Mutex::new(graph)),
            canvas_id: canvas.id().to_string(),
            events,
            layout_engine: Arc::new(Mutex::new(LayoutEngine::new(canvas_clone.id().clone()))),
        };

        // Setup toolbar based on config
        // if config.show_default_toolbar {
        graph_canvas
            .setup_default_toolbar(&toolbar_container, &config, &graph_canvas)
            .map_err(GraphError::SetupFailed)?;
        // }
        // if let Some(custom_toolbar) = &config.custom_toolbar {
        //     toolbar_container
        //         .append_child(custom_toolbar)
        //         .map_err(|err| GraphError::SetupFailed(err))?;
        // }
        //
        graph_canvas.setup_events()?;

        // Apply force layout when the graph is first initialized
        {
            let mut layout_engine = graph_canvas.layout_engine.lock().unwrap();
            let mut graph = graph_canvas.graph.lock().unwrap();

            // If the graph has nodes, apply force layout on initialization
            if !graph.node_instances.is_empty() {
                layout_engine.switch_layout(LayoutType::ForceDirected, &mut graph);
            }
        }

        graph_canvas.start_render_loop()?;

        log(&format!("{:#?}", graph_canvas));

        Ok(graph_canvas)
    }

    fn create_canvas(
        container: &web_sys::HtmlElement,
    ) -> Result<(HtmlCanvasElement, HtmlDivElement), JsValue> {
        // Create canvas
        let document = window().unwrap().document().unwrap();
        let canvas = document.create_element("canvas")?;
        canvas.set_id("graph-canvas-canvas-element");
        let canvas = canvas.dyn_into::<HtmlCanvasElement>()?;

        // Set canvas style to fill container
        canvas.style().set_property("width", "100%")?;
        canvas.style().set_property("height", "100%")?;
        canvas.style().set_property("display", "block")?;

        let graph_container = document.create_element("div")?;
        graph_container.set_id("graph_canvas_container");
        let graph_container = graph_container.dyn_into::<web_sys::HtmlDivElement>()?;
        graph_container.style().set_property("width", "100%")?;
        graph_container.style().set_property("min-width", "400px")?;
        graph_container
            .style()
            .set_property("height", "calc(100% - 40px)")?;
        graph_container
            .style()
            .set_property("min-height", "400px")?;
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

        Ok((canvas, toolbar_container))
    }

    fn setup_default_toolbar(
        &self,
        toolbar_container: &HtmlDivElement,
        config: &GraphCanvasConfig,
        graph_canvas: &GraphCanvas,
    ) -> Result<(), JsValue> {
        let document = window().unwrap().document().unwrap();

        // Organize templates into groups
        let template_groups = if config.template_groups.is_empty() {
            // If no groups are defined, create a default group with all templates
            let mut groups = Vec::new();
            let creatable_templates = config
                .node_templates
                .iter()
                .filter(|template| template.can_create)
                .collect::<Vec<_>>();

            if !creatable_templates.is_empty() {
                groups.push(("All Templates".to_string(), creatable_templates));
            }
            groups
        } else {
            // Use the defined template groups
            config.get_template_group_map()
        };

        // Create toolbar using builder
        let toolbar_builder =
            toolbar_ui::ToolbarBuilder::new(&document, graph_canvas, template_groups);
        let elements = toolbar_builder.build()?;

        // Attach event handlers
        let event_handler =
            toolbar_ui::ToolbarEventHandler::new(&elements, &document, graph_canvas)?;
        event_handler.attach_all_handlers()?;

        // Append toolbar to container
        toolbar_container.append_child(&elements.toolbar)?;

        Ok(())
    }

    fn setup_events(&self) -> Result<(), GraphError> {
        // Mouse Down Handler
        let self_clone = self.clone();
        let canvas = window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(&self.canvas_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        let canvas_clone = canvas.clone();
        let mouse_down = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            let rect = canvas_clone.get_bounding_client_rect();
            let x = event.client_x() as f64 - rect.left();
            let y = event.client_y() as f64 - rect.top();
            self_clone.handle_mouse_down(x, y).unwrap();
        }) as Box<dyn FnMut(_)>);

        // Mouse Move Handler
        let self_clone = self.clone();
        let canvas_clone = canvas.clone();
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
        let canvas_clone = canvas.clone();
        let mouse_up = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            let rect = canvas_clone.get_bounding_client_rect();
            let x = event.client_x() as f64 - rect.left();
            let y = event.client_y() as f64 - rect.top();
            match self_clone.handle_mouse_up(x, y) {
                Ok(_) => {}
                Err(e) => log(&format!("{:?}", e.as_string())),
            }
        }) as Box<dyn FnMut(_)>);

        // Mouse Wheel Handler for zooming
        let self_clone = self.clone();
        let canvas_clone = canvas.clone();
        let wheel_handler = Closure::wrap(Box::new(move |event: web_sys::WheelEvent| {
            // Prevent default browser behavior (page scrolling)
            event.prevent_default();

            let rect = canvas_clone.get_bounding_client_rect();
            let x = event.client_x() as f64 - rect.left();
            let y = event.client_y() as f64 - rect.top();

            // Get delta (negative for zoom in, positive for zoom out)
            let delta = event.delta_y();

            match self_clone.handle_zoom(delta, x, y) {
                Ok(_) => {}
                Err(e) => log(&format!("Zoom error: {:?}", e.as_string())),
            }
        }) as Box<dyn FnMut(_)>);

        let canvas_clone = canvas.clone();
        // Add event listeners
        canvas_clone
            .add_event_listener_with_callback("mousedown", mouse_down.as_ref().unchecked_ref())
            .map_err(GraphError::SetupFailed)?;
        canvas_clone
            .add_event_listener_with_callback("mousemove", mouse_move.as_ref().unchecked_ref())
            .map_err(GraphError::SetupFailed)?;
        canvas_clone
            .add_event_listener_with_callback("mouseup", mouse_up.as_ref().unchecked_ref())
            .map_err(GraphError::SetupFailed)?;
        canvas_clone
            .add_event_listener_with_callback("wheel", wheel_handler.as_ref().unchecked_ref())
            .map_err(GraphError::SetupFailed)?;

        // Prevent memory leaks
        mouse_down.forget();
        mouse_move.forget();
        mouse_up.forget();
        wheel_handler.forget();

        Ok(())
    }

    fn get_test_template() -> NodeTemplate {
        NodeTemplate {
            can_modify_slots: true,
            min_instances: Some(1),
            max_instances: None,
            can_delete: true,
            can_create: true,
            template_id: "test_node".to_string(),
            name: "Test Node".to_string(),
            slot_templates: vec![
                SlotTemplate {
                    id: "first".to_string(),
                    name: "First".to_string(),
                    position: SlotPosition::Right,
                    slot_type: SlotType::Outgoing,
                    allowed_connections: vec!["test_node".to_string(), "Node".to_string()],
                    min_connections: 2,
                    max_connections: Some(3),
                    can_modify_connections: true,
                },
                SlotTemplate {
                    id: "second".to_string(),
                    name: "Second".to_string(),
                    position: SlotPosition::Right,
                    slot_type: SlotType::Outgoing,
                    allowed_connections: vec!["test_node".to_string(), "Node".to_string()],
                    min_connections: 2,
                    max_connections: Some(3),
                    can_modify_connections: true,
                },
                SlotTemplate {
                    id: "third".to_string(),
                    name: "Third".to_string(),
                    position: SlotPosition::Top,
                    slot_type: SlotType::Outgoing,

                    allowed_connections: vec!["test_node".to_string(), "Node".to_string()],

                    min_connections: 2,
                    max_connections: Some(3),
                    can_modify_connections: true,
                },
                SlotTemplate {
                    id: "fourth".to_string(),
                    name: "Fourth".to_string(),
                    position: SlotPosition::Top,
                    slot_type: SlotType::Outgoing,

                    allowed_connections: vec!["test_node".to_string(), "Node".to_string()],

                    min_connections: 2,
                    max_connections: Some(3),
                    can_modify_connections: true,
                },
                SlotTemplate {
                    id: "fifth".to_string(),
                    name: "Fifth".to_string(),
                    position: SlotPosition::Bottom,
                    slot_type: SlotType::Outgoing,

                    allowed_connections: vec!["test_node".to_string(), "Node".to_string()],

                    min_connections: 2,
                    max_connections: Some(3),
                    can_modify_connections: true,
                },
                SlotTemplate {
                    id: "sixth".to_string(),
                    name: "Sixth".to_string(),
                    position: SlotPosition::Bottom,
                    slot_type: SlotType::Outgoing,

                    allowed_connections: vec!["test_node".to_string(), "Node".to_string()],

                    min_connections: 2,
                    max_connections: Some(3),
                    can_modify_connections: true,
                },
                SlotTemplate {
                    id: "seventh".to_string(),
                    name: "Seventh".to_string(),
                    position: SlotPosition::Left,
                    slot_type: SlotType::Outgoing,

                    allowed_connections: vec!["test_node".to_string(), "Node".to_string()],

                    min_connections: 2,
                    max_connections: Some(3),
                    can_modify_connections: true,
                },
                SlotTemplate {
                    id: "eigth".to_string(),
                    name: "Eigth".to_string(),
                    position: SlotPosition::Left,
                    slot_type: SlotType::Outgoing,

                    allowed_connections: vec!["test_node".to_string(), "Node".to_string()],

                    min_connections: 2,
                    max_connections: Some(3),
                    can_modify_connections: true,
                },
            ],
            // Add field templates for testing
            field_templates: vec![
                FieldTemplate {
                    id: "bool_field".to_string(),
                    name: "Active".to_string(),
                    field_type: FieldType::Boolean,
                    default_value: "true".to_string(),
                },
                FieldTemplate {
                    id: "int_field".to_string(),
                    name: "Count".to_string(),
                    field_type: FieldType::Integer,
                    default_value: "42".to_string(),
                },
                FieldTemplate {
                    id: "string_field".to_string(),
                    name: "Label".to_string(),
                    field_type: FieldType::String,
                    default_value: "Test".to_string(),
                },
            ],
            default_radius: 150.0,
            default_color: "red".to_string(),
            can_modify_fields: true,
        }
    }
    pub fn save(&self) -> GraphResult<Graph> {
        let graph = self.graph.try_lock();
        if graph.is_err() {
            return Err(GraphError::SaveFailed {
                reason: Box::new(GraphError::GraphLockFailed),
            });
        }
        let graph = graph.unwrap();
        self.check_conformity(&graph)?;
        Ok(graph.clone())
    }

    fn check_conformity(&self, graph: &Graph) -> GraphResult<()> {
        let node_errors = graph
            .node_instances
            .values()
            .filter_map(|instance| {
                let template = graph
                    .get_node_template_by_identifier(&TemplateIdentifier::Id(
                        instance.template_id.clone(),
                    ))
                    .unwrap();
                let slot_errors = instance
                    .slots
                    .iter()
                    .filter_map(|slot_instance| {
                        let slot_template = template
                            .slot_templates
                            .iter()
                            .find(|slot_template| {
                                slot_template.id == slot_instance.slot_template_id
                            })
                            .unwrap();
                        if slot_template.min_connections > slot_instance.connections.len()
                            || slot_template
                                .max_connections
                                .is_some_and(|max_connections| {
                                    max_connections < slot_instance.connections.len()
                                })
                        {
                            return Some(GraphError::SlotMalformed {
                                template_name: template.name.clone(),
                                slot_name: slot_template.name.clone(),
                                min: slot_template.min_connections,
                                max: slot_template.max_connections,
                                actual: slot_instance.connections.len(),
                            });
                        }
                        None
                    })
                    .collect::<Vec<_>>();
                if slot_errors.is_empty() {
                    return None;
                }
                Some(slot_errors)
            })
            .collect::<Vec<_>>();
        if node_errors.is_empty() {
            return Ok(());
        }
        Err(GraphError::SaveFailed {
            reason: Box::new(GraphError::ListOfErrors(
                node_errors.into_iter().flatten().collect::<Vec<_>>(),
            )),
        })
    }
    pub fn apply_layout(&mut self, layout: LayoutType) -> Result<(), GraphError> {
        match self.layout_engine.lock() {
            Ok(mut engine) => {
                match self.graph.lock() {
                    Ok(mut graph) => {
                        engine.switch_layout(layout, &mut graph);
                    }
                    Err(_) => return Err(GraphError::GraphLockFailed),
                }
                Ok(())
            }
            Err(_) => Err(GraphError::GraphLockFailed),
        }
    }
}
