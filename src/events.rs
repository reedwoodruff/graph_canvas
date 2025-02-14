use crate::{
    errors::GraphError,
    graph::{Connection, GraphCommand},
    interaction::ContextMenuTarget,
};

#[derive(Debug)]
pub enum SystemEvent {
    // State Changes
    NodeMoved {
        node: String,
        x: f64,
        y: f64,
    },
    ConnectionStarted {
        node: String,
        slot: String,
    },
    ConnectionCompleted(Connection),
    ConnectionFailed(String), // with reason

    // UI Events
    ContextMenuOpened(ContextMenuTarget),
    ContextMenuClosed,

    // Command Results
    CommandExecuted(GraphCommand),
    CommandFailed {
        command: GraphCommand,
        reason: GraphError,
    },
}

pub type EventListener = Box<dyn Fn(&SystemEvent) + Send + 'static>;

pub struct EventSystem {
    listeners: Vec<EventListener>,
}

impl Default for EventSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSystem {
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    pub fn subscribe(&mut self, listener: EventListener) {
        self.listeners.push(listener);
    }

    pub fn emit(&self, event: SystemEvent) {
        for listener in &self.listeners {
            listener(&event);
        }
    }
}
