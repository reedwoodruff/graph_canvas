use core::fmt;

use wasm_bindgen::JsValue;

use crate::{graph::Connection, group, groupEnd, log};

#[derive(Debug, Clone)]
pub enum GraphError {
    SetupFailed(JsValue),
    ConfigurationError(String, Box<GraphError>),
    NodeNotFound(String),
    SlotNotFound {
        node_id: String,
        slot_id: String,
    },
    TemplateNotFound(String),
    InvalidConnection {
        connection: Connection,
        reason: String,
    },
    ValidationFailed(String),
    LockFailed(String),
    NodeCreationFailed {
        node_template_id: String,
        node_template_name: String,
        reason: Box<GraphError>,
    },
    NodeDeletionFailed {
        node_id: String,
        node_template_name: String,
        reason: Box<GraphError>,
    },
    Other(String),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::ConfigurationError(msg, inner_error) => {
                write!(f, "Configuration error: {}; {}", msg, inner_error)
            }
            GraphError::SetupFailed(err) => write!(f, "Setup failed: {:#?}", err),
            GraphError::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            GraphError::SlotNotFound { node_id, slot_id } => {
                write!(f, "Slot {} not found on node {}", slot_id, node_id)
            }
            GraphError::TemplateNotFound(id) => write!(f, "Template not found: {}", id),
            GraphError::InvalidConnection { connection, reason } => {
                let Connection {
                    host_node_id,
                    host_slot_template_id: host_slot_id,
                    target_node_id,
                    target_slot_template_id: target_slot_id,
                } = connection;
                write!(
                    f,
                    "Invalid connection from {}:{} to {}:{} - {}",
                    host_node_id, host_slot_id, target_node_id, target_slot_id, reason
                )
            }
            GraphError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
            GraphError::LockFailed(msg) => write!(f, "Lock acquisition failed: {}", msg),
            GraphError::NodeCreationFailed {
                node_template_id,
                node_template_name,
                reason,
            } => {
                group("Node Creation Failed");
                log(&format!("Node Template ID: {}", node_template_id));

                log(&format!("Template name: {}", node_template_name));
                log(&format!("Reason: {}", reason));
                groupEnd();
                Ok(())
                // write!(
                //             f,
                //             "Node creation failed: {}\n Template name: {}\n Reason: {:#?}",
                //             node_id, node_template_name, reason
                //         )
            }
            GraphError::NodeDeletionFailed {
                node_id,
                node_template_name,
                reason,
            } => {
                group("Node Deletion Failed");
                log(&format!("Node ID: {}", node_id));

                log(&format!("Template name: {}", node_template_name));
                log(&format!("Reason: {}", reason));
                groupEnd();
                Ok(())
                // write!(
                //             f,
                //             "Node deletion failed: {}\n Template name: {}\n Reason: {:#?}",
                //             node_id, node_template_name, reason
                // )
            }
            GraphError::Other(msg) => write!(f, "Other error: {}", msg),
        }
    }
}

impl std::error::Error for GraphError {}

// Helper trait for converting to JsValue
pub trait IntoJsError {
    fn into_js_error(self) -> wasm_bindgen::JsValue;
}

impl IntoJsError for GraphError {
    fn into_js_error(self) -> wasm_bindgen::JsValue {
        wasm_bindgen::JsValue::from_str(&self.to_string())
    }
}

// Helper type for Results that need to be converted to JS
pub type GraphResult<T> = Result<T, GraphError>;

pub fn log_and_convert_error<E: std::error::Error>(err: E) -> JsValue {
    log(&format!("Error: {:#}", err)); // {:#} shows the full error chain
    JsValue::from_str(&err.to_string())
}
