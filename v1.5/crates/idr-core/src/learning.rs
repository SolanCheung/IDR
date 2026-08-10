use crate::runtime::{now_string, AppliedFeedback};
use crate::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const IMPLICIT_ACTIVATION_COUNT: usize = 3;

impl IdrCore {
    pub fn observe(
        &mut self,
        observation: ObservationV1,
    ) -> Result<HumanModelAssertionV1, IdrError> {
        validate_observation(&observation)?;
        let evidence = EvidenceV1 {
            evidence_id: observation.evidence_id.clone(),
            kind: match observation.source_type {
                AssertionSourceTypeV1::Explicit => EvidenceKindV1::ExplicitStatement,
                AssertionSourceTypeV1::Inferred => EvidenceKindV1::ImplicitBehavior,
            },
            strength: match observation.source_type {
                AssertionSourceTypeV1::Explicit => 0.95,
                AssertionSourceTypeV1::Inferred => 0.3,
            },
            source: "host_observation".into(),
            observed_at: observation.observed_at.clone(),
            data: json!({
                "kind": observation.kind,
                "predicate": observation.predicate,
                "value": observation.value,
                "scope": observation.scope,
            }),
        };
        self.evidence.push(evidence);
        Ok(self.apply_observation(observation))
    }

    pub fn query_human_model(
        &self,
        query: QueryHumanModelRequestV1,
    ) -> Vec<HumanModelAssertionV1> {
        let mut assertions = self.applicable_assertions(
            &query.subject_ref,
            &query.scope,
            &query.as_of,
        );
        if query.scope.0.is_empty() {
            assertions = self
                .assertions
                .iter()
                .filter(|assertion| {
                    assertion.subject_ref == query.subject_ref
                        && assertion_applies(assertion, &assertion.scope, &query.as_of)
                })
                .cloned()
                .collect();
        }
        assertions.sort_by(|left, right| {
            source_rank(&right.source_type)
                .cmp(&source_rank(&left.source_type))
                .then_with(|| right.confidence.total_cmp(&left.confidence))
        });
        assertions
    }

    pub fn feedback(
        &mut self,
        feedback: OutcomeFeedbackV1,
    ) -> Result<FeedbackResultV1, IdrError> {
        let stored = self
            .decisions
            .get(&feedback.decision_id)
            .cloned()
            .ok_or_else(|| IdrError::UnknownDecision(feedback.decision_id.clone()))?;
        let digest = feedback_digest(&feedback);
        if let Some(applied) = self.feedback_ledger.get(&feedback.decision_id) {
            if applied.feedback_digest == digest {
                return Ok(applied.result.clone());
            }
            return Err(IdrError::FeedbackConflict(feedback.decision_id.clone()));
        }
        validate_feedback_binding(&stored, &feedback)?;
        let evaluation_index = self
            .evaluations
            .iter()
            .position(|record| record.decision_id == feedback.decision_id)
            .ok_or_else(|| IdrError::UnknownDecision(feedback.decision_id.clone()))?;
        let mut emitted_evidence = Vec::new();
        let mut assertions_updated = Vec::new();

        let outcome_evidence = EvidenceV1 {
            evidence_id: self.next_id("evidence-outcome"),
            kind: EvidenceKindV1::Outcome,
            strength: outcome_strength(&feedback.outcome.status),
            source: "outcome_feedback".into(),
            observed_at: feedback.observed_at.clone(),
            data: serde_json::to_value(&feedback.outcome).unwrap_or(Value::Null),
        };
        self.evidence.push(outcome_evidence.clone());
        emitted_evidence.push(outcome_evidence);

        let decision_overridden = feedback.actual_action != feedback.recommended_action
            || feedback.user_response == UserResponseV1::Corrected;
        if decision_overridden {
            let override_evidence = EvidenceV1 {
                evidence_id: self.next_id("evidence-override"),
                kind: EvidenceKindV1::DecisionOverride,
                strength: 0.9,
                source: "outcome_feedback".into(),
                observed_at: feedback.observed_at.clone(),
                data: json!({
                    "recommended_action": feedback.recommended_action,
                    "actual_action": feedback.actual_action,
                    "outcome": feedback.outcome,
                }),
            };
            self.evidence.push(override_evidence.clone());
            emitted_evidence.push(override_evidence);
        }

        let learning_action = feedback
            .correction
            .as_ref()
            .and_then(|correction| correction.preferred_action.clone())
            .unwrap_or_else(|| feedback.actual_action.clone());
        let learning_source = match feedback.user_response {
            UserResponseV1::Corrected => Some(AssertionSourceTypeV1::Explicit),
            UserResponseV1::Accepted if feedback.outcome.status == OutcomeStatusV1::Success => {
                Some(AssertionSourceTypeV1::Inferred)
            }
            _ => None,
        };
        if let Some(source_type) = learning_source {
            let evidence_id = emitted_evidence
                .last()
                .map(|evidence| evidence.evidence_id.clone())
                .unwrap_or_else(|| self.next_id("evidence"));
            let assertion = self.apply_observation(ObservationV1 {
                evidence_id,
                subject_ref: stored.subject_ref.clone(),
                kind: AssertionKindV1::DecisionPattern,
                predicate: "preferred_action".into(),
                value: serde_json::to_value(learning_action).unwrap_or(Value::Null),
                scope: feedback.scope.clone(),
                source_type,
                observed_at: feedback.observed_at.clone(),
            });
            assertions_updated.push(assertion.assertion_id);
        }

        let corrected_intent = feedback
            .correction
            .as_ref()
            .and_then(|correction| correction.corrected_intent.as_ref())
            .is_some_and(|intent| intent != &stored.decision.resolved_intent);
        let evaluation = &mut self.evaluations[evaluation_index];
        evaluation.intent_corrected = corrected_intent;
        evaluation.decision_overridden = decision_overridden;
        evaluation.outcome_status = feedback.outcome.status.clone().into();
        for assertion_ref in &assertions_updated {
            if !evaluation
                .human_model_assertion_refs
                .contains(assertion_ref)
            {
                evaluation
                    .human_model_assertion_refs
                    .push(assertion_ref.clone());
            }
        }
        for evidence in &emitted_evidence {
            if !evaluation.evidence_refs.contains(&evidence.evidence_id) {
                evaluation.evidence_refs.push(evidence.evidence_id.clone());
            }
        }

        let result = FeedbackResultV1 {
            evidence: emitted_evidence,
            assertions_updated,
            evaluation_record: evaluation.clone(),
        };
        self.feedback_ledger.insert(
            feedback.decision_id,
            AppliedFeedback {
                feedback_digest: digest,
                result: result.clone(),
            },
        );
        Ok(result)
    }

