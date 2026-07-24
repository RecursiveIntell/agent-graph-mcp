use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    GraphRead,
    GraphCreate,
    GraphRun,
    GraphCancel,
    WitnessCapture,
    WitnessRead,
    CheckpointRequest,
    CheckpointRead,
    ApprovalDecide,
    GraphDelete,
    DatabaseMigration,
    ConfigInstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Principal {
    ModelClient,
    StdioProxy,
    Daemon,
    LocalOperator,
}

#[derive(Debug, Clone)]
pub struct CapabilityPolicy {
    principal: Principal,
    capabilities: BTreeSet<Capability>,
}

impl CapabilityPolicy {
    pub fn model() -> Self {
        Self {
            principal: Principal::ModelClient,
            capabilities: [
                Capability::GraphRead,
                Capability::GraphCreate,
                Capability::GraphRun,
                Capability::GraphCancel,
                Capability::WitnessCapture,
                Capability::WitnessRead,
                Capability::CheckpointRequest,
                Capability::CheckpointRead,
            ]
            .into_iter()
            .collect(),
        }
    }
    pub fn allows(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
    pub fn principal(&self) -> Principal {
        self.principal
    }
    pub fn require(&self, capability: Capability) -> Result<(), &'static str> {
        if self.allows(capability) {
            Ok(())
        } else {
            Err("FORBIDDEN")
        }
    }
}
