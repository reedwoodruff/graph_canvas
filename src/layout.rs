use std::collections::{HashMap, VecDeque};

use web_sys::HtmlCanvasElement;

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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LayoutType {
    Free,
    Hierarchical,
}

pub struct LayoutEngine {
    current_type: LayoutType,
    snapshots: HashMap<LayoutType, LayoutSnapshot>,
    canvas_ref: HtmlCanvasElement,
}

impl LayoutEngine {
    pub fn new(canvas_ref: HtmlCanvasElement) -> Self {
        Self {
            current_type: LayoutType::Free,
            snapshots: HashMap::new(),
            canvas_ref,
        }
    }

    pub fn switch_layout(&mut self, layout_type: LayoutType, graph: &mut Graph) {
        // Save current positions to current layout snapshot
        self.save_current_snapshot(graph);

        // Switch to new layout
        self.current_type = layout_type.clone();

        // If we don't have a snapshot for this layout type, generate one
        if !self.snapshots.contains_key(&layout_type) {
            let new_snapshot = match layout_type {
                LayoutType::Free => self.generate_free_layout(graph),
                LayoutType::Hierarchical => self.generate_hierarchical_layout(graph),
            };
            self.snapshots.insert(layout_type.clone(), new_snapshot);
        }

        // Apply the layout snapshot
        self.apply_snapshot(graph, &self.snapshots[&layout_type]);
    }

    pub fn reset_current_layout(&mut self, graph: &mut Graph, ix: &mut InteractionState) {
        // Regenerate the current layout snapshot
        let new_snapshot = match self.current_type {
            LayoutType::Free => self.generate_free_layout(graph),
            LayoutType::Hierarchical => self.generate_hierarchical_layout(graph),
        };
        self.snapshots
            .insert(self.current_type.clone(), new_snapshot.clone());

        // Reset pan offset
        ix.view_transform.pan_x = 0.0;
        ix.view_transform.pan_y = 0.0;

        // Apply the new snapshot
        self.apply_snapshot(graph, &new_snapshot);
    }

    fn save_current_snapshot(&mut self, graph: &Graph) {
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
        self.snapshots
            .insert(self.current_type.clone(), LayoutSnapshot { positions });
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

        // Create dependency graphs
        let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
        let mut reverse_dependencies: HashMap<String, Vec<String>> = HashMap::new();

        graph.node_instances.keys().for_each(|id| {
            dependencies.insert(id.clone(), Vec::new());
            reverse_dependencies.insert(id.clone(), Vec::new());
        });
        // Build dependency graph
        for (_id, instance) in &graph.node_instances {
            for slot in &instance.slots {
                for conn in &slot.connections {
                    dependencies
                        .get_mut(&conn.host_node_id)
                        .unwrap()
                        .push(conn.target_node_id.clone());
                    reverse_dependencies
                        .get_mut(&conn.target_node_id)
                        .unwrap()
                        .push(conn.host_node_id.clone());
                }
            }
        }

        // Find root nodes (nodes with no incoming connections)
        let root_nodes: Vec<String> = graph
            .node_instances
            .keys()
            .filter(|id| reverse_dependencies[*id].is_empty())
            .cloned()
            .collect();

        // Assign levels through BFS
        let mut levels: HashMap<String, usize> = HashMap::new();
        let mut queue = VecDeque::new();

        // Add root nodes to queue with level 0
        for node in root_nodes {
            queue.push_back((node, 0));
        }

        // Process queue
        while let Some((node_id, level)) = queue.pop_front() {
            if !levels.contains_key(&node_id) {
                levels.insert(node_id.clone(), level);

                // Add children to queue
                for child in &dependencies[&node_id] {
                    queue.push_back((child.clone(), level + 1));
                }
            }
        }

        // Calculate positions based on levels
        let level_spacing = 250.0; // Horizontal spacing between levels
        let node_spacing = 150.0; // Vertical spacing between nodes

        // Group nodes by level
        let mut nodes_by_level: HashMap<usize, Vec<String>> = HashMap::new();
        for (node_id, level) in &levels {
            nodes_by_level
                .entry(*level)
                .or_insert_with(Vec::new)
                .push(node_id.clone());
        }

        // Canvas size offset
        let canvas_width_offset = self.canvas_ref.get_bounding_client_rect().width() / 10.0;
        let canvas_height_offset = self.canvas_ref.get_bounding_client_rect().height() / 2.0;

        // Position nodes
        for (level, nodes) in &nodes_by_level {
            let level_height = nodes.len() as f64 * node_spacing;
            let start_y = -level_height / 2.0;

            for (i, node_id) in nodes.iter().enumerate() {
                positions.insert(
                    node_id.clone(),
                    NodePosition {
                        x: canvas_width_offset + *level as f64 * level_spacing, // Use level for x-coordinate
                        y: canvas_height_offset + start_y + (i as f64 * node_spacing), // Stack nodes vertically
                    },
                );
            }
        }

        LayoutSnapshot { positions }
    }

