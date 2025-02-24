use uuid::Uuid;

use crate::graph::SlotPosition;
pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn get_bezier_control_points(
    x: f64,
    y: f64,
    position: &SlotPosition,
    control_point_distance: f64,
) -> (f64, f64) {
    match position {
        SlotPosition::Right => (x + control_point_distance, y),
        SlotPosition::Left => (x - control_point_distance, y),
        SlotPosition::Top => (x, y - control_point_distance),
        SlotPosition::Bottom => (x, y + control_point_distance),
    }
}

// Helper function to adjust a point's position based on an angle from center
pub fn get_point_from_center(
    center_x: f64,
    center_y: f64,
    angle: f64,
    distance: f64,
) -> (f64, f64) {
    (
        center_x + distance * angle.cos(),
        center_y + distance * angle.sin(),
    )
}
