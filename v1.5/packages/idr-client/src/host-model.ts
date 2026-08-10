import { IdrClient } from "./client.ts";
import type {
  DecisionContractV1,
  HostModelRequestV1,
  HostModelResultV1,
  ResolveRequestV1,
  UnresolvedResultV1,
} from "./contracts.ts";

export interface HostModelProvider {
  infer(request: HostModelRequestV1): Promise<HostModelResultV1>;
}

export class IdrUnresolvedError extends Error {
  readonly unresolved: UnresolvedResultV1;

  constructor(unresolved: UnresolvedResultV1) {
    super(`IDR resolution failed closed: ${unresolved.reason}`);
    this.name = "IdrUnresolvedError";
    this.unresolved = unresolved;
  }
}

export async function resolveWithHostModel(
  idr: IdrClient,
  originalRequest: ResolveRequestV1,
  provider: HostModelProvider,
): Promise<DecisionContractV1> {
  const result = await idr.resolve(originalRequest);
  if (result.type === "decision") {
    return result.decision;
  }
  if (result.type === "unresolved") {
    throw new IdrUnresolvedError(result.unresolved);
  }

  const modelResult = await provider.infer(result.model_request);
  return idr.continueResolve({
    original_request: originalRequest,
    model_result: modelResult,
  });
}
