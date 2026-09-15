import type {
  ContinueResolveRequestV1,
  DecisionContractV1,
  FeedbackResultV1,
  OutcomeFeedbackV1,
  ResolveOutcomeV1,
  ResolveRequestV1,
} from "./contracts.ts";

export type FetchLike = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>;

export class IdrClient {
  readonly #baseUrl: string;
  readonly #fetch: FetchLike;

  constructor(baseUrl: string, fetchImpl: FetchLike = globalThis.fetch) {
    this.#baseUrl = baseUrl.replace(/\/$/, "");
    this.#fetch = fetchImpl;
  }

  resolve(request: ResolveRequestV1): Promise<ResolveOutcomeV1> {
    return this.#post("/v1/resolve", request);
  }

  async continueResolve(
    request: ContinueResolveRequestV1,
  ): Promise<DecisionContractV1> {
    const result = await this.#post<ResolveOutcomeV1>(
      "/v1/resolve/continue",
      request,
    );
    if (result.type !== "decision") {
      throw new Error("IDR continuation did not return a decision");
    }
    return result.decision;
  }

  feedback(request: OutcomeFeedbackV1): Promise<FeedbackResultV1> {
    return this.#post("/v1/feedback", request);
  }

  async #post<T>(path: string, body: unknown): Promise<T> {
    const response = await this.#fetch(`${this.#baseUrl}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!response.ok) {
      throw new Error(`IDR request failed with status ${response.status}`);
    }
    return response.json() as Promise<T>;
  }
}

