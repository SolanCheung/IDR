use super::{
    valid_digest, HumanCenteredInputEventIdV1, HumanCenteredProtocolError, HumanCenteredRunIdV1,
    HumanCenteredTurnIdV1, HUMAN_CENTERED_MAX_SAFE_WIRE_INTEGER, HUMAN_CENTERED_SCHEMA_VERSION,
};
use crate::ProductionReferenceV1;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceActorV1 {
    User,
    Host,
    Agent,
    Tool,
    ExternalSystem,
    HumanApprover,
    Scheduler,
    PolicyEngine,
    DeviceEnvironment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRoleV1 {
    Objective,
    Intent,
    Command,
    Fact,
    Observation,
    Proposal,
    Authorization,
    Policy,
    Result,
    Feedback,
    TimeTrigger,
    SecuritySignal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalInputEventV1 {
    event_id: HumanCenteredInputEventIdV1,
    run_id: HumanCenteredRunIdV1,
    turn_id: HumanCenteredTurnIdV1,
    source_actor: SourceActorV1,
    actor_ref: ProductionReferenceV1,
    primary_semantic_role: SemanticRoleV1,
    semantic_roles: Vec<SemanticRoleV1>,
    content_ref: ProductionReferenceV1,
    content_digest: String,
    correlation_ref: ProductionReferenceV1,
    logical_time: u64,
    schema_version: u16,
}

impl CanonicalInputEventV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: HumanCenteredRunIdV1,
        turn_id: HumanCenteredTurnIdV1,
        source_actor: SourceActorV1,
        actor_ref: ProductionReferenceV1,
        primary_semantic_role: SemanticRoleV1,
        mut semantic_roles: Vec<SemanticRoleV1>,
        content_ref: ProductionReferenceV1,
        content_digest: impl Into<String>,
        correlation_ref: ProductionReferenceV1,
        logical_time: u64,
    ) -> Result<Self, HumanCenteredProtocolError> {
        semantic_roles.sort();
        semantic_roles.dedup();
        let value = Self {
            event_id: HumanCenteredInputEventIdV1::mint(),
            run_id,
            turn_id,
            source_actor,
            actor_ref,
            primary_semantic_role,
            semantic_roles,
            content_ref,
            content_digest: content_digest.into(),
            correlation_ref,
            logical_time,
            schema_version: HUMAN_CENTERED_SCHEMA_VERSION,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        let roles: BTreeSet<_> = self.semantic_roles.iter().copied().collect();
        if self.event_id.is_empty()
            || self.run_id.is_empty()
            || self.turn_id.is_empty()
            || self.semantic_roles.is_empty()
            || self.semantic_roles.len() > 16
            || roles.len() != self.semantic_roles.len()
            || !roles.contains(&self.primary_semantic_role)
            || self
                .semantic_roles
                .iter()
                .any(|role| !source_actor_may_assert(self.source_actor, *role))
            || self.actor_ref.as_str().trim().is_empty()
            || self.content_ref.as_str().trim().is_empty()
            || self.correlation_ref.as_str().trim().is_empty()
            || !self
                .content_digest
                .strip_prefix("sha256:")
                .is_some_and(valid_digest)
            || self.logical_time == 0
            || self.logical_time > HUMAN_CENTERED_MAX_SAFE_WIRE_INTEGER
            || self.schema_version != HUMAN_CENTERED_SCHEMA_VERSION
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(())
    }

    pub fn event_id(&self) -> Uuid {
        self.event_id.as_uuid()
    }

    pub fn run_id(&self) -> HumanCenteredRunIdV1 {
        self.run_id
    }

    pub fn turn_id(&self) -> HumanCenteredTurnIdV1 {
        self.turn_id
    }

    pub fn source_actor(&self) -> SourceActorV1 {
        self.source_actor
    }

    pub fn primary_semantic_role(&self) -> SemanticRoleV1 {
        self.primary_semantic_role
    }

    pub fn semantic_roles(&self) -> &[SemanticRoleV1] {
        &self.semantic_roles
    }

    pub fn content_ref(&self) -> &ProductionReferenceV1 {
        &self.content_ref
    }
}

fn source_actor_may_assert(source: SourceActorV1, role: SemanticRoleV1) -> bool {
    use SemanticRoleV1::*;
    use SourceActorV1::*;
    match source {
        User => matches!(
            role,
            Objective | Intent | Command | Fact | Proposal | Authorization | Feedback
        ),
        Host => !matches!(role, Authorization),
        Agent => matches!(role, Fact | Observation | Proposal | Result | Feedback),
        Tool => matches!(role, Fact | Observation | Result | SecuritySignal),
        ExternalSystem => matches!(role, Fact | Observation | Result | SecuritySignal),
        HumanApprover => matches!(role, Authorization | Feedback | Fact),
        Scheduler => role == TimeTrigger,
        PolicyEngine => matches!(role, Policy | Authorization | SecuritySignal),
        DeviceEnvironment => matches!(role, Observation | Fact | SecuritySignal),
    }
}
