export type ProjectionView = Record<string, unknown>;
export type JsonRecord = Record<string, unknown>;
export type ConversationRichness = 'minimal' | 'rich' | 'full_trace';

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

export interface EventArchiveSegmentRef extends JsonRecord {
  segment?: string;
  relative_path?: string;
  event_count?: number;
}

export interface EventArchiveIndex extends JsonRecord {
  session_id?: string;
  live_stream_path?: string;
  local_archive_dir?: string;
  cold_archive_dir?: string;
  live_event_count?: number;
  local_archive_file_count?: number;
  cold_archive_file_count?: number;
  local_segments?: EventArchiveSegmentRef[];
  cold_segments?: EventArchiveSegmentRef[];
}

export type EventLedgerScope = 'live' | 'local_archive' | 'cold_archive';

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

export interface ReasoningViewRecord extends JsonRecord {
  reasoning_id?: string;
  operation_id?: string;
  trace_id?: string;
  created_at?: string;
  summary?: string;
}

export interface ToolExecutionRecord extends JsonRecord {
  tool_call_id?: string;
  operation_id?: string;
  trace_id?: string;
  tool_name?: string;
  status?: string;
  title?: string;
  purpose?: string;
  target_kind?: string;
  target_ref?: string;
  input_summary?: string;
  output_summary?: string;
  side_effects?: string[];
}

export interface ClosureTraceRecord extends JsonRecord {
  closure_id?: string;
  digest_id?: string;
  operation_id?: string;
  trace_id?: string;
  created_at?: string;
  assistant_response?: string;
}

export interface FocusTurn {
  operationId: string;
  traceId?: string | null;
  userMessage?: SessionMessage;
  assistantMessage?: SessionMessage;
  contextSnapshot?: ContextSnapshotRecord;
  digest?: DigestRecord;
  reasoningView?: ReasoningViewRecord;
  toolRecords: ToolExecutionRecord[];
  closureTrace?: ClosureTraceRecord;
  events: RuntimeEvent[];
}

export interface EventLedgerView {
  index: EventArchiveIndex | null;
  scope: EventLedgerScope;
  segment: string | null;
  events: RuntimeEvent[];
  selectedOperationId: string | null;
  liveTurn?: FocusTurn | null;
  matchedDigest?: DigestRecord | null;
  matchedReasoningView?: ReasoningViewRecord | null;
  matchedClosures?: ClosureTraceRecord[];
  matchedToolRecords?: ToolExecutionRecord[];
}

export type DashboardCardId = 'provider' | 'context' | 'system' | 'operation';

export interface RefreshState {
  binding: DebugBinding | null;
  projection: ProjectionView;
  events: RuntimeEvent[];
  sessionEvents: RuntimeEvent[];
  eventArchiveIndex: EventArchiveIndex | null;
  eventLedgerScope: EventLedgerScope;
  eventLedgerSegment: string | null;
  eventLedgerEvents: RuntimeEvent[];
  eventLedgerSelectedOperationId: string | null;
  lastRun: JsonRecord | null;
  currentContext: JsonRecord | null;
  recentContexts: ContextSnapshotRecord[];
  recentDigests: DigestRecord[];
  recentReasoningViews: ReasoningViewRecord[];
  recentToolRecords: ToolExecutionRecord[];
  recentClosures: ClosureTraceRecord[];
  messages: SessionMessage[];
  focusTurns: FocusTurn[];
  selectedOperationId: string | null;
  conversationRichness: ConversationRichness;
  openedCard: DashboardCardId | null;
  openedSectionKey: string | null;
}
