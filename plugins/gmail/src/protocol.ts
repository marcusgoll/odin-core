export type RiskTier = "safe" | "sensitive" | "destructive";

export interface EventEnvelope {
  event_id: string;
  event_type: string;
  task_id?: string;
  request_id?: string;
  project?: string;
  payload: Record<string, unknown>;
}

export interface RequestCapabilityDirective {
  action: "request_capability";
  capability: { id: string; project?: string };
  reason?: string;
  input?: unknown;
  risk_tier?: RiskTier;
}

export interface EnqueueTaskDirective {
  action: "enqueue_task";
  task_type: string;
  project?: string;
  reason?: string;
  payload?: unknown;
}

export interface NoopDirective {
  action: "noop";
}

export type PluginDirective =
  | RequestCapabilityDirective
  | EnqueueTaskDirective
  | NoopDirective;
