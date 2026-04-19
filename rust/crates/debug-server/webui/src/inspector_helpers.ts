import type { JsonRecord, RuntimeEvent } from './types.js';

export function findEvent(events: RuntimeEvent[], eventType: string): RuntimeEvent | undefined {
  return events.find((event) => event.event_type === eventType);
}

export function asRecord(value: unknown): JsonRecord {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return value as JsonRecord;
}

export function scalar(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'string') return value || '-';
  return JSON.stringify(value);
}

export function shortText(value: string, limit: number): string {
  const text = value.trim();
  if (!text) return '-';
  return text.length <= limit ? text : `${text.slice(0, limit)}…`;
}

export function shortPath(value: unknown): string {
  const text = scalar(value);
  if (text === '-') return text;
  const parts = text.split('/').filter(Boolean);
  return parts.length <= 3 ? text : `…/${parts.slice(-3).join('/')}`;
}

export function promptLayerById(layers: JsonRecord[], layerId: string): JsonRecord {
  return layers.find((layer) => scalar(layer.layer_id) === layerId) ?? {};
}

export function modulesForLayer(modules: JsonRecord[], layer: JsonRecord): JsonRecord[] {
  const moduleIds = Array.isArray(layer.module_ids)
    ? layer.module_ids.map((item) => String(item))
    : [];
  if (!moduleIds.length) return [];
  const order = new Map(moduleIds.map((moduleId, index) => [moduleId, index]));
  return modules
    .filter((module) => order.has(scalar(module.module_id)))
    .sort((left, right) => {
      const leftIndex = order.get(scalar(left.module_id)) ?? 0;
      const rightIndex = order.get(scalar(right.module_id)) ?? 0;
      return leftIndex - rightIndex;
    });
}

export function layerDigest(layer: JsonRecord, modules: JsonRecord[]): string {
  if (!countKeys(layer)) return 'inactive';
  const title = scalar(layer.title);
  const summary = scalar(layer.summary);
  return `${title} · modules=${modules.length} · ${summary}`;
}

export function countKeys(value: JsonRecord): number {
  return Object.keys(value).length;
}

export function arrayCount(value: unknown): number {
  return Array.isArray(value) ? value.length : 0;
}

export function firstArrayItem(value: unknown): unknown {
  return Array.isArray(value) && value.length ? value[0] : undefined;
}

export function toolCount(value: JsonRecord): number {
  return arrayCount(value.model_tools) + arrayCount(value.framework_tools);
}

export function timelineLevel(eventType?: string): string {
  if (!eventType) return 'neutral';
  if (eventType.includes('failed') || eventType.includes('timeout')) return 'error';
  if (eventType.includes('completed') || eventType.includes('finalized')) return 'ok';
  return 'neutral';
}
