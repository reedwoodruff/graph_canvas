use std::collections::HashMap;

use web_sys::HtmlCanvasElement;

use crate::{graph::Graph, interaction::InteractionState, log};

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

        // Initialize dependencies
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

        // Simple level assignment: nodes with no incoming edges are level 0,
        // then each node's level is one more than its minimum-level incoming node
        let mut levels: HashMap<String, usize> = HashMap::new();

        // First assign level 0 to nodes with no incoming edges
        for (node_id, incoming) in &reverse_dependencies {
            if incoming.is_empty() {
                levels.insert(node_id.clone(), 0);
            }
        }

        // If no root nodes found, assign level 0 to an arbitrary node
        if levels.is_empty() && !graph.node_instances.is_empty() {
            let start_node = graph.node_instances.keys().next().unwrap();
            levels.insert(start_node.clone(), 0);
        }

        // Then make a fixed number of passes to assign levels to remaining nodes
        let max_passes = graph.node_instances.len();
        for _ in 0..max_passes {
            let current_levels = levels.clone();

            for node_id in graph.node_instances.keys() {
                if !levels.contains_key(node_id) {
                    // Check if any incoming nodes have levels assigned
                    let incoming_levels: Vec<usize> = reverse_dependencies[node_id]
                        .iter()
                        .filter_map(|n| current_levels.get(n))
                        .cloned()
                        .collect();

                    if !incoming_levels.is_empty() {
                        // Assign this node's level as one more than the minimum incoming level
                        levels.insert(node_id.clone(), incoming_levels.iter().min().unwrap() + 1);
                    }
                }
            }
        }

        // Assign level 0 to any remaining nodes
        for node_id in graph.node_instances.keys() {
            levels.entry(node_id.clone()).or_insert(0);
        }

        // Group nodes by level for positioning
        let mut nodes_by_level: HashMap<usize, Vec<String>> = HashMap::new();
        for (node_id, level) in &levels {
            nodes_by_level
                .entry(*level)
                .or_insert_with(Vec::new)
                .push(node_id.clone());
        }

        self.optimize_level_ordering(&mut nodes_by_level, &dependencies, &reverse_dependencies);
        self.minimize_edge_crossings(&mut nodes_by_level, &dependencies);

        // Position nodes
        let level_spacing = 250.0;
        let node_spacing = 150.0;

        // Canvas size offset
        let canvas_width_offset = self.canvas_ref.get_bounding_client_rect().width() / 10.0;
        let canvas_height_offset = self.canvas_ref.get_bounding_client_rect().height() / 2.0;

        for (level, nodes) in &nodes_by_level {
            let level_height = nodes.len() as f64 * node_spacing;
            let start_y = -level_height / 2.0;

            for (i, node_id) in nodes.iter().enumerate() {
                positions.insert(
                    node_id.clone(),
                    NodePosition {
                        x: canvas_width_offset + *level as f64 * level_spacing,
                        y: canvas_height_offset + start_y + (i as f64 * node_spacing),
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
        for (_i2, n2) in level.iter().enumerate().skip(i1 + 1) {
            for (j1, p1) in previous_level.iter().enumerate() {
                for (_j2, p2) in previous_level.iter().enumerate().skip(j1 + 1) {
                    if dependencies[n1].contains(p2) && dependencies[n2].contains(p1) {
                        crossings += 1;
                    }
                }
            }
        }
    }

    crossings
}
