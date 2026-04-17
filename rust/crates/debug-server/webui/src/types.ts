export type InspectorTab =
  | 'overview'
  | 'provider'
  | 'context'
  | 'history'
  | 'events'
  | 'last-run'
  | 'alerts';

export type ProjectionView = Record<string, unknown>;
export type JsonRecord = Record<string, unknown>;

export interface DebugBinding {
  project_id: string;
  project_label: string;
  runtime_home: string;
  session_id?: string | null;
  task_id?: string | null;
  session_messages_path?: string | null;
  recent_contexts_path?: string | null;
  recent_digests_path?: string | null;
}

export interface DebugSnapshot {
  projection: ProjectionView;
  events: RuntimeEvent[];
}

export interface RuntimeEvent {
  sequence?: number;
  event_type?: string;
  occurred_at?: string;
  timestamp?: string;
  trace_id?: string;
  source?: string;
  refs?: JsonRecord;
  payload?: unknown;
}

export interface SessionMessage {
  message_id?: string;
  role?: string;
  content?: string;
  created_at?: string;
}

export interface RefreshState {
  binding: DebugBinding | null;
  projection: ProjectionView;
  events: RuntimeEvent[];
  lastRun: JsonRecord | null;
  currentContext: JsonRecord | null;
  recentContexts: JsonRecord[];
  messages: SessionMessage[];
}
