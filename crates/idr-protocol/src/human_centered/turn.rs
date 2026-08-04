use super::{
    contract_digest, validate_required_text, HumanCenteredContractKindV1,
    HumanCenteredContractMetadataV1, HumanCenteredContractRefV1, HumanCenteredProtocolError,
    TurnCoordinationModeV1,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnGateKindV1 {
    UserAuthorization,
    UserInput,
    PolicyApproval,
    HumanApprover,
    Dependency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnDependencyEdgeV1 {
    pub before: HumanCenteredContractRefV1,
    pub after: HumanCenteredContractRefV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnGateV1 {
    pub gate_kind: TurnGateKindV1,
    pub required_before: HumanCenteredContractRefV1,
    pub policy_ref: Option<crate::ProductionReferenceV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnFailurePolicyV1 {
    ReportOnly,
    ReportAndOfferRecovery,
    CompensateThenReport,
    Escalate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnProgressPolicyV1 {
    Silent,
    AcknowledgeOnly,
    StreamStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnCoordinationPlanV1 {
    metadata: HumanCenteredContractMetadataV1,
    mode: TurnCoordinationModeV1,
    response_refs: Vec<HumanCenteredContractRefV1>,
    action_refs: Vec<HumanCenteredContractRefV1>,
    dependencies: Vec<TurnDependencyEdgeV1>,
    gates: Vec<TurnGateV1>,
    failure_policy: TurnFailurePolicyV1,
    progress_policy: TurnProgressPolicyV1,
    rationale: String,
    record_digest: String,
}

impl TurnCoordinationPlanV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        metadata: HumanCenteredContractMetadataV1,
        mode: TurnCoordinationModeV1,
        response_refs: Vec<HumanCenteredContractRefV1>,
        action_refs: Vec<HumanCenteredContractRefV1>,
        dependencies: Vec<TurnDependencyEdgeV1>,
        gates: Vec<TurnGateV1>,
        failure_policy: TurnFailurePolicyV1,
        progress_policy: TurnProgressPolicyV1,
        rationale: impl Into<String>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let mut value = Self {
            metadata,
            mode,
            response_refs,
            action_refs,
            dependencies,
            gates,
            failure_policy,
            progress_policy,
            rationale: rationale.into(),
            record_digest: String::new(),
        };
        value.record_digest = {
            let input = value.digest_input();
            contract_digest("turn-coordination-plan-v1", &input)?
        };
        value.validate()?;
        Ok(value)
    }

    fn digest_input(&self) -> impl Serialize + '_ {
        (
            &self.metadata,
            self.mode,
            &self.response_refs,
            &self.action_refs,
            &self.dependencies,
            &self.gates,
            self.failure_policy,
            self.progress_policy,
            &self.rationale,
        )
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        self.metadata.validate()?;
        if self.metadata.contract_kind() != HumanCenteredContractKindV1::TurnCoordination {
            return Err(HumanCenteredProtocolError::ContractKindMismatch);
        }
        validate_required_text(&self.rationale)?;
        if self.response_refs.is_empty()
            || self
                .response_refs
                .iter()
                .any(|reference| reference.kind() != HumanCenteredContractKindV1::Response)
            || self
                .action_refs
                .iter()
                .any(|reference| reference.kind() != HumanCenteredContractKindV1::Action)
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        let action_mode = self.mode != TurnCoordinationModeV1::RespondOnly;
        if action_mode == self.action_refs.is_empty() {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        let confirmation_mode = self.mode == TurnCoordinationModeV1::RespondThenConfirmThenAct;
        let has_authorization_gate = self
            .gates
            .iter()
            .any(|gate| gate.gate_kind == TurnGateKindV1::UserAuthorization);
        if confirmation_mode != has_authorization_gate {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        let known_refs: BTreeSet<_> = self
            .response_refs
            .iter()
            .chain(self.action_refs.iter())
            .cloned()
            .collect();
        if known_refs
            .iter()
            .any(|reference| !self.metadata.dependency_refs().contains(reference))
        {
            return Err(HumanCenteredProtocolError::InvalidDependency);
        }
        for edge in &self.dependencies {
            edge.before.validate()?;
            edge.after.validate()?;
            if edge.before == edge.after
                || !known_refs.contains(&edge.before)
                || !known_refs.contains(&edge.after)
            {
                return Err(HumanCenteredProtocolError::InvalidDependency);
            }
        }
        if !dependency_graph_is_acyclic(&known_refs, &self.dependencies) {
            return Err(HumanCenteredProtocolError::InvalidDependency);
        }
        for gate in &self.gates {
            gate.required_before.validate()?;
            if !known_refs.contains(&gate.required_before) {
                return Err(HumanCenteredProtocolError::InvalidDependency);
            }
            if gate.gate_kind == TurnGateKindV1::UserAuthorization
                && gate.required_before.kind() != HumanCenteredContractKindV1::Action
            {
                return Err(HumanCenteredProtocolError::InvalidDependency);
            }
        }
        if confirmation_mode
            && self.action_refs.iter().any(|action| {
                !self.gates.iter().any(|gate| {
                    gate.gate_kind == TurnGateKindV1::UserAuthorization
                        && &gate.required_before == action
                })
            })
        {
            return Err(HumanCenteredProtocolError::InvalidDependency);
        }
        let response_before_action = matches!(
            self.mode,
            TurnCoordinationModeV1::RespondThenAct
                | TurnCoordinationModeV1::RespondThenConfirmThenAct
                | TurnCoordinationModeV1::AcknowledgeThenRun
        );
        if response_before_action
            && self.action_refs.iter().any(|action| {
                !self
                    .response_refs
                    .iter()
                    .any(|response| dependency_path_exists(response, action, &self.dependencies))
            })
        {
            return Err(HumanCenteredProtocolError::InvalidDependency);
        }
        if self.mode == TurnCoordinationModeV1::ActThenRespond
            && self.response_refs.iter().any(|response| {
                !self
                    .action_refs
                    .iter()
                    .any(|action| dependency_path_exists(action, response, &self.dependencies))
            })
        {
            return Err(HumanCenteredProtocolError::InvalidDependency);
        }
        if contract_digest("turn-coordination-plan-v1", &self.digest_input())? != self.record_digest
        {
            return Err(HumanCenteredProtocolError::DigestMismatch);
        }
        Ok(())
    }

    pub fn record_ref(&self) -> Result<HumanCenteredContractRefV1, HumanCenteredProtocolError> {
        self.validate()?;
        HumanCenteredContractRefV1::new(
            HumanCenteredContractKindV1::TurnCoordination,
            self.metadata.contract_id(),
            self.metadata.revision(),
            self.record_digest.clone(),
        )
    }

    pub fn metadata(&self) -> &HumanCenteredContractMetadataV1 {
        &self.metadata
    }

    pub fn response_refs(&self) -> &[HumanCenteredContractRefV1] {
        &self.response_refs
    }

    pub fn action_refs(&self) -> &[HumanCenteredContractRefV1] {
        &self.action_refs
    }
}

fn dependency_graph_is_acyclic(
    nodes: &BTreeSet<HumanCenteredContractRefV1>,
    edges: &[TurnDependencyEdgeV1],
) -> bool {
    let mut indegrees: BTreeMap<_, usize> = nodes.iter().cloned().map(|node| (node, 0)).collect();
    let mut adjacency: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    for edge in edges {
        if adjacency
            .entry(edge.before.clone())
            .or_default()
            .insert(edge.after.clone())
        {
            *indegrees.entry(edge.after.clone()).or_default() += 1;
        }
    }
    let mut ready: VecDeque<_> = indegrees
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some(node.clone()))
        .collect();
    let mut visited = 0;
    while let Some(node) = ready.pop_front() {
        visited += 1;
        if let Some(next_nodes) = adjacency.get(&node) {
            for next in next_nodes {
                let Some(degree) = indegrees.get_mut(next) else {
                    return false;
                };
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back(next.clone());
                }
            }
        }
    }
    visited == nodes.len()
}

fn dependency_path_exists(
    start: &HumanCenteredContractRefV1,
    target: &HumanCenteredContractRefV1,
    edges: &[TurnDependencyEdgeV1],
) -> bool {
    let mut queue = VecDeque::from([start.clone()]);
    let mut visited = BTreeSet::new();
    while let Some(current) = queue.pop_front() {
        if !visited.insert(current.clone()) {
            continue;
        }
        for next in edges
            .iter()
            .filter(|edge| edge.before == current)
            .map(|edge| &edge.after)
        {
            if next == target {
                return true;
            }
            queue.push_back(next.clone());
        }
    }
    false
}