    pub(crate) fn applicable_assertions(
        &self,
        subject_ref: &str,
        scope: &ScopeV1,
        as_of: &str,
    ) -> Vec<HumanModelAssertionV1> {
        self.assertions
            .iter()
            .filter(|assertion| {
                assertion.subject_ref == subject_ref && assertion_applies(assertion, scope, as_of)
            })
            .cloned()
            .collect()
    }

    fn apply_observation(&mut self, observation: ObservationV1) -> HumanModelAssertionV1 {
        let matching_index = self.assertions.iter().position(|assertion| {
            assertion.subject_ref == observation.subject_ref
                && assertion.kind == observation.kind
                && assertion.predicate == observation.predicate
                && assertion.scope == observation.scope
                && assertion.value == observation.value
                && matches!(
                    assertion.status,
                    AssertionStatusV1::Candidate | AssertionStatusV1::Active
                )
        });

        let index = if let Some(index) = matching_index {
            let assertion = &mut self.assertions[index];
            if !assertion.evidence_refs.contains(&observation.evidence_id) {
                assertion.evidence_refs.push(observation.evidence_id.clone());
            }
            assertion.last_confirmed_at = Some(observation.observed_at.clone());
            if observation.source_type == AssertionSourceTypeV1::Explicit {
                assertion.source_type = AssertionSourceTypeV1::Explicit;
                assertion.confidence = assertion.confidence.max(0.9);
                assertion.status = AssertionStatusV1::Active;
            } else {
                let observations = assertion.evidence_refs.len();
                assertion.confidence = implicit_confidence(observations);
                if observations >= IMPLICIT_ACTIVATION_COUNT {
                    assertion.status = AssertionStatusV1::Active;
                }
            }
            index
        } else {
            let explicit = observation.source_type == AssertionSourceTypeV1::Explicit;
            let assertion_id = self.next_id("assertion");
            self.assertions.push(HumanModelAssertionV1 {
                assertion_id,
                subject_ref: observation.subject_ref,
                kind: observation.kind,
                predicate: observation.predicate,
                value: observation.value,
                scope: observation.scope,
                confidence: if explicit { 0.9 } else { implicit_confidence(1) },
                source_type: observation.source_type,
                evidence_refs: vec![observation.evidence_id],
                status: if explicit {
                    AssertionStatusV1::Active
                } else {
                    AssertionStatusV1::Candidate
                },
                valid_from: observation.observed_at.clone(),
                valid_until: None,
                recorded_at: now_string(),
                last_confirmed_at: Some(observation.observed_at),
                supersedes: Vec::new(),
                contradicts: Vec::new(),
            });
            self.assertions.len() - 1
        };

        if self.assertions[index].status == AssertionStatusV1::Active {
            self.resolve_contradictions(index);
        }
        self.assertions[index].clone()
    }

