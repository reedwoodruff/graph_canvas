use crate::interaction::InteractionMode;
use crate::layout::LayoutType;
use crate::{log, GraphCanvas};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{window, Document, Element, HtmlElement, HtmlInputElement};

// --- Style constants ---
pub struct Styles {
    pub button: &'static str,
    pub button_active: &'static str,
    pub section: &'static str,
    pub toolbar: &'static str,
    pub label: &'static str,
    pub field_editor: &'static str,
    pub template_container: &'static str,
    pub template_button: &'static str,
}

impl Styles {
    pub fn default() -> Self {
        Self {
                button: "padding: 4px 8px; border: 1px solid #ccc; border-radius: 4px; background: white; cursor: pointer;",
                button_active: "padding: 4px 8px; border: 1px solid #ccc; border-radius: 4px; background: #e6f7ff; cursor: pointer;",
                section: "display: flex; gap: 6px; align-items: center;",
                toolbar: "display: flex; gap: 12px; padding: 8px; background-color: #f5f5f5; border-bottom: 1px solid #ddd; align-items: center; flex-wrap: wrap;",
                label: "font-size: 12px; font-weight: bold;",
                field_editor: "display: none; gap: 8px; align-items: center; margin-left: 10px; padding: 4px 8px; border: 1px solid #eee; border-radius: 4px; background-color: #fff;",
                template_container: "display: none; position: absolute; top: 40px; left: 160px; background: white; border: 1px solid #ccc; border-radius: 4px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); z-index: 100; min-width: 200px; padding: 8px;",
                template_button: "padding: 2px 10px; border: 1px solid #ddd; border-radius: 4px; text-align: left; background: white; cursor: pointer; margin: 2px 0;",
            }
    }
}

// --- Helper functions to create UI elements ---
pub fn create_element<T: JsCast>(
    document: &Document,
    tag: &str,
    id: Option<&str>,
    class: Option<&str>,
    style: Option<&str>,
) -> Result<T, JsValue> {
    let element = document.create_element(tag)?;

    if let Some(id_value) = id {
        element.set_attribute("id", id_value)?;
    }

    if let Some(class_value) = class {
        element.set_attribute("class", class_value)?;
    }

    if let Some(style_value) = style {
        element.set_attribute("style", style_value)?;
    }

    element.dyn_into::<T>().map_err(|e| e.into())
}

pub fn create_button(
    document: &Document,
    text: &str,
    id: Option<&str>,
    class: Option<&str>,
    style: Option<&str>,
) -> Result<HtmlElement, JsValue> {
    let button: HtmlElement = create_element(document, "button", id, class, style)?;
    button.set_inner_html(text);
    Ok(button)
}

pub fn create_section(
    document: &Document,
    id: Option<&str>,
    style: Option<&str>,
    margin_left: Option<&str>,
) -> Result<HtmlElement, JsValue> {
    let section: HtmlElement = create_element(document, "div", id, None, None)?;

    let mut style_value = style.unwrap_or("").to_string();
    if let Some(margin) = margin_left {
        style_value.push_str(&format!("; margin-left: {}", margin));
    }

    section.set_attribute("style", &style_value)?;
    Ok(section)
}

pub fn create_label(
    document: &Document,
    text: &str,
    style: Option<&str>,
) -> Result<HtmlElement, JsValue> {
    let label: HtmlElement = create_element(document, "span", None, None, style)?;
    label.set_inner_html(text);
    Ok(label)
}

// --- Struct to hold all UI elements for access during event binding ---
pub struct ToolbarElements {
    pub toolbar: HtmlElement,
    pub pointer_btn: HtmlElement,
    pub add_node_btn: HtmlElement,
    pub cancel_btn: HtmlElement,
    pub template_group_container: HtmlElement,
    pub template_buttons: Vec<HtmlElement>,
    pub tab_buttons: Vec<HtmlElement>,
    pub view_buttons: Vec<HtmlElement>,
    pub layout_buttons: Vec<(HtmlElement, String)>,
    pub reset_btn: HtmlElement,
    pub physics_checkbox: HtmlInputElement,
    pub field_editor_section: HtmlElement,
    pub field_editor_container: Element,
}

