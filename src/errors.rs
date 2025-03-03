use core::fmt;

use wasm_bindgen::JsValue;

use crate::{graph::Connection, log};

#[derive(Debug, Clone)]
pub enum GraphError {
    SaveFailed {
        reason: Box<GraphError>,
    },
    SlotMalformed {
        template_name: String,
        slot_name: String,
        min: usize,
        max: Option<usize>,
        actual: usize,
    },
    GraphLockFailed,
    SetupFailed(JsValue),
    ConfigurationError(String, Box<GraphError>),
    NodeNotFound(String),
    SlotNotFound {
        node_id: String,
        slot_id: String,
    },
    TemplateNotFound(String),
    ConnectionCreationFailed {
        node_template_name: String,
        slot_template_name: String,
        reason: Box<GraphError>,
    },
    InvalidConnection {
        connection: Connection,
        reason: String,
    },
    ValidationFailed(String),
    LockFailed(String),
    SomeSlotDeletionsFailed {
        failures: Vec<GraphError>,
    },
    SomeConnectionDeletionsFailed {
        failures: Vec<GraphError>,
    },
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
    SlotDeletionFailed {
        slot_name: String,
        reason: Box<GraphError>,
    },
    ConnectionDeletionFailed {
        connection: Connection,
        reason: Box<GraphError>,
    },
    ConnectionLocked,
    SlotTemplateLocked {
        name: String,
    },
    SlotInstanceLocked,
    NodeTemplateLocked {
        name: String,
    },
    NodeInstanceLocked,
    ListOfErrors(Vec<GraphError>),
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
            GraphError::ConnectionCreationFailed {
                node_template_name,
                slot_template_name,
                reason,
            } => {
                write!(
                    f,
                    "Connection Creation Failed. From node {}, slot {}; Reason: {:#?}",
                    node_template_name, slot_template_name, reason
                )
            }
            GraphError::InvalidConnection { connection, reason } => {
                let Connection {
                    host_node_id,
                    host_slot_template_id: host_slot_id,
                    target_node_id,
                    target_slot_template_id: target_slot_id,
                    ..
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
                write!(
                    f,
                    "Node creation failed: {}\n Template name: {}\n Reason: {:#?}",
                    node_template_id, node_template_name, reason
                )
            }
            GraphError::NodeDeletionFailed {
                node_id,
                node_template_name,
                reason,
            } => {
                write!(
                    f,
                    "Node deletion failed: {}\n Template name: {}\n Reason: {:#?}",
                    node_id, node_template_name, reason
                )
            }
            GraphError::SlotDeletionFailed { slot_name, reason } => {
                write!(
                    f,
                    "Slot deletion failed: {}\n Reason: {:#?}",
                    slot_name, reason
                )
            }
            GraphError::ConnectionDeletionFailed { connection, reason } => {
                write!(
                    f,
                    "Connection deletion failed; {:?}; {}",
                    connection, reason
                )
            }
            GraphError::ConnectionLocked => {
                write!(f, "Connection is locked and cannot be deleted")
            }
            GraphError::SlotTemplateLocked { name } => {
                write!(
                    f,
                    "Slot Template is locked and cannot be modified: {}",
                    name
                )
            }
            GraphError::SlotInstanceLocked => {
                write!(f, "Node Instance is locked and cannot be modified")
            }
            GraphError::NodeTemplateLocked { name } => {
                write!(
                    f,
                    "Node Template is locked and cannot be modified: {}",
                    name
                )
            }
            GraphError::NodeInstanceLocked => {
                write!(f, "Node Instance is locked and cannot be modified")
            }
            GraphError::SomeConnectionDeletionsFailed { failures } => {
                write!(
                    f,
                    "Some connections in the requested action were not deleted"
                )?;
                for failure in failures {
                    write!(f, "{}", failure)?;
                }
                Ok(())
            }
            GraphError::SomeSlotDeletionsFailed { failures } => {
                write!(f, "Some slots in the requested action were not cleared")?;
                for failure in failures {
                    write!(f, "{}", failure)?;
                }
                Ok(())
            }
            GraphError::SaveFailed { reason } => {
                write!(f, "Save action failed. \nReason: {:#?}", reason)
            }
            GraphError::GraphLockFailed => {
                write!(f, "Could not obtain graph lock")
            }
            GraphError::SlotMalformed {
                template_name,
                slot_name,
                min,
                max,
                actual,
            } => {
                write!(f, "Slot is not well-formed. \nTemplate Name: {}, Slot Name: {}, Minimum Connections: {}, Maximum Connections: {:#?}, Current Number of Instances: {}", template_name, slot_name, min, max, actual)
            }
            GraphError::ListOfErrors(vec) => {
                for error in vec {
                    write!(f, "{:#?}", error)?;
                }
                Ok(())
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
