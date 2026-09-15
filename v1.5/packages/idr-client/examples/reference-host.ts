import {
  IdrClient,
  resolveWithHostModel,
  type HostModelProvider,
  type ResolveRequestV1,
} from "../src/index.ts";

export async function resolveForAgent(
  idr: IdrClient,
  existingAgentLlm: HostModelProvider,
  input: ResolveRequestV1,
) {
  const decision = await resolveWithHostModel(idr, input, existingAgentLlm);
  return decision.recommended_action;
}

