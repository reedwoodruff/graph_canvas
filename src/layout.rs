use std::collections::{HashMap, HashSet, VecDeque};
use wasm_bindgen::JsCast;

use web_sys::window;

use crate::{graph::Graph, interaction::InteractionState};

#[derive(Clone, Debug)]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug)]
pub struct LayoutSnapshot {
    pub positions: HashMap<String, NodePosition>, // node_id -> position
}

#[cfg_attr(
    feature = "js",
    derive(serde::Serialize, serde::Deserialize, tsify::Tsify)
)]
#[cfg_attr(feature = "js", tsify(into_wasm_abi, from_wasm_abi))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LayoutType {
    Free,
    Hierarchical,
    ForceDirected,
}

// A struct to represent each view's state
#[derive(Clone, Debug)]
pub struct ViewState {
    pub layout_type: LayoutType,
    pub snapshot: LayoutSnapshot,
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
    pub physics_enabled: bool,
}

impl ViewState {
    pub fn new(layout_type: LayoutType, snapshot: LayoutSnapshot) -> Self {
        Self {
            layout_type,
            snapshot,
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            physics_enabled: true,
        }
    }
}

pub struct LayoutEngine {
    current_view_index: usize, // Which view is currently active (0, 1, or 2)
    views: Vec<ViewState>,     // The three different views
    canvas_id: String,
    // Force simulation state
    force_simulation_active: bool,
    fixed_node_id: Option<String>,
    simulation_iteration: usize,
    temperature: f64,
    connections: HashMap<String, Vec<String>>,
}

impl LayoutEngine {
    pub fn new(canvas_ref_id: String) -> Self {
        // Create initial empty views - snapshots will be generated on first use
        let empty_snapshot = LayoutSnapshot {
            positions: HashMap::new(),
        };

        // Create three views with different default layouts
        let views = vec![
            ViewState::new(LayoutType::ForceDirected, empty_snapshot.clone()),
            ViewState::new(LayoutType::Hierarchical, empty_snapshot.clone()),
            ViewState::new(LayoutType::Free, empty_snapshot.clone()),
        ];

        Self {
            current_view_index: 0, // Start with view 1 (ForceDirected)
            views,
            canvas_id: canvas_ref_id,
            force_simulation_active: false,
            fixed_node_id: None,
            simulation_iteration: 0,
            temperature: 0.0,
            connections: HashMap::new(),
        }
    }

    pub fn switch_layout(&mut self, layout_type: LayoutType, graph: &mut Graph) {
        // Save current state of current view (keeping the current view index)
        self.save_current_view_state(graph);

        // Update current view to use the new layout type

        // Generate appropriate layout snapshot if the view doesn't have one yet or it's Force layout
        // Always regenerate Force layout when switching to it to ensure it's applied
        let needs_layout = self.views[self.current_view_index]
            .snapshot
            .positions
            .is_empty();

        let snapshot = if needs_layout {
            let new_snapshot = match layout_type {
                LayoutType::Free => self.generate_free_layout(graph),
                LayoutType::Hierarchical => self.generate_hierarchical_layout(graph),
                LayoutType::ForceDirected => self.generate_force_directed_layout(graph),
            };
            new_snapshot
        } else {
            self.views[self.current_view_index].snapshot.clone()
        };
        self.views[self.current_view_index].layout_type = layout_type.clone();

        // Apply the layout snapshot from the current view
        self.apply_snapshot(graph, &snapshot);

        // Set physics based on layout type
        self.views[self.current_view_index].physics_enabled =
            layout_type == LayoutType::ForceDirected;
    }

    // Method to switch to a specific view by index (0, 1, or 2)
    pub fn switch_to_view(
        &mut self,
        view_index: usize,
        graph: &mut Graph,
        ix: &mut InteractionState,
    ) {
        if view_index >= self.views.len() {
            return; // Invalid view index
        }

        // Save current view state before switching
        self.save_current_view_state(graph);

        // Switch to new view
        self.current_view_index = view_index;
        let current_view = &self.views[self.current_view_index];

        // Apply the view's state
        self.apply_snapshot(graph, &current_view.snapshot);

        // Set interaction state from view
        ix.view_transform.pan_x = current_view.pan_x;
        ix.view_transform.pan_y = current_view.pan_y;
        ix.view_transform.zoom = current_view.zoom;
    }

