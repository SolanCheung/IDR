export type HostModelRequest = {
  task: string;
  context?: Record<string, unknown>;
};

export type HostModelResponse = {
  content: string;
};

export interface HostModelProvider {
  infer(request: HostModelRequest): Promise<HostModelResponse>;
}

export type IdrResolveInput = {
  intentCandidate?: unknown;
  request: HostModelRequest;
};

export async function resolveWithHostModel(
  provider: HostModelProvider,
  input: IdrResolveInput,
): Promise<HostModelResponse | undefined> {
  if (input.intentCandidate !== undefined) {
    return undefined;
  }

  return provider.infer(input.request);
}
