import { IdrClient } from "./client.ts";
import type {
  DecisionContractV1,
  HostModelRequestV1,
  HostModelResultV1,
  ResolveRequestV1,
} from "./contracts.ts";

export interface HostModelProvider {
  infer(request: HostModelRequestV1): Promise<HostModelResultV1>;
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

  const modelResult = await provider.infer(result.model_request);
  return idr.continueResolve({
    original_request: originalRequest,
    model_result: modelResult,
  });
}