    // Check if physics is enabled for the current view
    pub fn is_physics_enabled(&self) -> bool {
        self.views[self.current_view_index].physics_enabled
    }

    // Toggle physics for the current view
    pub fn toggle_physics(&mut self) -> bool {
        let current_view = &mut self.views[self.current_view_index];
        current_view.physics_enabled = !current_view.physics_enabled;
        current_view.physics_enabled
    }

    pub fn reset_current_layout(&mut self, graph: &mut Graph, ix: &mut InteractionState) {
        // Regenerate the current layout snapshot based on the current view's layout type
        let layout_type = self.views[self.current_view_index].layout_type.clone();
        let new_snapshot = match layout_type {
            LayoutType::Free => self.generate_free_layout(graph),
            LayoutType::Hierarchical => self.generate_hierarchical_layout(graph),
            LayoutType::ForceDirected => self.generate_force_directed_layout(graph),
        };

        let current_view = &mut self.views[self.current_view_index];

        // Update the current view's snapshot
        current_view.snapshot = new_snapshot.clone();

        // Reset pan and zoom
        current_view.pan_x = 0.0;
        current_view.pan_y = 0.0;
        current_view.zoom = 1.0;

        // Reset interaction state
        ix.view_transform.pan_x = 0.0;
        ix.view_transform.pan_y = 0.0;
        ix.view_transform.zoom = 1.0;

        // Apply the new snapshot
        self.apply_snapshot(graph, &new_snapshot);
    }

    fn save_current_view_state(&mut self, graph: &Graph) {
        // Create a snapshot of current node positions
        let mut positions = HashMap::new();
        for (id, instance) in &graph.node_instances {
            positions.insert(
                id.clone(),
                NodePosition {
                    x: instance.x,
                    y: instance.y,
                },
            );
        }

        // Save the snapshot to the current view
        self.views[self.current_view_index].snapshot = LayoutSnapshot { positions };
    }

    fn apply_snapshot(&self, graph: &mut Graph, snapshot: &LayoutSnapshot) {
        for (id, pos) in &snapshot.positions {
            if let Some(instance) = graph.node_instances.get_mut(id) {
                instance.x = pos.x;
                instance.y = pos.y;
            }
        }
    }