// --- ToolbarBuilder for toolbar creation ---
pub struct ToolbarBuilder<'a> {
    document: &'a Document,
    styles: Styles,
    graph_canvas: &'a GraphCanvas,
    template_groups: Vec<(String, Vec<&'a crate::NodeTemplate>)>,
}

impl<'a> ToolbarBuilder<'a> {
    pub fn new(
        document: &'a Document,
        graph_canvas: &'a GraphCanvas,
        template_groups: Vec<(String, Vec<&'a crate::NodeTemplate>)>,
    ) -> Self {
        Self {
            document,
            styles: Styles::default(),
            graph_canvas,
            template_groups,
        }
    }

    pub fn build(&self) -> Result<ToolbarElements, JsValue> {
        let toolbar: HtmlElement = create_element(
            self.document,
            "div",
            Some("graph-canvas-toolbar"),
            None,
            Some(self.styles.toolbar),
        )?;

        // Create all sections with their elements
        let (interaction_section, pointer_btn) = self.create_interaction_section()?;
        let (
            add_node_section,
            add_node_btn,
            cancel_btn,
            template_group_container,
            tab_buttons,
            template_buttons,
        ) = self.create_add_node_section()?;
        let (field_editor_section, field_editor_container) = self.create_field_editor_section()?;
        let (layout_section, view_buttons, layout_buttons, reset_btn, physics_checkbox) =
            self.create_layout_section()?;

        // Add sections to toolbar
        toolbar.append_child(&interaction_section)?;
        toolbar.append_child(&add_node_section)?;
        toolbar.append_child(&field_editor_section)?;
        toolbar.append_child(&layout_section)?;

        // Collect all created elements to return
        Ok(ToolbarElements {
            toolbar,
            pointer_btn,
            add_node_btn,
            cancel_btn,
            template_group_container,
            template_buttons,
            tab_buttons,
            view_buttons,
            layout_buttons,
            reset_btn,
            physics_checkbox,
            field_editor_section,
            field_editor_container,
        })
    }

    fn create_interaction_section(&self) -> Result<(HtmlElement, HtmlElement), JsValue> {
        let section = create_section(self.document, None, Some(self.styles.section), None)?;

        // Add label
        let label = create_label(self.document, "Mode:", Some(self.styles.label))?;
        section.append_child(&label)?;

        // Pointer button
        let pointer_btn = create_button(
            self.document,
            "🖱 Pointer",
            Some("btn-pointer"),
            Some("toolbar-btn active"),
            Some(self.styles.button_active),
        )?;
        section.append_child(&pointer_btn)?;

        // Return both the section and important elements
        Ok((section, pointer_btn))
    }

    fn create_add_node_section(
        &self,
    ) -> Result<
        (
            HtmlElement,
            HtmlElement,
            HtmlElement,
            HtmlElement,
            Vec<HtmlElement>,
            Vec<HtmlElement>,
        ),
        JsValue,
    > {
        let section = create_section(self.document, None, Some(self.styles.section), Some("10px"))?;

        // Add Node button
        let add_node_btn = create_button(
            self.document,
            "➕ Add Node",
            Some("btn-add-node"),
            Some("toolbar-btn"),
            Some(self.styles.button),
        )?;
        section.append_child(&add_node_btn)?;

        // Cancel button (hidden initially)
        let cancel_btn = create_button(
            self.document,
            "Cancel",
            Some("btn-cancel"),
            None,
            Some(&format!("{}; display: none", self.styles.button)),
        )?;
        section.append_child(&cancel_btn)?;

        // Template group container with its buttons
        let (template_container, tab_buttons, template_buttons) =
            self.create_template_container()?;
        section.append_child(&template_container)?;

        // Return the section and all important elements
        Ok((
            section,
            add_node_btn,
            cancel_btn,
            template_container,
            tab_buttons,
            template_buttons,
        ))
    }

    fn create_template_container(
        &self,
    ) -> Result<(HtmlElement, Vec<HtmlElement>, Vec<HtmlElement>), JsValue> {
        let container: HtmlElement = create_element(
            self.document,
            "div",
            Some("template-group-container"),
            None,
            Some(self.styles.template_container),
        )?;

        // Create tab buttons container
        let tab_buttons: HtmlElement = create_element(
                self.document,
                "div",
                None,
                Some("template-group-tabs"),
                Some("display: flex; gap: 2px; margin-bottom: 8px; border-bottom: 1px solid #eee; padding-bottom: 4px;"),
            )?;

        let mut tab_button_elements = Vec::new();
        let mut template_button_elements = Vec::new();
        let mut content_containers = Vec::new();
        let mut first_template_name = None;

        // Create tabs and content for each template group
        for (i, (group_id, templates)) in self.template_groups.iter().enumerate() {
            // Skip empty groups
            if templates.is_empty() {
                continue;
            }

            // Create tab button
            let group_name = if group_id == "other" {
                "Other".to_string()
            } else if let Some(group) = self
                .graph_canvas
                .config
                .template_groups
                .iter()
                .find(|g| &g.id == group_id)
            {
                group.name.clone()
            } else {
                group_id.clone()
            };

            let tab_button = create_button(
                    self.document,
                    &group_name,
                    None,
                    Some(if i == 0 { "tab-button active" } else { "tab-button" }),
                    Some(&format!(
                        "padding: 4px 8px; border: none; background: {}; cursor: pointer; border-radius: 4px 4px 0 0;",
                        if i == 0 { "#f0f0f0" } else { "transparent" }
                    )),
                )?;
            tab_button.set_attribute("data-group-id", group_id)?;
            tab_buttons.append_child(&tab_button)?;
            tab_button_elements.push(tab_button);

            // Create content container
            let content_container: HtmlElement = create_element(
                self.document,
                "div",
                None,
                Some("template-group-content"),
                Some(&format!(
                    "display: {}; flex-direction: column; gap: 2px;",
                    if i == 0 { "flex" } else { "none" }
                )),
            )?;
            content_container.set_attribute("data-group-id", group_id)?;

            // Add template buttons
            for template in templates {
                let template_button = create_button(
                    self.document,
                    &template.name,
                    None,
                    Some("template-button"),
                    Some(self.styles.template_button),
                )?;
                template_button.set_attribute("data-template-name", &template.name)?;
                template_button.set_attribute("data-template-id", &template.template_id)?;
                content_container.append_child(&template_button)?;
                template_button_elements.push(template_button);

                // Remember first template
                if first_template_name.is_none() {
                    first_template_name = Some(template.name.clone());
                }
            }

            content_containers.push(content_container);
        }

        // Add tab buttons to container
        container.append_child(&tab_buttons)?;

        // Add content containers
        for content in content_containers {
            container.append_child(&content)?;
        }

        // Set first template as default if available
        if let Some(template_name) = first_template_name {
            self.graph_canvas.set_current_node_template(&template_name);
        }

        Ok((container, tab_button_elements, template_button_elements))
    }

    fn create_field_editor_section(&self) -> Result<(HtmlElement, Element), JsValue> {
        let section: HtmlElement = create_element(
            self.document,
            "div",
            Some("field-editor-section"),
            None,
            Some(self.styles.field_editor),
        )?;

        // Field editor title
        let title = create_label(self.document, "Node Fields", Some(self.styles.label))?;
        title.set_attribute("id", "field-editor-title")?;
        section.append_child(&title)?;

        // Field editor container
        let container: Element = create_element(
            self.document,
            "div",
            Some("field-editor-container"),
            None,
            Some("display: flex; flex-direction: column; gap: 6px;"),
        )?;
        section.append_child(&container)?;

        Ok((section, container))
    }

    fn create_layout_section(
        &self,
    ) -> Result<
        (
            HtmlElement,
            Vec<HtmlElement>,
            Vec<(HtmlElement, String)>,
            HtmlElement,
            HtmlInputElement,
        ),
        JsValue,
    > {
        let section = create_section(
            self.document,
            None,
            Some(self.styles.section),
            Some("auto"), // Push to right side
        )?;

        // Views label
        let label = create_label(self.document, "Views:", Some(self.styles.label))?;
        section.append_child(&label)?;

        // View tabs
        let view_tabs: HtmlElement = create_element(
            self.document,
            "div",
            None,
            None,
            Some("display: flex; margin: 0 10px;"),
        )?;

        // Create view buttons
        let view_names = ["View 1", "View 2", "View 3"];
        let mut view_buttons = Vec::new();

        for (i, name) in view_names.iter().enumerate() {
            let is_active = i == 0;
            let view_btn = create_button(
                    self.document,
                    name,
                    None,
                    Some(if is_active { "view-btn active" } else { "view-btn" }),
                    Some(&format!(
                        "padding: 4px 8px; border: 1px solid #ccc; margin: 0 2px; border-radius: 4px; background: {}; cursor: pointer;",
                        if is_active { "#e6f7ff" } else { "white" }
                    )),
                )?;
            view_btn.set_attribute("data-view-index", &i.to_string())?;
            view_tabs.append_child(&view_btn)?;
            view_buttons.push(view_btn);
        }
        section.append_child(&view_tabs)?;

        // Layout buttons
        let layout_buttons_container: HtmlElement = create_element(
            self.document,
            "div",
            None,
            None,
            Some("display: flex; margin-left: 10px;"),
        )?;

        // Create layout buttons
        let layouts = [
            (
                "Force",
                "force",
                "border-radius: 4px 0 0 4px; background: #e6f7ff",
            ),
            ("Hierarchical", "hierarchical", "border-left: none;"),
            (
                "Free",
                "free",
                "border-left: none; border-radius: 0 4px 4px 0;",
            ),
        ];

        let mut layout_buttons = Vec::new();

        for (i, (name, layout_type, extra_style)) in layouts.iter().enumerate() {
            let is_active = i == 0;
            let btn = create_button(
                self.document,
                name,
                None,
                Some(if is_active {
                    "view-btn active"
                } else {
                    "view-btn"
                }),
                Some(&format!("{}; {}", self.styles.button, extra_style)),
            )?;
            btn.set_attribute("data-layout", layout_type)?;
            layout_buttons_container.append_child(&btn)?;
            layout_buttons.push((btn, layout_type.to_string()));
        }
        section.append_child(&layout_buttons_container)?;

        // Physics toggle
        let physics_toggle: HtmlElement = create_element(
            self.document,
            "div",
            None,
            None,
            Some("display: flex; align-items: center; margin-left: 10px;"),
        )?;

        let physics_label = create_label(
            self.document,
            "Physics:",
            Some("font-size: 12px; margin-right: 4px;"),
        )?;
        physics_toggle.append_child(&physics_label)?;

        let physics_checkbox: HtmlInputElement =
            create_element(self.document, "input", Some("physics-toggle"), None, None)?;
        physics_checkbox.set_attribute("type", "checkbox")?;
        physics_checkbox.set_attribute("checked", "")?;
        physics_toggle.append_child(&physics_checkbox)?;
        section.append_child(&physics_toggle)?;

        // Reset layout button
        let reset_btn = create_button(
            self.document,
            "Reset View",
            None,
            None,
            Some(&format!("{}; margin-left: 10px", self.styles.button)),
        )?;
        section.append_child(&reset_btn)?;

        Ok((
            section,
            view_buttons,
            layout_buttons,
            reset_btn,
            physics_checkbox,
        ))
    }
}

// --- Event handler attachment ---
pub struct ToolbarEventHandler<'a> {
    elements: &'a ToolbarElements,
    document: &'a Document,
    graph_canvas: &'a GraphCanvas,
    canvas: web_sys::HtmlCanvasElement,
}