    // Sort nodes within each level based on their connections
    fn optimize_level_ordering(
        &self,
        nodes_by_level: &mut HashMap<usize, Vec<String>>,
        dependencies: &HashMap<String, Vec<String>>,
        reverse_dependencies: &HashMap<String, Vec<String>>,
    ) {
        for level_nodes in nodes_by_level.values_mut() {
            level_nodes.sort_by_cached_key(|node_id| {
                // Calculate a "center of gravity" based on connected nodes' positions
                let incoming = reverse_dependencies.get(node_id).unwrap();
                let outgoing = dependencies.get(node_id).unwrap();

                // Weight based on number of connections
                let connection_weight = incoming.len() + outgoing.len();

                // You could also consider the relative positions of connected nodes
                // to minimize crossing connections
                connection_weight
            });
        }
    }

    fn minimize_edge_crossings(
        &self,
        nodes_by_level: &mut HashMap<usize, Vec<String>>,
        dependencies: &HashMap<String, Vec<String>>,
    ) {
        // Implement the Sugiyama algorithm's crossing minimization phase
        for level in 1..nodes_by_level.len() {
            let current_level = &nodes_by_level[&level];
            let previous_level = &nodes_by_level[&(level - 1)];

            // Calculate optimal ordering to minimize crossings between these levels
            let mut crossing_count = count_crossings(current_level, previous_level, dependencies);
            let mut best_ordering = current_level.clone();

            // Simple hill climbing
            for _ in 0..100 {
                // Number of attempts
                let mut new_ordering = current_level.clone();
                // Randomly swap two nodes
                if new_ordering.len() >= 2 {
                    let idx1 = rand::random::<u32>() as usize % new_ordering.len();
                    let idx2 = rand::random::<u32>() as usize % new_ordering.len();
                    new_ordering.swap(idx1, idx2);

                    let new_crossings =
                        count_crossings(&new_ordering, previous_level, dependencies);
                    if new_crossings < crossing_count {
                        crossing_count = new_crossings;
                        best_ordering = new_ordering;
                    }
                }
            }

            nodes_by_level.insert(level, best_ordering);
        }
    }

    fn optimize_layer_assignment(
        &self,
        levels: &mut HashMap<String, usize>,
        dependencies: &HashMap<String, Vec<String>>,
        reverse_dependencies: &HashMap<String, Vec<String>>,
    ) {
        // Try to minimize the total edge length
        let mut changed = true;
        while changed {
            changed = false;

            for (node_id, level) in levels.clone().iter() {
                let incoming = reverse_dependencies.get(node_id).unwrap();
                let outgoing = dependencies.get(node_id).unwrap();

                if !incoming.is_empty() || !outgoing.is_empty() {
                    // Calculate optimal level based on connected nodes
                    let incoming_levels: Vec<usize> = incoming.iter().map(|n| levels[n]).collect();
                    let outgoing_levels: Vec<usize> = outgoing.iter().map(|n| levels[n]).collect();

                    let optimal_level =
                        if !incoming_levels.is_empty() && !outgoing_levels.is_empty() {
                            // Try to position node between its incoming and outgoing connections
                            (incoming_levels.iter().sum::<usize>() / incoming_levels.len()
                                + outgoing_levels.iter().sum::<usize>() / outgoing_levels.len())
                                / 2
                        } else if !incoming_levels.is_empty() {
                            incoming_levels.iter().sum::<usize>() / incoming_levels.len() + 1
                        } else {
                            outgoing_levels.iter().sum::<usize>() / outgoing_levels.len() - 1
                        };

                    if optimal_level != *level {
                        levels.insert(node_id.clone(), optimal_level);
                        changed = true;
                    }
                }
            }
        }
    }