    fn generate_hierarchical_layout(&self, graph: &Graph) -> LayoutSnapshot {
        let mut positions = HashMap::new();

        // STEP 1: Create dependency graphs
        let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
        let mut reverse_dependencies: HashMap<String, Vec<String>> = HashMap::new();

        // Initialize with empty vectors
        for id in graph.node_instances.keys() {
            dependencies.insert(id.clone(), Vec::new());
            reverse_dependencies.insert(id.clone(), Vec::new());
        }

        // Build dependency graph from connections
        for (id, instance) in &graph.node_instances {
            for slot in &instance.slots {
                for conn in &slot.connections {
                    dependencies
                        .get_mut(id)
                        .unwrap()
                        .push(conn.target_node_id.clone());
                    reverse_dependencies
                        .get_mut(&conn.target_node_id)
                        .unwrap()
                        .push(id.clone());
                }
            }
        }

        // STEP 2: Cycle detection and breaking
        let mut temp_dependencies = dependencies.clone();
        let mut removed_edges = Vec::new();

        // Simple greedy cycle breaking
        self.break_cycles(&mut temp_dependencies, &mut removed_edges);

        // STEP 3: Improved level assignment using longest path method
        let levels = self.assign_levels(&temp_dependencies, &reverse_dependencies);

        // STEP 4: Group nodes by level
        let mut nodes_by_level: HashMap<usize, Vec<String>> = HashMap::new();
        for (node_id, level) in &levels {
            nodes_by_level
                .entry(*level)
                .or_insert_with(Vec::new)
                .push(node_id.clone());
        }

        // Convert level map to a sorted vector of levels for easier iteration
        let mut level_keys: Vec<usize> = nodes_by_level.keys().cloned().collect();
        level_keys.sort();

        // STEP 5: Minimize edge crossings using barycenter method
        self.minimize_edge_crossings_barycenter(
            &mut nodes_by_level,
            &temp_dependencies,
            &level_keys,
        );

        // STEP 6: Optimize horizontal positions for better distribution
        self.optimize_horizontal_positions(&mut nodes_by_level, &temp_dependencies, &level_keys);

        // Get canvas dimensions for layout scaling and centering
        let canvas = window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(&self.canvas_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();

        let canvas_width = canvas.get_bounding_client_rect().width();
        let canvas_height = canvas.get_bounding_client_rect().height();

        // Calculate bounds of the current layout
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        for (level, x_positions) in &nodes_by_level {
            for (idx, _node_id) in x_positions.iter().enumerate() {
                let x = *level as f64;
                let y = idx as f64;

                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }

        // Scale and center the layout
        let width = (max_x - min_x).max(1.0);
        let height = (max_y - min_y).max(1.0);

        let scale_x = (canvas_width * 0.8) / width;
        let scale_y = (canvas_height * 0.8) / height;
        let scale = scale_x.min(scale_y);

        let x_offset = canvas_width * 0.1 - min_x * scale;
        let y_offset = canvas_height * 0.1 - min_y * scale;

        // Apply final positions
        let level_spacing = 250.0;
        let node_spacing = 150.0;

        for (level, nodes) in &nodes_by_level {
            for (idx, node_id) in nodes.iter().enumerate() {
                positions.insert(
                    node_id.clone(),
                    NodePosition {
                        x: x_offset + *level as f64 * level_spacing,
                        y: y_offset + idx as f64 * node_spacing,
                    },
                );
            }
        }

        LayoutSnapshot { positions }
    }

    // Cycle detection and breaking using a greedy approach
    fn break_cycles(
        &self,
        dependencies: &mut HashMap<String, Vec<String>>,
        removed_edges: &mut Vec<(String, String)>,
    ) {
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        let node_ids: Vec<String> = dependencies.keys().cloned().collect();

        for node_id in &node_ids {
            if !visited.contains(node_id) {
                self.dfs_cycle_detection(
                    node_id,
                    dependencies,
                    &mut visited,
                    &mut stack,
                    removed_edges,
                );
            }
        }
    }

    fn dfs_cycle_detection(
        &self,
        node_id: &str,
        dependencies: &mut HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
        removed_edges: &mut Vec<(String, String)>,
    ) {
        visited.insert(node_id.to_string());
        stack.insert(node_id.to_string());

        let targets = dependencies.get(node_id).unwrap().clone();
        for target in targets {
            if !visited.contains(&target) {
                self.dfs_cycle_detection(&target, dependencies, visited, stack, removed_edges);
            } else if stack.contains(&target) {
                // Cycle detected - remove this edge
                let targets = dependencies.get_mut(node_id).unwrap();
                if let Some(pos) = targets.iter().position(|x| x == &target) {
                    targets.remove(pos);
                    removed_edges.push((node_id.to_string(), target));
                }
            }
        }

        stack.remove(node_id);
    }

    // Improved level assignment using longest path method
    fn assign_levels(
        &self,
        dependencies: &HashMap<String, Vec<String>>,
        reverse_dependencies: &HashMap<String, Vec<String>>,
    ) -> HashMap<String, usize> {
        let mut levels: HashMap<String, usize> = HashMap::new();

        // Find source nodes (no incoming edges)
        let mut source_nodes: Vec<String> = Vec::new();
        for (node_id, incoming) in reverse_dependencies {
            if incoming.is_empty() {
                source_nodes.push(node_id.clone());
            }
        }

        // If no source nodes found, use nodes with minimum incoming edges
        if source_nodes.is_empty() {
            let mut min_incoming = usize::MAX;
            for (node_id, incoming) in reverse_dependencies {
                if incoming.len() < min_incoming {
                    min_incoming = incoming.len();
                    source_nodes = vec![node_id.clone()];
                } else if incoming.len() == min_incoming {
                    source_nodes.push(node_id.clone());
                }
            }
        }

        // Assign level 0 to source nodes
        for node_id in &source_nodes {
            levels.insert(node_id.clone(), 0);
        }

        // Queue for topological traversal
        let mut queue: VecDeque<String> = source_nodes.into_iter().collect();

        // Process nodes in topological order
        while let Some(node_id) = queue.pop_front() {
            let current_level = *levels.get(&node_id).unwrap();

            for target in &dependencies[&node_id] {
                let target_level = levels.entry(target.clone()).or_insert(0);
                *target_level = (*target_level).max(current_level + 1);

                // Add target to queue if all its dependencies have been processed
                let all_deps_processed = reverse_dependencies[target]
                    .iter()
                    .all(|dep| levels.contains_key(dep));

                if all_deps_processed && !queue.contains(target) {
                    queue.push_back(target.clone());
                }
            }
        }

        // Assign level 0 to any remaining nodes (happens in disconnected graphs)
        for node_id in dependencies.keys() {
            levels.entry(node_id.clone()).or_insert(0);
        }

        levels
    }

    // Minimize edge crossings using the barycenter method
    fn minimize_edge_crossings_barycenter(
        &self,
        nodes_by_level: &mut HashMap<usize, Vec<String>>,
        dependencies: &HashMap<String, Vec<String>>,
        level_keys: &[usize],
    ) {
        // Number of iterations for the algorithm
        let iterations = 4;

        for _ in 0..iterations {
            // Forward sweep (top to bottom)
            for i in 1..level_keys.len() {
                let current_level = level_keys[i];
                let previous_level = level_keys[i - 1];

                // Current level nodes
                let current_nodes = nodes_by_level.get(&current_level).unwrap().clone();

                // Calculate barycenter values for each node
                let mut node_values: Vec<(String, f64)> = Vec::new();

                for node_id in current_nodes {
                    // Find all nodes in the previous level that connect to this node
                    let mut connected_indices: Vec<usize> = Vec::new();
                    let previous_nodes = nodes_by_level.get(&previous_level).unwrap();

                    for (idx, prev_node) in previous_nodes.iter().enumerate() {
                        if dependencies[prev_node].contains(&node_id) {
                            connected_indices.push(idx);
                        }
                    }

                    // Calculate barycenter value
                    let barycenter = if connected_indices.is_empty() {
                        // If no connections, keep original position
                        node_values.len() as f64
                    } else {
                        // Average of connected nodes' positions
                        connected_indices.iter().map(|&idx| idx as f64).sum::<f64>()
                            / connected_indices.len() as f64
                    };

                    node_values.push((node_id, barycenter));
                }

                // Sort nodes by barycenter value
                node_values
                    .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

                // Update level ordering
                nodes_by_level.insert(
                    current_level,
                    node_values.iter().map(|(id, _)| id.clone()).collect(),
                );
            }

            // Backward sweep (bottom to top)
            for i in (1..level_keys.len()).rev() {
                let current_level = level_keys[i - 1];
                let next_level = level_keys[i];

                // Current level nodes
                let current_nodes = nodes_by_level.get(&current_level).unwrap().clone();

                // Calculate barycenter values based on next level
                let mut node_values: Vec<(String, f64)> = Vec::new();

                for node_id in current_nodes {
                    // Find all nodes in the next level that this node connects to
                    let mut connected_indices: Vec<usize> = Vec::new();
                    let next_nodes = nodes_by_level.get(&next_level).unwrap();

                    for (idx, next_node) in next_nodes.iter().enumerate() {
                        if dependencies[&node_id].contains(next_node) {
                            connected_indices.push(idx);
                        }
                    }

                    // Calculate barycenter value
                    let barycenter = if connected_indices.is_empty() {
                        // If no connections, keep original position
                        node_values.len() as f64
                    } else {
                        // Average of connected nodes' positions
                        connected_indices.iter().map(|&idx| idx as f64).sum::<f64>()
                            / connected_indices.len() as f64
                    };

                    node_values.push((node_id, barycenter));
                }

                // Sort nodes by barycenter value
                node_values
                    .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

                // Update level ordering
                nodes_by_level.insert(
                    current_level,
                    node_values.iter().map(|(id, _)| id.clone()).collect(),
                );
            }
        }
    }

    // Optimize horizontal positions for better spacing
    fn optimize_horizontal_positions(
        &self,
        nodes_by_level: &mut HashMap<usize, Vec<String>>,
        dependencies: &HashMap<String, Vec<String>>,
        level_keys: &[usize],
    ) {
        // Calculate ideal horizontal positions to minimize edge lengths
        let mut node_x_positions: HashMap<String, f64> = HashMap::new();

        // Assign initial x positions based on level ordering
        for level in level_keys {
            let nodes = nodes_by_level.get(level).unwrap();
            for (idx, node_id) in nodes.iter().enumerate() {
                node_x_positions.insert(node_id.clone(), idx as f64);
            }
        }

        // Adjust positions to minimize edge lengths
        let iterations = 3;
        let weight = 0.5; // Weight for position adjustments

        for _ in 0..iterations {
            // For each node, adjust position based on connected nodes
            for level in level_keys {
                let nodes = nodes_by_level.get(level).unwrap();

                for node_id in nodes {
                    let mut connected_nodes = Vec::new();

                    // Add outgoing connections
                    for target in &dependencies[node_id] {
                        connected_nodes.push(target);
                    }

                    // Add incoming connections
                    for (source, targets) in dependencies {
                        if targets.contains(node_id) {
                            connected_nodes.push(source);
                        }
                    }

                    if !connected_nodes.is_empty() {
                        // Calculate average position of connected nodes
                        let avg_pos = connected_nodes
                            .iter()
                            .filter_map(|id| node_x_positions.get(*id))
                            .sum::<f64>()
                            / connected_nodes.len() as f64;

                        // Adjust position (weighted average)
                        let current_pos = *node_x_positions.get(node_id).unwrap();
                        let new_pos = current_pos * (1.0 - weight) + avg_pos * weight;
                        node_x_positions.insert(node_id.clone(), new_pos);
                    }
                }
            }

            // Reorder nodes within each level based on new x positions
            for level in level_keys {
                let mut nodes = nodes_by_level.get(level).unwrap().clone();
                nodes.sort_by(|a, b| {
                    let a_pos = node_x_positions.get(a).unwrap();
                    let b_pos = node_x_positions.get(b).unwrap();
                    a_pos
                        .partial_cmp(b_pos)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                nodes_by_level.insert(*level, nodes);
            }
        }
    }
    fn generate_free_layout(&self, graph: &Graph) -> LayoutSnapshot {
        // For free layout, we can either:
        // 1. Use current positions
        // 2. Generate a simple grid layout
        // 3. Use a force-directed layout
        // For now, let's use current positions
        let mut positions = HashMap::new();
        for (id, instance) in &graph.node_instances {
            positions.insert(
                id.clone(),
                NodePosition {
                    x: instance.x,
                    y: instance.y,
                },
            );
        }
        LayoutSnapshot { positions }
    }
    // Start force simulation when dragging a node
    pub fn start_force_simulation(&mut self, graph: &Graph, node_id: &str) {
        // Only activate if physics is enabled for the current view
        if !self.views[self.current_view_index].physics_enabled {
            return;
        }

        // Get canvas dimensions for simulation bounds
        let canvas = window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(&self.canvas_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();

        let canvas_width = canvas.get_bounding_client_rect().width();

        // Initialize simulation parameters
        self.force_simulation_active = true;
        self.fixed_node_id = Some(node_id.to_string());
        self.simulation_iteration = 0;
        self.temperature = canvas_width * 0.3; // Initial temperature - smaller than full sim for more control

        // Build connection graph
        self.build_connection_graph(graph);
    }

    // Stop force simulation
    pub fn stop_force_simulation(&mut self) {
        self.force_simulation_active = false;
        self.fixed_node_id = None;
    }

    // Save view transform state from interaction to current view
    pub fn save_view_transform(&mut self, ix: &InteractionState) {
        let current_view = &mut self.views[self.current_view_index];
        current_view.pan_x = ix.view_transform.pan_x;
        current_view.pan_y = ix.view_transform.pan_y;
        current_view.zoom = ix.view_transform.zoom;
    }

    // Build graph of node connections for force calculation
    fn build_connection_graph(&mut self, graph: &Graph) {
        self.connections.clear();

        // Initialize with empty vectors
        for id in graph.node_instances.keys() {
            self.connections.insert(id.clone(), Vec::new());
        }

        // Build connections graph (bidirectional for physics simulation)
        for (id, instance) in &graph.node_instances {
            for slot in &instance.slots {
                for conn in &slot.connections {
                    // Add bidirectional connection for force calculation
                    self.connections
                        .get_mut(id)
                        .unwrap()
                        .push(conn.target_node_id.clone());
                    self.connections
                        .get_mut(&conn.target_node_id)
                        .unwrap()
                        .push(id.clone());
                }
            }
        }
    }

    // Run a single iteration of the force simulation while a node is being dragged
    pub fn run_simulation_step(&mut self, graph: &mut Graph) {
        // Check both that simulation is active and physics is enabled for the current view
        if !self.force_simulation_active
            || self.fixed_node_id.is_none()
            || !self.views[self.current_view_index].physics_enabled
        {
            return;
        }

        // Get canvas dimensions for simulation bounds
        let canvas = window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(&self.canvas_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();

        let canvas_width = canvas.get_bounding_client_rect().width();
        let canvas_height = canvas.get_bounding_client_rect().height();

        // Extract current positions from graph
        let mut positions: HashMap<String, NodePosition> = HashMap::new();
        for (id, instance) in &graph.node_instances {
            positions.insert(
                id.clone(),
                NodePosition {
                    x: instance.x,
                    y: instance.y,
                },
            );
        }

        // Parameters for interactive simulation
        let repulsive_force = canvas_width * 15.0; // Slightly less than full sim
        let attractive_force = 0.01; // Stronger for more responsive dragging
        let center_gravity = 0.001; // Less gravity to allow free movement
        let cooling_factor = 0.995; // Slower cooling to maintain responsiveness

        // Calculate forces for this iteration
        let mut forces: HashMap<String, (f64, f64)> = HashMap::new();

        // Initialize forces to zero
        for id in positions.keys() {
            forces.insert(id.clone(), (0.0, 0.0));
        }

        // Calculate repulsive forces (nodes repel each other)
        let node_ids: Vec<String> = positions.keys().cloned().collect();
        for i in 0..node_ids.len() {
            for j in (i + 1)..node_ids.len() {
                let id1 = &node_ids[i];
                let id2 = &node_ids[j];

                let pos1 = &positions[id1];
                let pos2 = &positions[id2];

                let dx = pos1.x - pos2.x;
                let dy = pos1.y - pos2.y;

                // Avoid division by zero by adding a small value
                let distance_sq = dx * dx + dy * dy + 0.01;
                let distance = distance_sq.sqrt();

                // Repulsive force is inversely proportional to distance
                let force = repulsive_force / distance_sq;

                // Direction from node2 to node1 normalized
                let force_x = force * dx / distance;
                let force_y = force * dy / distance;

                // Add force to both nodes (action = -reaction)
                let (fx1, fy1) = forces.get(id1).unwrap();
                forces.insert(id1.clone(), (fx1 + force_x, fy1 + force_y));

                let (fx2, fy2) = forces.get(id2).unwrap();
                forces.insert(id2.clone(), (fx2 - force_x, fy2 - force_y));
            }
        }

        // Calculate attractive forces (connected nodes attract each other)
        for (id, connected_ids) in &self.connections {
            let pos1 = &positions[id];

            for connected_id in connected_ids {
                let pos2 = &positions[connected_id];

                let dx = pos1.x - pos2.x;
                let dy = pos1.y - pos2.y;

                let distance = (dx * dx + dy * dy).sqrt() + 0.01;

                // Attractive force is proportional to distance
                let force = attractive_force * distance;

                // Direction from node1 to node2 normalized
                let force_x = force * dx / distance;
                let force_y = force * dy / distance;

                // Only apply to the current node (the connected node will get its own turn)
                let (fx, fy) = forces.get(id).unwrap();
                forces.insert(id.clone(), (fx - force_x, fy - force_y));
            }
        }

        // Add center gravity to pull nodes toward the center
        let center_x = canvas_width / 2.0;
        let center_y = canvas_height / 2.0;

        for (id, pos) in &positions {
            let dx = pos.x - center_x;
            let dy = pos.y - center_y;

            let distance = (dx * dx + dy * dy).sqrt() + 0.01;
            let force = center_gravity * distance;

            let force_x = force * dx / distance;
            let force_y = force * dy / distance;

            let (fx, fy) = forces.get(id).unwrap();
            forces.insert(id.clone(), (fx - force_x, fy - force_y));
        }

        // Apply forces to update positions - but skip the fixed (dragged) node
        let fixed_node_id = self.fixed_node_id.as_ref().unwrap();

        for (id, (force_x, force_y)) in &forces {
            // Skip the node being dragged
            if id == fixed_node_id {
                continue;
            }

            // Get the node instance and update its position
            if let Some(instance) = graph.node_instances.get_mut(id) {
                // Limit maximum movement by temperature
                let force_magnitude = (force_x * force_x + force_y * force_y).sqrt();
                let scale = if force_magnitude > self.temperature {
                    self.temperature / force_magnitude
                } else {
                    1.0
                };

                // Update position - without canvas boundary constraints
                instance.x += force_x * scale;
                instance.y += force_y * scale;
            }
        }

        // Cool down system gradually
        self.temperature *= cooling_factor;
        self.simulation_iteration += 1;

        // Maintain a minimum temperature to keep the simulation responsive
        if self.temperature < 1.0 {
            self.temperature = 1.0;
        }

        // Only stop after an extremely large number of iterations
        if self.simulation_iteration > 10000 {
            self.force_simulation_active = false;
        }
    }

    pub fn generate_force_directed_layout(&self, graph: &Graph) -> LayoutSnapshot {
        let mut positions = HashMap::new();

        // Initialize positions - either use current positions or create random ones
        for (id, instance) in &graph.node_instances {
            positions.insert(
                id.clone(),
                NodePosition {
                    x: instance.x,
                    y: instance.y,
                },
            );
        }

        // Get canvas dimensions for positioning
        let canvas = window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(&self.canvas_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();

        let canvas_width = canvas.get_bounding_client_rect().width();
        let canvas_height = canvas.get_bounding_client_rect().height();

        // Create a map of node connections for force calculation
        let mut connections: HashMap<String, Vec<String>> = HashMap::new();
        for (id, _) in &graph.node_instances {
            connections.insert(id.clone(), Vec::new());
        }

        // Build connections graph
        for (id, instance) in &graph.node_instances {
            for slot in &instance.slots {
                for conn in &slot.connections {
                    // Add bidirectional connection for force calculation
                    connections
                        .get_mut(id)
                        .unwrap()
                        .push(conn.target_node_id.clone());
                    connections
                        .get_mut(&conn.target_node_id)
                        .unwrap()
                        .push(id.clone());
                }
            }
        }

        // Force-directed layout parameters
        let iterations = 300;
        let mut temperature = canvas_width * 0.8; // Initial "temperature" controls movement scale
        let cooling_factor = 0.99; // How quickly the system "cools down"
        let repulsive_force = canvas_width * 20.0; // Strength of repulsion between all nodes
        let attractive_force = 0.005; // Strength of attraction along edges
        let center_gravity = 0.003; // Force pulling nodes toward the center

        // Run simulation for a fixed number of iterations
        for _ in 0..iterations {
            // Calculate forces
            let mut forces: HashMap<String, (f64, f64)> = HashMap::new();

            // Initialize forces to zero
            for id in positions.keys() {
                forces.insert(id.clone(), (0.0, 0.0));
            }

            // Calculate repulsive forces (nodes repel each other)
            let node_ids: Vec<String> = positions.keys().cloned().collect();
            for i in 0..node_ids.len() {
                for j in (i + 1)..node_ids.len() {
                    let id1 = &node_ids[i];
                    let id2 = &node_ids[j];

                    let pos1 = &positions[id1];
                    let pos2 = &positions[id2];

                    let dx = pos1.x - pos2.x;
                    let dy = pos1.y - pos2.y;

                    // Avoid division by zero by adding a small value
                    let distance_sq = dx * dx + dy * dy + 0.01;
                    let distance = distance_sq.sqrt();

                    // Repulsive force is inversely proportional to distance
                    let force = repulsive_force / distance_sq;

                    // Direction from node2 to node1 normalized
                    let force_x = force * dx / distance;
                    let force_y = force * dy / distance;

                    // Add force to both nodes (action = -reaction)
                    let (fx1, fy1) = forces.get(id1).unwrap();
                    forces.insert(id1.clone(), (fx1 + force_x, fy1 + force_y));

                    let (fx2, fy2) = forces.get(id2).unwrap();
                    forces.insert(id2.clone(), (fx2 - force_x, fy2 - force_y));
                }
            }

            // Calculate attractive forces (connected nodes attract each other)
            for (id, connected_ids) in &connections {
                let pos1 = &positions[id];

                for connected_id in connected_ids {
                    let pos2 = &positions[connected_id];

                    let dx = pos1.x - pos2.x;
                    let dy = pos1.y - pos2.y;

                    let distance = (dx * dx + dy * dy).sqrt() + 0.01;

                    // Attractive force is proportional to distance
                    let force = attractive_force * distance;

                    // Direction from node1 to node2 normalized
                    let force_x = force * dx / distance;
                    let force_y = force * dy / distance;

                    // Only apply to the current node (the connected node will get its own turn)
                    let (fx, fy) = forces.get(id).unwrap();
                    forces.insert(id.clone(), (fx - force_x, fy - force_y));
                }
            }

            // Add center gravity to pull nodes toward the center
            let center_x = canvas_width / 2.0;
            let center_y = canvas_height / 2.0;

            for (id, pos) in &positions {
                let dx = pos.x - center_x;
                let dy = pos.y - center_y;

                let distance = (dx * dx + dy * dy).sqrt() + 0.01;
                let force = center_gravity * distance;

                let force_x = force * dx / distance;
                let force_y = force * dy / distance;

                let (fx, fy) = forces.get(id).unwrap();
                forces.insert(id.clone(), (fx - force_x, fy - force_y));
            }

            // Apply forces to update positions
            for (id, (force_x, force_y)) in &forces {
                let pos = positions.get_mut(id).unwrap();

                // Limit maximum movement by temperature
                let force_magnitude = (force_x * force_x + force_y * force_y).sqrt();
                let scale = if force_magnitude > temperature {
                    temperature / force_magnitude
                } else {
                    1.0
                };

                // Update position - without canvas boundary constraints
                pos.x += force_x * scale;
                pos.y += force_y * scale;
            }

            // Cool down system
            temperature *= cooling_factor;
        }

        LayoutSnapshot { positions }
    }
}