impl<'a> ToolbarEventHandler<'a> {
    pub fn new(
        elements: &'a ToolbarElements,
        document: &'a Document,
        graph_canvas: &'a GraphCanvas,
    ) -> Result<Self, JsValue> {
        // Get canvas reference
        let canvas = window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(&graph_canvas.canvas_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        Ok(Self {
            elements,
            document,
            graph_canvas,
            canvas,
        })
    }

    pub fn attach_all_handlers(&self) -> Result<(), JsValue> {
        self.attach_mode_handlers()?;
        self.attach_add_node_handlers()?;
        self.attach_template_handlers()?;
        self.attach_view_handlers()?;
        self.attach_layout_handlers()?;
        self.attach_node_selection_handler()?;

        Ok(())
    }

    fn attach_mode_handlers(&self) -> Result<(), JsValue> {
        let pointer_btn = &self.elements.pointer_btn;
        let template_container = &self.elements.template_group_container;
        let cancel_btn = &self.elements.cancel_btn;

        let graph_canvas_clone = self.graph_canvas.clone();
        let pointer_btn_clone = pointer_btn.clone();
        let template_container_clone = template_container.clone();
        let cancel_btn_clone = cancel_btn.clone();

        // Pointer button click handler
        let pointer_click = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
            // Switch to default mode
            graph_canvas_clone.set_interaction_mode(InteractionMode::Default);

            // Update button styles
            pointer_btn_clone
                .set_attribute("class", "toolbar-btn active")
                .unwrap();

            // Hide add node UI
            template_container_clone
                .style()
                .set_property("display", "none")
                .unwrap();
            cancel_btn_clone
                .style()
                .set_property("display", "none")
                .unwrap();
        }) as Box<dyn FnMut(_)>);

        pointer_btn
            .add_event_listener_with_callback("click", pointer_click.as_ref().unchecked_ref())?;
        pointer_click.forget();

        Ok(())
    }

    fn attach_add_node_handlers(&self) -> Result<(), JsValue> {
        let add_node_btn = &self.elements.add_node_btn;
        let pointer_btn = &self.elements.pointer_btn;
        let template_container = &self.elements.template_group_container;
        let cancel_btn = &self.elements.cancel_btn;

        // Add node button click handler
        let graph_canvas_clone = self.graph_canvas.clone();
        let pointer_btn_clone = pointer_btn.clone();
        let template_container_clone = template_container.clone();
        let cancel_btn_clone = cancel_btn.clone();

        let add_node_click = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
            // Switch to add node mode
            graph_canvas_clone.set_interaction_mode(InteractionMode::AddNode);

            // Update button styles
            pointer_btn_clone
                .set_attribute("class", "toolbar-btn")
                .unwrap();

            // Show template selection UI
            template_container_clone
                .style()
                .set_property("display", "block")
                .unwrap();
            cancel_btn_clone
                .style()
                .set_property("display", "inline-block")
                .unwrap();
        }) as Box<dyn FnMut(_)>);

        add_node_btn
            .add_event_listener_with_callback("click", add_node_click.as_ref().unchecked_ref())?;
        add_node_click.forget();

        // Cancel button click handler
        let graph_canvas_clone = self.graph_canvas.clone();
        let pointer_btn_clone = pointer_btn.clone();
        let template_container_clone = template_container.clone();
        let cancel_btn_clone = cancel_btn.clone();

        let cancel_click = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
            // Switch back to default mode
            graph_canvas_clone.set_interaction_mode(InteractionMode::Default);

            // Update button styles
            pointer_btn_clone
                .set_attribute("class", "toolbar-btn active")
                .unwrap();

            // Hide template UI
            template_container_clone
                .style()
                .set_property("display", "none")
                .unwrap();
            cancel_btn_clone
                .style()
                .set_property("display", "none")
                .unwrap();
        }) as Box<dyn FnMut(_)>);

        cancel_btn
            .add_event_listener_with_callback("click", cancel_click.as_ref().unchecked_ref())?;
        cancel_click.forget();

        Ok(())
    }

    fn attach_template_handlers(&self) -> Result<(), JsValue> {
        // Handle tab buttons
        let template_container = &self.elements.template_group_container;

        for tab_button in &self.elements.tab_buttons {
            let tab_button_clone = tab_button.clone();
            let template_container_clone = template_container.clone();
            let tab_buttons_clone = self.elements.tab_buttons.clone();

            let tab_click = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
                let group_id = tab_button_clone.get_attribute("data-group-id").unwrap();

                // Update active tab
                for button in &tab_buttons_clone {
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

                // Show corresponding content
                let content_containers = template_container_clone
                    .query_selector_all(".template-group-content")
                    .unwrap();

                for j in 0..content_containers.length() {
                    let container = content_containers
                        .get(j)
                        .unwrap()
                        .dyn_into::<HtmlElement>()
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

        // Handle template buttons
        for template_button in &self.elements.template_buttons {
            let template_button_clone = template_button.clone();
            let graph_canvas_clone = self.graph_canvas.clone();
            let template_buttons_clone = self.elements.template_buttons.clone();

            let template_click = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
                // Update button styles
                for button in &template_buttons_clone {
                    button
                        .style()
                        .set_property("background-color", "white")
                        .unwrap();
                    button
                        .style()
                        .set_property("border", "1px solid #ddd")
                        .unwrap();
                }

                template_button_clone
                    .style()
                    .set_property("background-color", "lightgoldenrodyellow")
                    .unwrap();
                template_button_clone
                    .style()
                    .set_property("border", "1px solid black")
                    .unwrap();

                // Set selected template
                let template_id = template_button_clone
                    .get_attribute("data-template-id")
                    .unwrap();
                graph_canvas_clone.set_current_node_template(&template_id);
            }) as Box<dyn FnMut(_)>);

            template_button.add_event_listener_with_callback(
                "click",
                template_click.as_ref().unchecked_ref(),
            )?;
            template_click.forget();
        }

        Ok(())
    }

    fn attach_view_handlers(&self) -> Result<(), JsValue> {
        // View button handlers
        for (i, view_btn) in self.elements.view_buttons.iter().enumerate() {
            let graph_canvas_clone = self.graph_canvas.clone();
            let view_buttons_clone = self.elements.view_buttons.clone();
            let physics_checkbox_clone = self.elements.physics_checkbox.clone();

            let on_view_change = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
                let view_index = i;

                // Update button styles
                for (j, btn) in view_buttons_clone.iter().enumerate() {
                    if j == view_index {
                        btn.set_attribute("class", "view-btn active").unwrap();
                        btn.style().set_property("background", "#e6f7ff").unwrap();
                    } else {
                        btn.set_attribute("class", "view-btn").unwrap();
                        btn.style().set_property("background", "white").unwrap();
                    }
                }

                // Switch to selected view
                let mut layout_engine = graph_canvas_clone.layout_engine.lock().unwrap();
                let mut graph = graph_canvas_clone.graph.lock().unwrap();
                let mut ix = graph_canvas_clone.interaction.lock().unwrap();

                layout_engine.switch_to_view(view_index, &mut graph, &mut ix);

                // Update physics checkbox
                let physics_enabled = layout_engine.is_physics_enabled();
                physics_checkbox_clone.set_checked(physics_enabled);
            }) as Box<dyn FnMut(_)>);

            view_btn.add_event_listener_with_callback(
                "click",
                on_view_change.as_ref().unchecked_ref(),
            )?;
            on_view_change.forget();
        }

        // Physics toggle handler
        let graph_canvas_clone = self.graph_canvas.clone();
        let physics_checkbox = &self.elements.physics_checkbox;

        let on_physics_toggle = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let checkbox = event
                .target()
                .unwrap()
                .dyn_into::<HtmlInputElement>()
                .unwrap();
            let checked = checkbox.checked();

            let mut layout_engine = graph_canvas_clone.layout_engine.lock().unwrap();
            let enabled = layout_engine.toggle_physics();

            // Make sure checkbox matches state
            if enabled != checked {
                checkbox.set_checked(enabled);
            }
        }) as Box<dyn FnMut(_)>);

        physics_checkbox.add_event_listener_with_callback(
            "change",
            on_physics_toggle.as_ref().unchecked_ref(),
        )?;
        on_physics_toggle.forget();

        Ok(())
    }

    fn attach_layout_handlers(&self) -> Result<(), JsValue> {
        // Layout button handlers
        for (i, (btn, layout_value)) in self.elements.layout_buttons.iter().enumerate() {
            let graph_canvas_clone = self.graph_canvas.clone();
            let layout_value_clone = layout_value.clone();
            let layout_buttons_clone = self.elements.layout_buttons.clone();

            let on_layout_change = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
                let view_index = i;
                for (j, (btn, _layout_value)) in layout_buttons_clone.iter().enumerate() {
                    if j == view_index {
                        btn.set_attribute("class", "layout-btn active").unwrap();
                        btn.style().set_property("background", "#e6f7ff").unwrap();
                    } else {
                        btn.set_attribute("class", "layout-btn").unwrap();
                        btn.style().set_property("background", "white").unwrap();
                    }
                }
                let layout_type = match layout_value_clone.as_str() {
                    "hierarchical" => LayoutType::Hierarchical,
                    "force" => LayoutType::ForceDirected,
                    _ => LayoutType::Free,
                };

                let mut layout_engine = graph_canvas_clone.layout_engine.lock().unwrap();
                let mut graph = graph_canvas_clone.graph.lock().unwrap();
                layout_engine.switch_layout(layout_type.clone(), &mut graph);
            }) as Box<dyn FnMut(_)>);

            btn.add_event_listener_with_callback(
                "click",
                on_layout_change.as_ref().unchecked_ref(),
            )?;
            on_layout_change.forget();
        }

        // Reset button handler
        let reset_btn = &self.elements.reset_btn;
        let graph_canvas_clone = self.graph_canvas.clone();

        let on_reset = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
            let mut layout_engine = graph_canvas_clone.layout_engine.lock().unwrap();
            let mut graph = graph_canvas_clone.graph.lock().unwrap();
            let mut ix = graph_canvas_clone.interaction.lock().unwrap();
            layout_engine.reset_current_layout(&mut graph, &mut ix);
        }) as Box<dyn FnMut(_)>);

        reset_btn.add_event_listener_with_callback("click", on_reset.as_ref().unchecked_ref())?;
        on_reset.forget();

        Ok(())
    }

    fn attach_node_selection_handler(&self) -> Result<(), JsValue> {
        let field_editor_section = &self.elements.field_editor_section;
        let field_editor_container = &self.elements.field_editor_container;

        let graph_canvas_clone = self.graph_canvas.clone();
        let document_clone = self.document.clone();
        let field_editor_section_clone = field_editor_section.clone();
        let field_editor_container_clone = field_editor_container.clone();

        let node_selection_handler = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
            let graph = graph_canvas_clone.graph.lock().unwrap();
            let interaction = graph_canvas_clone.interaction.lock().unwrap();

            // Check if a node is selected
            if let Some(selected_node_id) = &interaction.click_initiated_on_node {
                if let Some(node_instance) = graph.node_instances.get(selected_node_id) {
                    if !node_instance.fields.is_empty() {
                        // Get node template
                        if let Some(node_template) =
                            graph.node_templates.get(&node_instance.template_id)
                        {
                            // Show field editor
                            field_editor_section_clone
                                .style()
                                .set_property("display", "flex")
                                .unwrap();

                            // Set title
                            let title_elem = document_clone
                                .get_element_by_id("field-editor-title")
                                .unwrap();
                            title_elem.set_inner_html(&format!("{} Fields:", node_template.name));

                            // Clear existing fields
                            field_editor_container_clone.set_inner_html("");

                            // Create field editor UI
                            for field_instance in &node_instance.fields {
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

                                    // Create input based on field type
                                    match field_template.field_type {
                                        crate::FieldType::Boolean => {
                                            Self::create_boolean_field(
                                                &document_clone,
                                                &field_container,
                                                field_instance,
                                                selected_node_id,
                                                &graph_canvas_clone,
                                            );
                                        }
                                        crate::FieldType::Integer => {
                                            Self::create_integer_field(
                                                &document_clone,
                                                &field_container,
                                                field_instance,
                                                selected_node_id,
                                                &graph_canvas_clone,
                                            );
                                        }
                                        crate::FieldType::String => {
                                            Self::create_string_field(
                                                &document_clone,
                                                &field_container,
                                                field_instance,
                                                selected_node_id,
                                                &graph_canvas_clone,
                                            );
                                        }
                                    }

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

        self.canvas.add_event_listener_with_callback(
            "mouseup",
            node_selection_handler.as_ref().unchecked_ref(),
        )?;
        node_selection_handler.forget();

        Ok(())
    }

    // Field creation helper methods
    fn create_boolean_field(
        document: &Document,
        container: &Element,
        field_instance: &crate::graph::FieldInstance,
        node_id: &str,
        graph_canvas: &GraphCanvas,
    ) {
        let checkbox = document.create_element("input").unwrap();
        checkbox.set_attribute("type", "checkbox").unwrap();
        checkbox
            .set_attribute("data-field-id", &field_instance.field_template_id)
            .unwrap();
        checkbox.set_attribute("data-node-id", node_id).unwrap();

        if field_instance.value == "true" {
            checkbox
                .dyn_ref::<HtmlInputElement>()
                .unwrap()
                .set_checked(true);
        }

        // Add change event
        let graph_canvas_clone = graph_canvas.clone();
        let field_id = field_instance.field_template_id.clone();
        let node_id = node_id.to_string();

        let change_callback = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let checked = event
                .target()
                .unwrap()
                .dyn_into::<HtmlInputElement>()
                .unwrap()
                .checked();

            let mut graph = graph_canvas_clone.graph.lock().unwrap();
            let events = graph_canvas_clone.events.lock().unwrap();

            // Update field value
            graph
                .execute_command(
                    crate::graph::GraphCommand::UpdateField {
                        node_id: node_id.clone(),
                        field_template_id: field_id.clone(),
                        new_value: if checked {
                            "true".to_string()
                        } else {
                            "false".to_string()
                        },
                    },
                    &events,
                )
                .unwrap_or_else(|_| log("Failed to update boolean field"));
        }) as Box<dyn FnMut(_)>);

        checkbox
            .add_event_listener_with_callback("change", change_callback.as_ref().unchecked_ref())
            .unwrap();
        change_callback.forget();

        container.append_child(&checkbox).unwrap();
    }

    fn create_integer_field(
        document: &Document,
        container: &Element,
        field_instance: &crate::graph::FieldInstance,
        node_id: &str,
        graph_canvas: &GraphCanvas,
    ) {
        let number_input = document.create_element("input").unwrap();
        number_input.set_attribute("type", "number").unwrap();
        number_input
            .set_attribute("value", &field_instance.value)
            .unwrap();
        number_input
            .set_attribute("data-field-id", &field_instance.field_template_id)
            .unwrap();
        number_input.set_attribute("data-node-id", node_id).unwrap();
        number_input.set_attribute("style", "width: 60px;").unwrap();

        // Add change event
        let graph_canvas_clone = graph_canvas.clone();
        let field_id = field_instance.field_template_id.clone();
        let node_id = node_id.to_string();

        let change_callback = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let value = event
                .target()
                .unwrap()
                .dyn_into::<HtmlInputElement>()
                .unwrap()
                .value();

            let mut graph = graph_canvas_clone.graph.lock().unwrap();
            let events = graph_canvas_clone.events.lock().unwrap();

            // Update field value
            graph
                .execute_command(
                    crate::graph::GraphCommand::UpdateField {
                        node_id: node_id.clone(),
                        field_template_id: field_id.clone(),
                        new_value: value,
                    },
                    &events,
                )
                .unwrap_or_else(|_| log("Failed to update integer field"));
        }) as Box<dyn FnMut(_)>);

        number_input
            .add_event_listener_with_callback("change", change_callback.as_ref().unchecked_ref())
            .unwrap();
        change_callback.forget();

        container.append_child(&number_input).unwrap();
    }

    fn create_string_field(
        document: &Document,
        container: &Element,
        field_instance: &crate::graph::FieldInstance,
        node_id: &str,
        graph_canvas: &GraphCanvas,
    ) {
        let text_input = document.create_element("input").unwrap();
        text_input.set_attribute("type", "text").unwrap();
        text_input
            .set_attribute("value", &field_instance.value)
            .unwrap();
        text_input
            .set_attribute("data-field-id", &field_instance.field_template_id)
            .unwrap();
        text_input.set_attribute("data-node-id", node_id).unwrap();
        text_input.set_attribute("style", "width: 120px;").unwrap();

        // Add change event
        let graph_canvas_clone = graph_canvas.clone();
        let field_id = field_instance.field_template_id.clone();
        let node_id = node_id.to_string();

        let change_callback = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let value = event
                .target()
                .unwrap()
                .dyn_into::<HtmlInputElement>()
                .unwrap()
                .value();

            let mut graph = graph_canvas_clone.graph.lock().unwrap();
            let events = graph_canvas_clone.events.lock().unwrap();

            // Update field value
            graph
                .execute_command(
                    crate::graph::GraphCommand::UpdateField {
                        node_id: node_id.clone(),
                        field_template_id: field_id.clone(),
                        new_value: value,
                    },
                    &events,
                )
                .unwrap_or_else(|_| log("Failed to update string field"));
        }) as Box<dyn FnMut(_)>);

        text_input
            .add_event_listener_with_callback("change", change_callback.as_ref().unchecked_ref())
            .unwrap();
        change_callback.forget();

        container.append_child(&text_input).unwrap();
    }
}