    fn resolve_contradictions(&mut self, new_index: usize) {
        let new_assertion = self.assertions[new_index].clone();
        let conflicts = self
            .assertions
            .iter()
            .enumerate()
            .filter_map(|(index, assertion)| {
                (index != new_index
                    && assertion.subject_ref == new_assertion.subject_ref
                    && assertion.kind == new_assertion.kind
                    && assertion.predicate == new_assertion.predicate
                    && assertion.scope == new_assertion.scope
                    && assertion.value != new_assertion.value
                    && assertion.status == AssertionStatusV1::Active)
                    .then_some(index)
            })
            .collect::<Vec<_>>();

        for old_index in conflicts {
            let old = self.assertions[old_index].clone();
            if old.source_type == AssertionSourceTypeV1::Explicit
                && new_assertion.source_type == AssertionSourceTypeV1::Inferred
            {
                self.assertions[old_index].confidence =
                    (self.assertions[old_index].confidence - 0.05).max(0.0);
                push_unique(
                    &mut self.assertions[old_index].contradicts,
                    new_assertion.assertion_id.clone(),
                );
                self.assertions[new_index].status = AssertionStatusV1::Contradicted;
                self.assertions[new_index].confidence *= 0.5;
                push_unique(
                    &mut self.assertions[new_index].contradicts,
                    old.assertion_id,
                );
                continue;
            }

            self.assertions[old_index].confidence *= 0.5;
            self.assertions[old_index].status = AssertionStatusV1::Superseded;
            self.assertions[old_index].valid_until = new_assertion.last_confirmed_at.clone();
            push_unique(
                &mut self.assertions[old_index].contradicts,
                new_assertion.assertion_id.clone(),
            );
            push_unique(
                &mut self.assertions[new_index].supersedes,
                old.assertion_id.clone(),
            );
            push_unique(
                &mut self.assertions[new_index].contradicts,
                old.assertion_id,
            );
        }
    }
}

fn validate_feedback_binding(
    stored: &crate::runtime::StoredDecision,
    feedback: &OutcomeFeedbackV1,
) -> Result<(), IdrError> {
    if feedback.decision_digest != stored.decision.decision_digest {
        return Err(IdrError::InvalidContract(
            "feedback decision_digest mismatch".into(),
        ));
    }
    if feedback.recommended_action != stored.decision.recommended_action {
        return Err(IdrError::InvalidContract(
            "feedback recommended_action does not match the decision".into(),
        ));
    }
    if feedback.scope != stored.scope {
        return Err(IdrError::InvalidContract(
            "feedback scope does not match the decision scope".into(),
        ));
    }
    Ok(())
}

fn feedback_digest(feedback: &OutcomeFeedbackV1) -> String {
    let encoded = serde_json::to_vec(feedback).expect("feedback must be serializable");
    format!("{:x}", Sha256::digest(encoded))
}

pub(crate) fn assertion_applies(
    assertion: &HumanModelAssertionV1,
    scope: &ScopeV1,
    as_of: &str,
) -> bool {
    assertion.status == AssertionStatusV1::Active
        && assertion.scope.matches(scope)
        && assertion.valid_from.as_str() <= as_of
        && assertion
            .valid_until
            .as_ref()
            .is_none_or(|valid_until| valid_until.as_str() > as_of)
}

fn validate_observation(observation: &ObservationV1) -> Result<(), IdrError> {
    let predicate = observation.predicate.to_ascii_lowercase();
    if predicate.contains("personality")
        || predicate.contains("psychological")
        || predicate.contains("psychology")
    {
        return Err(IdrError::InvalidContract(
            "personality and psychological profiling are not supported".into(),
        ));
    }
    if observation.subject_ref.trim().is_empty()
        || observation.evidence_id.trim().is_empty()
        || observation.predicate.trim().is_empty()
    {
        return Err(IdrError::InvalidContract(
            "observation identifiers and predicate are required".into(),
        ));
    }
    Ok(())
}

fn implicit_confidence(observations: usize) -> f64 {
    match observations {
        0 | 1 => 0.25,
        2 => 0.4,
        _ => 0.65,
    }
}

fn outcome_strength(status: &OutcomeStatusV1) -> f64 {
    match status {
        OutcomeStatusV1::Success | OutcomeStatusV1::Failure => 0.9,
        OutcomeStatusV1::Partial => 0.65,
        OutcomeStatusV1::Unknown => 0.25,
    }
}

fn source_rank(source: &AssertionSourceTypeV1) -> u8 {
    match source {
        AssertionSourceTypeV1::Explicit => 2,
        AssertionSourceTypeV1::Inferred => 1,
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}
