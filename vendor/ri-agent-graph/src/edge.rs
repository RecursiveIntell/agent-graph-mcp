use crate::router::RoutingFunction;

/// Type of edge in the graph
pub enum EdgeType {
    /// Normal edge: always goes to specified node
    Normal(String),

    /// Conditional edge: calls router to determine next node
    Conditional(Box<dyn RoutingFunction>),
}

impl std::fmt::Debug for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeType::Normal(next) => write!(f, "Normal({})", next),
            EdgeType::Conditional(_) => write!(f, "Conditional(<router>)"),
        }
    }
}