    fn optimize_node_spacing(
        &self,
        positions: &mut HashMap<String, NodePosition>,
        dependencies: &HashMap<String, Vec<String>>,
        node_size: (f64, f64),
    ) {
        let prev_positions = positions.clone(); // Store previous positions

        for (node_id, pos) in positions.iter_mut() {
            let connected_nodes: Vec<&String> = dependencies[node_id]
                .iter()
                .chain(dependencies.iter().filter_map(|(k, v)| {
                    if v.contains(node_id) {
                        Some(k)
                    } else {
                        None
                    }
                }))
                .collect();

            if !connected_nodes.is_empty() {
                // Use prev_positions for averaging
                let avg_x = connected_nodes
                    .iter()
                    .filter_map(|n| prev_positions.get(*n))
                    .map(|p| p.x)
                    .sum::<f64>()
                    / connected_nodes.len() as f64;
                let avg_y = connected_nodes
                    .iter()
                    .filter_map(|n| prev_positions.get(*n))
                    .map(|p| p.y)
                    .sum::<f64>()
                    / connected_nodes.len() as f64;

                // Move slightly toward connected nodes while maintaining minimum spacing
                pos.x = pos.x * 0.8 + avg_x * 0.2;
                pos.y = pos.y * 0.8 + avg_y * 0.2;
            }
        }
    }

    fn fancy_optimize_node_spacing(
        &self,
        positions: &mut HashMap<String, NodePosition>,
        dependencies: &HashMap<String, Vec<String>>,
        node_size: (f64, f64),
    ) {
        let iterations = 5; // Number of refinement passes
        let min_spacing = 20.0;

        for iteration in 0..iterations {
            let prev_positions = positions.clone();
            let mut total_movement = 0.0;

            for (node_id, pos) in positions.iter_mut() {
                let connected_nodes: Vec<&String> = dependencies[node_id]
                    .iter()
                    .chain(dependencies.iter().filter_map(|(k, v)| {
                        if v.contains(node_id) {
                            Some(k)
                        } else {
                            None
                        }
                    }))
                    .collect();

                if !connected_nodes.is_empty() {
                    let avg_x = connected_nodes
                        .iter()
                        .filter_map(|n| prev_positions.get(*n))
                        .map(|p| p.x)
                        .sum::<f64>()
                        / connected_nodes.len() as f64;
                    let avg_y = connected_nodes
                        .iter()
                        .filter_map(|n| prev_positions.get(*n))
                        .map(|p| p.y)
                        .sum::<f64>()
                        / connected_nodes.len() as f64;

                    let old_x = pos.x;
                    let old_y = pos.y;

                    // Use iteration instead of _
                    let movement_factor = 0.2 * (1.0 - (iteration as f64 / iterations as f64));
                    pos.x = pos.x * (1.0 - movement_factor) + avg_x * movement_factor;
                    pos.y = pos.y * (1.0 - movement_factor) + avg_y * movement_factor;

                    // Track total movement
                    total_movement += ((pos.x - old_x).powi(2) + (pos.y - old_y).powi(2)).sqrt();
                }
            }

            // Optional: break early if movement is very small
            if total_movement < min_spacing {
                break;
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
}

fn count_crossings(
    level: &[String],
    previous_level: &[String],
    dependencies: &HashMap<String, Vec<String>>,
) -> usize {
    let mut crossings = 0;

    for (i1, n1) in level.iter().enumerate() {
        for (i2, n2) in level.iter().enumerate().skip(i1 + 1) {
            for (j1, p1) in previous_level.iter().enumerate() {
                for (j2, p2) in previous_level.iter().enumerate().skip(j1 + 1) {
                    if dependencies[n1].contains(p2) && dependencies[n2].contains(p1) {
                        crossings += 1;
                    }
                }
            }
        }
    }

    crossings
}
