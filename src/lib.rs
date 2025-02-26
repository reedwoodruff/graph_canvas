#[cfg(feature = "js")]
use errors::IntoJsError;
use errors::{GraphError, GraphResult};
use graph::Graph;
use interaction::{InteractionMode, InteractionState};
use layout::{LayoutEngine, LayoutType};
use std::collections::HashMap;
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

pub use config::GraphCanvasConfig;
pub use config::InitialConnection;
pub use config::InitialNode;
pub use config::TemplateGroup;
pub use graph::Connection;
pub use graph::FieldTemplate;
pub use graph::FieldType;
pub use graph::NodeTemplate;
pub use graph::SlotPosition;
pub use graph::SlotTemplate;
pub use graph::SlotType;
#[cfg(feature = "js")]
pub use js::JsInitialConnection;
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
    canvas: HtmlCanvasElement,
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
            canvas: canvas_clone.clone(),
            events,
            layout_engine: Arc::new(Mutex::new(LayoutEngine::new(canvas_clone))),
        };

        // Setup toolbar based on config
        // if config.show_default_toolbar {
        GraphCanvas::setup_default_toolbar(&toolbar_container, &config, &graph_canvas)
            .map_err(GraphError::SetupFailed)?;
        // }
        // if let Some(custom_toolbar) = &config.custom_toolbar {
        //     toolbar_container
        //         .append_child(custom_toolbar)
        //         .map_err(|err| GraphError::SetupFailed(err))?;
        // }
        //
        graph_canvas.setup_events()?;
        graph_canvas.start_render_loop()?;

        log(&format!("{:#?}", graph_canvas));

        Ok(graph_canvas)
    }

    fn create_canvas(
        container: &web_sys::HtmlElement,
    ) -> Result<(HtmlCanvasElement, HtmlDivElement), JsValue> {
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
        toolbar_container: &HtmlDivElement,
        config: &GraphCanvasConfig,
        graph_canvas: &GraphCanvas,
    ) -> Result<(), JsValue> {
        let document = window().unwrap().document().unwrap();

        // Create main toolbar container with improved styling
        let toolbar = document.create_element("div")?;
        toolbar.set_attribute("id", "graph-canvas-toolbar")?;
        toolbar.set_attribute("style",
            "display: flex; gap: 12px; padding: 8px; background-color: #f5f5f5; border-bottom: 1px solid #ddd; align-items: center; flex-wrap: wrap;"
        )?;

        // Create toolbar sections
        let interaction_section = document.create_element("div")?;
        interaction_section
            .set_attribute("style", "display: flex; gap: 6px; align-items: center;")?;

        let add_node_section = document.create_element("div")?;
        add_node_section.set_attribute(
            "style",
            "display: flex; gap: 6px; align-items: center; margin-left: 10px;",
        )?;

        let field_editor_section = document.create_element("div")?;
        field_editor_section.set_attribute("id", "field-editor-section")?;
        field_editor_section.set_attribute(
            "style",
            "display: none; gap: 8px; align-items: center; margin-left: 10px; padding: 4px 8px; border: 1px solid #eee; border-radius: 4px; background-color: #fff;",
        )?;

        let layout_section = document.create_element("div")?;
        layout_section.set_attribute(
            "style",
            "display: flex; gap: 6px; align-items: center; margin-left: auto;",
        )?;

        // === INTERACTION SECTION ===

        // Add section label
        let interaction_label = document.create_element("span")?;
        interaction_label.set_inner_html("Mode:");
        interaction_label.set_attribute("style", "font-size: 12px; font-weight: bold;")?;
        interaction_section.append_child(&interaction_label)?;

        // Pointer button
        let pointer_btn = document.create_element("button")?;
        pointer_btn.set_inner_html("🖱 Pointer");
        pointer_btn.set_attribute("id", "btn-pointer")?;
        pointer_btn.set_attribute("class", "toolbar-btn active")?;
        pointer_btn.set_attribute("style", "padding: 4px 8px; border: 1px solid #ccc; border-radius: 4px; background: white; cursor: pointer;")?;
        interaction_section.append_child(&pointer_btn)?;

        // Pan button
        let pan_btn = document.create_element("button")?;
        pan_btn.set_inner_html("✋ Pan");
        pan_btn.set_attribute("id", "btn-pan")?;
        pan_btn.set_attribute("class", "toolbar-btn")?;
        pan_btn.set_attribute("style", "padding: 4px 8px; border: 1px solid #ccc; border-radius: 4px; background: white; cursor: pointer;")?;
        interaction_section.append_child(&pan_btn)?;

        // === FIELD EDITOR SECTION ===

        // Create field editor title
        let field_editor_title = document.create_element("span")?;
        field_editor_title.set_attribute("id", "field-editor-title")?;
        field_editor_title.set_attribute("style", "font-size: 12px; font-weight: bold;")?;
        field_editor_title.set_inner_html("Node Fields");
        field_editor_section.append_child(&field_editor_title)?;

        // Create field editor container (will be dynamically populated)
        let field_editor_container = document.create_element("div")?;
        field_editor_container.set_attribute("id", "field-editor-container")?;
        field_editor_container
            .set_attribute("style", "display: flex; flex-direction: column; gap: 6px;")?;
        field_editor_section.append_child(&field_editor_container)?;

        // === ADD NODE SECTION ===

        // Add Node button - now styled as a dropdown trigger
        let add_node_btn = document.create_element("button")?;
        add_node_btn.set_inner_html("➕ Add Node");
        add_node_btn.set_attribute("id", "btn-add-node")?;
        add_node_btn.set_attribute("class", "toolbar-btn")?;
        add_node_btn.set_attribute("style", "padding: 4px 8px; border: 1px solid #ccc; border-radius: 4px; background: white; cursor: pointer;")?;
        add_node_section.append_child(&add_node_btn)?;

        // Template group container (visible only when in add mode)
        let template_group_container = document.create_element("div")?;
        template_group_container.set_attribute("id", "template-group-container")?;
        template_group_container.set_attribute("style", "display: none; position: absolute; top: 40px; left: 160px; background: white; border: 1px solid #ccc; border-radius: 4px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); z-index: 100; min-width: 200px; padding: 8px;")?;

        // Organize templates into groups
        let template_groups = if config.template_groups.is_empty() {
            // If no groups are defined, create a default group with all templates
            let mut groups = HashMap::new();
            let creatable_templates = config
                .node_templates
                .iter()
                .filter(|template| template.can_create)
                .collect::<Vec<_>>();

            if !creatable_templates.is_empty() {
                groups.insert("All Templates".to_string(), creatable_templates);
            }
            groups
        } else {
            // Use the defined template groups
            config.get_template_group_map()
        };

        // Set first available template as default if any exist
        let mut first_template_name = None;

        // Create tab buttons for each group
        let tab_buttons = document.create_element("div")?;
        tab_buttons.set_attribute("class", "template-group-tabs")?;
        tab_buttons.set_attribute("style", "display: flex; gap: 2px; margin-bottom: 8px; border-bottom: 1px solid #eee; padding-bottom: 4px;")?;

        let mut content_containers = Vec::new();

        for (i, (group_id, templates)) in template_groups.iter().enumerate() {
            // Skip empty groups
            if templates.is_empty() {
                continue;
            }

            // Create tab button for this group
            let tab_button = document.create_element("button")?;
            let group_name = if group_id == "other" {
                "Other".to_string()
            } else if let Some(group) = config.template_groups.iter().find(|g| &g.id == group_id) {
                group.name.clone()
            } else {
                group_id.clone()
            };

            tab_button.set_inner_html(&group_name);
            tab_button.set_attribute("data-group-id", group_id)?;
            tab_button.set_attribute(
                "class",
                if i == 0 {
                    "tab-button active"
                } else {
                    "tab-button"
                },
            )?;
            tab_button.set_attribute("style", &format!(
                "padding: 4px 8px; border: none; background: {}; cursor: pointer; border-radius: 4px 4px 0 0;",
                if i == 0 { "#f0f0f0" } else { "transparent" }
            ))?;
            tab_buttons.append_child(&tab_button)?;

            // Create content container for this group
            let content_container = document.create_element("div")?;
            content_container.set_attribute("class", "template-group-content")?;
            content_container.set_attribute("data-group-id", group_id)?;
            content_container.set_attribute(
                "style",
                &format!(
                    "display: {}; flex-direction: column; gap: 4px;",
                    if i == 0 { "flex" } else { "none" }
                ),
            )?;

            // Add template buttons to the content container
            for template in templates {
                let template_button = document.create_element("button")?;
                template_button.set_inner_html(&template.name);
                template_button.set_attribute("data-template-name", &template.name)?;
                template_button.set_attribute("class", "template-button")?;
                template_button.set_attribute("style", "padding: 6px 10px; border: 1px solid #ddd; border-radius: 4px; text-align: left; background: white; cursor: pointer; margin: 2px 0;")?;
                content_container.append_child(&template_button)?;

                // Remember the first template name for default selection
                if first_template_name.is_none() {
                    first_template_name = Some(template.name.clone());
                }
            }

            template_group_container.append_child(&content_container)?;
            content_containers.push(content_container);
        }

        template_group_container.append_child(&tab_buttons)?;
        for container in content_containers {
            template_group_container.append_child(&container)?;
        }

        // Set the first template as default if available
        if let Some(template_name) = first_template_name {
            graph_canvas.set_current_node_template(&template_name);
        }

        // Cancel button
        let cancel_btn = document.create_element("button")?;
        cancel_btn.set_inner_html("Cancel");
        cancel_btn.set_attribute("id", "btn-cancel")?;
        cancel_btn.set_attribute("style", "display: none; padding: 4px 8px; border: 1px solid #ccc; border-radius: 4px; background: white; cursor: pointer;")?;
        add_node_section.append_child(&cancel_btn)?;

        // Add the template group container to the add node section
        add_node_section.append_child(&template_group_container)?;

        // === LAYOUT SECTION ===

        // Layout label
        let layout_label = document.create_element("span")?;
        layout_label.set_inner_html("Layout:");
        layout_label.set_attribute("style", "font-size: 12px; font-weight: bold;")?;
        layout_section.append_child(&layout_label)?;

        // Layout type selector
        let layout_select = document.create_element("select")?;
        layout_select.set_attribute(
            "style",
            "padding: 4px 8px; border: 1px solid #ccc; border-radius: 4px; background: white;",
        )?;
        layout_select.set_inner_html(
            r#"
            <option value="free">Free Layout</option>
            <option value="hierarchical">Hierarchical Layout</option>
            "#,
        );
        layout_section.append_child(&layout_select)?;

        // Reset layout button
        let reset_layout_btn = document.create_element("button")?;
        reset_layout_btn.set_inner_html("Reset");
        reset_layout_btn.set_attribute("style", "padding: 4px 8px; border: 1px solid #ccc; border-radius: 4px; background: white; cursor: pointer;")?;
        layout_section.append_child(&reset_layout_btn)?;

        // Add all sections to the toolbar
        toolbar.append_child(&interaction_section)?;
        toolbar.append_child(&add_node_section)?;
        toolbar.append_child(&field_editor_section)?; // Add the field editor section
        toolbar.append_child(&layout_section)?;

        // Append toolbar to container
        toolbar_container.append_child(&toolbar)?;

        // ===== EVENT HANDLERS =====

        // Mode switching buttons (pointer, pan)
        {
            let graph_canvas_clone = graph_canvas.clone();
            let pointer_btn_clone = pointer_btn.clone();
            let pan_btn_clone = pan_btn.clone();
            let template_group_container_clone = template_group_container.clone();
            let cancel_btn_clone = cancel_btn.clone();

            // Pointer button click
            let pointer_click = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                // Switch to default mode
                graph_canvas_clone.set_interaction_mode(InteractionMode::Default);

                // Update button styles
                pointer_btn_clone
                    .set_attribute("class", "toolbar-btn active")
                    .unwrap();
                pan_btn_clone.set_attribute("class", "toolbar-btn").unwrap();

                // Hide add node UI
                template_group_container_clone
                    .set_attribute("style", "display: none;")
                    .unwrap();
                cancel_btn_clone
                    .set_attribute("style", "display: none;")
                    .unwrap();
            }) as Box<dyn FnMut(_)>);

            pointer_btn.add_event_listener_with_callback(
                "click",
                pointer_click.as_ref().unchecked_ref(),
            )?;
            pointer_click.forget();
        }

        {
            let graph_canvas_clone = graph_canvas.clone();
            let pointer_btn_clone = pointer_btn.clone();
            let pan_btn_clone = pan_btn.clone();
            let template_group_container_clone = template_group_container.clone();
            let cancel_btn_clone = cancel_btn.clone();

            // Pan button click
            let pan_click = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                // Switch to pan mode
                graph_canvas_clone.set_interaction_mode(InteractionMode::Pan);

                // Update button styles
                pointer_btn_clone
                    .set_attribute("class", "toolbar-btn")
                    .unwrap();
                pan_btn_clone
                    .set_attribute("class", "toolbar-btn active")
                    .unwrap();

                // Hide add node UI
                template_group_container_clone
                    .set_attribute("style", "display: none;")
                    .unwrap();
                cancel_btn_clone
                    .set_attribute("style", "display: none;")
                    .unwrap();
            }) as Box<dyn FnMut(_)>);

            pan_btn
                .add_event_listener_with_callback("click", pan_click.as_ref().unchecked_ref())?;
            pan_click.forget();
        }

        // Add Node button - toggle template group container
        {
            let graph_canvas_clone = graph_canvas.clone();
            let pointer_btn_clone = pointer_btn.clone();
            let pan_btn_clone = pan_btn.clone();
            let template_group_container_clone = template_group_container.clone();
            let cancel_btn_clone = cancel_btn.clone();

            let add_node_click = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                // Switch to add node mode
                graph_canvas_clone.set_interaction_mode(InteractionMode::AddNode);

                // Update button styles
                pointer_btn_clone
                    .set_attribute("class", "toolbar-btn")
                    .unwrap();
                pan_btn_clone.set_attribute("class", "toolbar-btn").unwrap();

                // Show template selection UI
                template_group_container_clone.set_attribute("style",
                    "display: block; position: absolute; top: 40px; left: 160px; background: white; border: 1px solid #ccc; border-radius: 4px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); z-index: 100; min-width: 200px; padding: 8px;"
                ).unwrap();
                cancel_btn_clone.set_attribute("style", "display: inline-block; padding: 4px 8px; border: 1px solid #ccc; border-radius: 4px; background: white; cursor: pointer;").unwrap();
            }) as Box<dyn FnMut(_)>);

            add_node_btn.add_event_listener_with_callback(
                "click",
                add_node_click.as_ref().unchecked_ref(),
            )?;
            add_node_click.forget();
        }

        // Cancel button
        {
            let graph_canvas_clone = graph_canvas.clone();
            let pointer_btn_clone = pointer_btn.clone();
            let template_group_container_clone = template_group_container.clone();
            let cancel_btn_clone = cancel_btn.clone();

            let cancel_click = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                // Switch back to default mode
                graph_canvas_clone.set_interaction_mode(InteractionMode::Default);

                // Update button styles
                pointer_btn_clone
                    .set_attribute("class", "toolbar-btn active")
                    .unwrap();

                // Hide template UI
                template_group_container_clone
                    .set_attribute("style", "display: none;")
                    .unwrap();
                cancel_btn_clone
                    .set_attribute("style", "display: none;")
                    .unwrap();
            }) as Box<dyn FnMut(_)>);

            cancel_btn
                .add_event_listener_with_callback("click", cancel_click.as_ref().unchecked_ref())?;
            cancel_click.forget();
        }

        // Tab button click handlers
        let tab_buttons = template_group_container.query_selector_all(".tab-button")?;
        for i in 0..tab_buttons.length() {
            let tab_button = tab_buttons
                .get(i)
                .unwrap()
                .dyn_into::<web_sys::HtmlElement>()?;
            let tab_button_clone = tab_button.clone();
            let template_group_container_clone = template_group_container.clone();

            let tab_click = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                let group_id = tab_button_clone.get_attribute("data-group-id").unwrap();

                // Update active tab button
                let tab_buttons = template_group_container_clone
                    .query_selector_all(".tab-button")
                    .unwrap();
                for j in 0..tab_buttons.length() {
                    let button = tab_buttons
                        .get(j)
                        .unwrap()
                        .dyn_into::<web_sys::HtmlElement>()
                        .unwrap();
                    button.set_attribute("class", "tab-button").unwrap();
                    button
                        .style()
                        .set_property("background", "transparent")
                        .unwrap();
                }
                tab_button_clone
                    .set_attribute("class", "tab-button active")
                    .unwrap();
                tab_button_clone
                    .style()
                    .set_property("background", "#f0f0f0")
                    .unwrap();

                // Show corresponding content container
                let content_containers = template_group_container_clone
                    .query_selector_all(".template-group-content")
                    .unwrap();
                for j in 0..content_containers.length() {
                    let container = content_containers
                        .get(j)
                        .unwrap()
                        .dyn_into::<web_sys::HtmlElement>()
                        .unwrap();
                    let container_group_id = container.get_attribute("data-group-id").unwrap();

                    if container_group_id == group_id {
                        container.style().set_property("display", "flex").unwrap();
                    } else {
                        container.style().set_property("display", "none").unwrap();
                    }
                }
            }) as Box<dyn FnMut(_)>);

            tab_button
                .add_event_listener_with_callback("click", tab_click.as_ref().unchecked_ref())?;
            tab_click.forget();
        }

        // Template button click handlers
        let template_buttons = template_group_container.query_selector_all(".template-button")?;
        for i in 0..template_buttons.length() {
            let template_button = template_buttons
                .get(i)
                .unwrap()
                .dyn_into::<web_sys::HtmlElement>()?;
            let template_button_clone = template_button.clone();
            let graph_canvas_clone = graph_canvas.clone();
            let template_group_container_clone = template_group_container.clone();

            let template_click = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                let template_name = template_button_clone
                    .get_attribute("data-template-name")
                    .unwrap();

                // Set the selected template
                graph_canvas_clone.set_current_node_template(&template_name);

                // Keep the template selection UI open so user can continue adding nodes
                // Optional: could close it here if preferred
            }) as Box<dyn FnMut(_)>);

            template_button.add_event_listener_with_callback(
                "click",
                template_click.as_ref().unchecked_ref(),
            )?;
            template_click.forget();
        }

        // Layout controls
        {
            let graph_canvas_clone = graph_canvas.clone();
            let on_layout_change = Closure::wrap(Box::new(move |event: web_sys::Event| {
                let target = event
                    .target()
                    .unwrap()
                    .dyn_into::<web_sys::HtmlSelectElement>()
                    .unwrap();
                let layout_type = match target.value().as_str() {
                    "hierarchical" => LayoutType::Hierarchical,
                    _ => LayoutType::Free,
                };

                let mut layout_engine = graph_canvas_clone.layout_engine.lock().unwrap();
                let mut graph = graph_canvas_clone.graph.lock().unwrap();
                layout_engine.switch_layout(layout_type, &mut graph);
            }) as Box<dyn FnMut(_)>);

            layout_select.add_event_listener_with_callback(
                "change",
                on_layout_change.as_ref().unchecked_ref(),
            )?;
            on_layout_change.forget();
        }

        {
            let graph_canvas_clone = graph_canvas.clone();
            let on_reset = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
                let mut layout_engine = graph_canvas_clone.layout_engine.lock().unwrap();
                let mut graph = graph_canvas_clone.graph.lock().unwrap();
                let mut ix = graph_canvas_clone.interaction.lock().unwrap();
                layout_engine.reset_current_layout(&mut graph, &mut ix);
            }) as Box<dyn FnMut(_)>);

            reset_layout_btn
                .add_event_listener_with_callback("click", on_reset.as_ref().unchecked_ref())?;
            on_reset.forget();
        }

        // Setup custom event listener for node selection to show field editor
        let document_clone = document.clone();
        let field_editor_section_clone = field_editor_section
            .clone()
            .dyn_into::<web_sys::HtmlDivElement>()?;
        let field_editor_container_clone = field_editor_container.clone();
        let graph_canvas_clone = graph_canvas.clone();

        // Custom event handler for node selection
        let node_selection_handler = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            let graph = graph_canvas_clone.graph.lock().unwrap();
            let interaction = graph_canvas_clone.interaction.lock().unwrap();

            // Check if a node is selected (clicked on)
            if let Some(selected_node_id) = &interaction.click_initiated_on_node {
                if let Some(node_instance) = graph.node_instances.get(selected_node_id) {
                    if !node_instance.fields.is_empty() {
                        // Get the node template to access field templates
                        if let Some(node_template) =
                            graph.node_templates.get(&node_instance.template_id)
                        {
                            // Show the field editor section
                            field_editor_section_clone
                                .style()
                                .set_property("display", "flex")
                                .unwrap();

                            // Set the title with node name
                            let title_elem = document_clone
                                .get_element_by_id("field-editor-title")
                                .unwrap();
                            title_elem.set_inner_html(&format!("{} Fields:", node_template.name));

                            // Clear existing fields
                            field_editor_container_clone.set_inner_html("");

                            // Create field editor UI for each field
                            for field_instance in &node_instance.fields {
                                // Get the corresponding field template
                                if let Some(field_template) = node_template
                                    .field_templates
                                    .iter()
                                    .find(|ft| ft.id == field_instance.field_template_id)
                                {
                                    // Create field container
                                    let field_container =
                                        document_clone.create_element("div").unwrap();
                                    field_container
                                        .set_attribute(
                                            "style",
                                            "display: flex; align-items: center; gap: 6px;",
                                        )
                                        .unwrap();

                                    // Create field label
                                    let field_label =
                                        document_clone.create_element("label").unwrap();
                                    field_label
                                        .set_inner_html(&format!("{}:", field_template.name));
                                    field_label
                                        .set_attribute("style", "min-width: 70px; font-size: 12px;")
                                        .unwrap();
                                    field_container.append_child(&field_label).unwrap();

                                    // Create field input based on type
                                    match field_template.field_type {
                                        FieldType::Boolean => {
                                            let checkbox =
                                                document_clone.create_element("input").unwrap();
                                            checkbox.set_attribute("type", "checkbox").unwrap();
                                            checkbox
                                                .set_attribute(
                                                    "data-field-id",
                                                    &field_instance.field_template_id,
                                                )
                                                .unwrap();
                                            checkbox
                                                .set_attribute("data-node-id", selected_node_id)
                                                .unwrap();

                                            if field_instance.value == "true" {
                                                checkbox
                                                    .dyn_ref::<web_sys::HtmlInputElement>()
                                                    .unwrap()
                                                    .set_checked(true);
                                            }

                                            // Add change event listener
                                            let graph_canvas_clone2 = graph_canvas_clone.clone();
                                            let field_id = field_instance.field_template_id.clone();
                                            let node_id = selected_node_id.clone();

                                            let change_callback = Closure::wrap(Box::new(
                                                move |event: web_sys::Event| {
                                                    let checked = event
                                                        .target()
                                                        .unwrap()
                                                        .dyn_into::<web_sys::HtmlInputElement>()
                                                        .unwrap()
                                                        .checked();

                                                    let mut graph =
                                                        graph_canvas_clone2.graph.lock().unwrap();
                                                    let events =
                                                        graph_canvas_clone2.events.lock().unwrap();

                                                    // Update the field value
                                                    graph.execute_command(
                                                    crate::graph::GraphCommand::UpdateField {
                                                        node_id: node_id.clone(),
                                                        field_template_id: field_id.clone(),
                                                        new_value: if checked { "true".to_string() } else { "false".to_string() },
                                                    },
                                                    &events,
                                                ).unwrap_or_else(|_| log("Failed to update boolean field"));
                                                },
                                            )
                                                as Box<dyn FnMut(_)>);

                                            checkbox
                                                .add_event_listener_with_callback(
                                                    "change",
                                                    change_callback.as_ref().unchecked_ref(),
                                                )
                                                .unwrap();
                                            change_callback.forget();

                                            field_container.append_child(&checkbox).unwrap();
                                        }
                                        FieldType::Integer => {
                                            let number_input =
                                                document_clone.create_element("input").unwrap();
                                            number_input.set_attribute("type", "number").unwrap();
                                            number_input
                                                .set_attribute("value", &field_instance.value)
                                                .unwrap();
                                            number_input
                                                .set_attribute(
                                                    "data-field-id",
                                                    &field_instance.field_template_id,
                                                )
                                                .unwrap();
                                            number_input
                                                .set_attribute("data-node-id", selected_node_id)
                                                .unwrap();
                                            number_input
                                                .set_attribute("style", "width: 60px;")
                                                .unwrap();

                                            // Add change event listener
                                            let graph_canvas_clone2 = graph_canvas_clone.clone();
                                            let field_id = field_instance.field_template_id.clone();
                                            let node_id = selected_node_id.clone();

                                            let change_callback = Closure::wrap(Box::new(
                                                move |event: web_sys::Event| {
                                                    let value = event
                                                        .target()
                                                        .unwrap()
                                                        .dyn_into::<web_sys::HtmlInputElement>()
                                                        .unwrap()
                                                        .value();

                                                    let mut graph =
                                                        graph_canvas_clone2.graph.lock().unwrap();
                                                    let events =
                                                        graph_canvas_clone2.events.lock().unwrap();

                                                    // Update the field value
                                                    graph.execute_command(
                                                    crate::graph::GraphCommand::UpdateField {
                                                        node_id: node_id.clone(),
                                                        field_template_id: field_id.clone(),
                                                        new_value: value,
                                                    },
                                                    &events,
                                                ).unwrap_or_else(|_| log("Failed to update integer field"));
                                                },
                                            )
                                                as Box<dyn FnMut(_)>);

                                            number_input
                                                .add_event_listener_with_callback(
                                                    "change",
                                                    change_callback.as_ref().unchecked_ref(),
                                                )
                                                .unwrap();
                                            change_callback.forget();

                                            field_container.append_child(&number_input).unwrap();
                                        }
                                        FieldType::String => {
                                            let text_input =
                                                document_clone.create_element("input").unwrap();
                                            text_input.set_attribute("type", "text").unwrap();
                                            text_input
                                                .set_attribute("value", &field_instance.value)
                                                .unwrap();
                                            text_input
                                                .set_attribute(
                                                    "data-field-id",
                                                    &field_instance.field_template_id,
                                                )
                                                .unwrap();
                                            text_input
                                                .set_attribute("data-node-id", selected_node_id)
                                                .unwrap();
                                            text_input
                                                .set_attribute("style", "width: 120px;")
                                                .unwrap();

                                            // Add change event listener
                                            let graph_canvas_clone2 = graph_canvas_clone.clone();
                                            let field_id = field_instance.field_template_id.clone();
                                            let node_id = selected_node_id.clone();

                                            let change_callback = Closure::wrap(Box::new(
                                                move |event: web_sys::Event| {
                                                    let value = event
                                                        .target()
                                                        .unwrap()
                                                        .dyn_into::<web_sys::HtmlInputElement>()
                                                        .unwrap()
                                                        .value();

                                                    let mut graph =
                                                        graph_canvas_clone2.graph.lock().unwrap();
                                                    let events =
                                                        graph_canvas_clone2.events.lock().unwrap();

                                                    // Update the field value
                                                    graph.execute_command(
                                                    crate::graph::GraphCommand::UpdateField {
                                                        node_id: node_id.clone(),
                                                        field_template_id: field_id.clone(),
                                                        new_value: value,
                                                    },
                                                    &events,
                                                ).unwrap_or_else(|_| log("Failed to update string field"));
                                                },
                                            )
                                                as Box<dyn FnMut(_)>);

                                            text_input
                                                .add_event_listener_with_callback(
                                                    "change",
                                                    change_callback.as_ref().unchecked_ref(),
                                                )
                                                .unwrap();
                                            change_callback.forget();

                                            field_container.append_child(&text_input).unwrap();
                                        }
                                    }

                                    // Add field container to the editor
                                    field_editor_container_clone
                                        .append_child(&field_container)
                                        .unwrap();
                                }
                            }
                        }
                    } else {
                        // Hide field editor if no fields
                        field_editor_section_clone
                            .style()
                            .set_property("display", "none")
                            .unwrap();
                    }
                } else {
                    // Hide field editor if node not found
                    field_editor_section_clone
                        .style()
                        .set_property("display", "none")
                        .unwrap();
                }
            } else {
                // Hide field editor if no node selected
                field_editor_section_clone
                    .style()
                    .set_property("display", "none")
                    .unwrap();
            }
        }) as Box<dyn FnMut(_)>);

        // Attach the handler to the canvas's mouseup event
        graph_canvas.canvas.add_event_listener_with_callback(
            "mouseup",
            node_selection_handler.as_ref().unchecked_ref(),
        )?;
        node_selection_handler.forget();

        Ok(())
    }

    fn setup_events(&self) -> Result<(), GraphError> {
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
            .add_event_listener_with_callback("mousedown", mouse_down.as_ref().unchecked_ref())
            .map_err(GraphError::SetupFailed)?;
        self.canvas
            .add_event_listener_with_callback("mousemove", mouse_move.as_ref().unchecked_ref())
            .map_err(GraphError::SetupFailed)?;
        self.canvas
            .add_event_listener_with_callback("mouseup", mouse_up.as_ref().unchecked_ref())
            .map_err(GraphError::SetupFailed)?;

        // Prevent memory leaks
        mouse_down.forget();
        mouse_move.forget();
        mouse_up.forget();

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
            default_width: 150.0,
            default_height: 130.0, // Increased height to fit fields
            can_modify_fields: true,
        }
    }
}
