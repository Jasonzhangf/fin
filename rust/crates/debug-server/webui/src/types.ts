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
  event_id?: string;
  sequence?: number;
  event_type?: string;
  occurred_at?: string;
  timestamp?: string;
  trace_id?: string;
  source?: string;
  operation_id?: string;
  refs?: JsonRecord;
  payload?: unknown;
}

export interface SessionMessage {
  message_id?: string;
  role?: string;
  content?: string;
  created_at?: string;
  session_id?: string;
  task_id?: string | null;
  operation_id?: string | null;
  trace_id?: string | null;
  closure_id?: string | null;
}

export interface ContextSnapshotRecord extends JsonRecord {
  operation_id?: string;
  trace_id?: string;
  captured_at?: string;
  input?: string;
  context?: JsonRecord;
}

export interface DigestRecord extends JsonRecord {
  digest_id?: string;
  closure_id?: string;
  created_at?: string;
}

export interface FocusTurn {
  operationId: string;
  traceId?: string | null;
  userMessage?: SessionMessage;
  assistantMessage?: SessionMessage;
  contextSnapshot?: ContextSnapshotRecord;
  digest?: DigestRecord;
  events: RuntimeEvent[];
}

export type DashboardCardId = 'provider' | 'context' | 'system' | 'operation';

export interface RefreshState {
  binding: DebugBinding | null;
  projection: ProjectionView;
  events: RuntimeEvent[];
  sessionEvents: RuntimeEvent[];
  lastRun: JsonRecord | null;
  currentContext: JsonRecord | null;
  recentContexts: ContextSnapshotRecord[];
  recentDigests: DigestRecord[];
  messages: SessionMessage[];
  focusTurns: FocusTurn[];
  selectedOperationId: string | null;
  openedCard: DashboardCardId | null;
}
