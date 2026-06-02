import createClient from "openapi-fetch";
import type { components, paths } from "./generated/api-types";

export const api = createClient<paths>({
  baseUrl: import.meta.env.VITE_API_BASE_URL || "",
  fetch: (request: Request) => fetch(request, { credentials: "include" }),
});

export type Course = components["schemas"]["Course"];
export type CourseOffering = components["schemas"]["CourseOffering"];
export type SectionDetail = components["schemas"]["SectionDetail"];
export type SectionSummary = components["schemas"]["SectionSummary"];
export type HistoryItem = components["schemas"]["HistoryItem"];
export type ReviewResponse = components["schemas"]["ReviewResponse"];
export type User = components["schemas"]["User"];

export class ApiClientError extends Error {
  status: number;
  payload: unknown;

  constructor(status: number, payload: unknown) {
    super(readErrorMessage(status, payload));
    this.status = status;
    this.payload = payload;
  }
}

export function unwrap<T>(result: {
  data?: T;
  error?: unknown;
  response: Response;
}): T {
  if (result.error) {
    throw new ApiClientError(result.response.status, result.error);
  }
  if (result.data === undefined) {
    throw new ApiClientError(result.response.status, {
      error: { message: "Empty API response" },
    });
  }
  return result.data;
}

export function errorMessage(error: unknown): string {
  if (error instanceof ApiClientError) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "Unexpected error";
}

function readErrorMessage(status: number, payload: unknown): string {
  if (
    payload &&
    typeof payload === "object" &&
    "error" in payload &&
    payload.error &&
    typeof payload.error === "object" &&
    "message" in payload.error &&
    typeof payload.error.message === "string"
  ) {
    return payload.error.message;
  }
  return `Request failed with HTTP ${status}`;
}
