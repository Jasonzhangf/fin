type JsonRecord = Record<string, unknown>;

export class StructuredTreeRenderer {
  escapeHtml(value: unknown): string {
    return String(value)
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&#39;');
  }

  prettyScalar(value: unknown): string {
    if (value === null || value === undefined) return '-';
    if (typeof value === 'string') return value.trim().length ? value : '(empty string)';
    if (typeof value === 'number' || typeof value === 'boolean') return String(value);
    return JSON.stringify(value);
  }

  renderScalar(value: unknown): string {
    return `<span class="value-pill ${this.scalarTone(value)}">${this.escapeHtml(this.prettyScalar(value))}</span>`;
  }

  renderNode(label: string, value: unknown, open = true): string {
    if (this.isScalar(value)) {
      return this.renderScalarRow(label, value);
    }

    const size = Array.isArray(value) ? value.length : Object.keys(this.asRecord(value)).length;
    const sizeLabel = Array.isArray(value) ? `[${size}]` : `{${size}}`;

    return `
      <details class="tree-node" ${open ? 'open' : ''}>
        <summary>${this.escapeHtml(this.humanizeKey(label))} <span class="muted">${sizeLabel}</span></summary>
        <div class="tree-body">${this.renderBody(value)}</div>
      </details>
    `;
  }

  renderPanelValue(value: unknown): string {
    if (this.isScalar(value)) {
      return `
        <div class="kv-table">
          <div class="kv-row">
            <div class="kv-key">value</div>
            <div class="kv-value">${this.renderScalar(value)}</div>
          </div>
        </div>
      `;
    }

    return this.renderBody(value);
  }

  private renderBody(value: unknown): string {
    if (Array.isArray(value)) return this.renderArrayBody(value);
    return this.renderObjectBody(this.asRecord(value));
  }

  private renderObjectBody(value: JsonRecord): string {
    const entries = Object.entries(value);
    if (!entries.length) return this.renderEmpty('(empty object)');

    const scalarEntries = entries.filter(([, item]) => this.isScalarLike(item));
    const nestedEntries = entries.filter(([, item]) => !this.isScalarLike(item));

    return `
      ${scalarEntries.length ? `<div class="kv-table">${scalarEntries.map(([key, item]) => this.renderFieldRow(key, item)).join('')}</div>` : ''}
      ${nestedEntries.length ? `<div class="nested-stack">${nestedEntries.map(([key, item]) => this.renderNestedSection(key, item)).join('')}</div>` : ''}
    `;
  }

  private renderArrayBody(items: unknown[]): string {
    if (!items.length) return this.renderEmpty('(empty array)');

    if (items.every((item) => this.isScalar(item))) {
      return `<div class="pill-row">${items.map((item) => this.renderScalar(item)).join('')}</div>`;
    }

    if (items.every((item) => this.isScalarLike(item))) {
      return `<div class="kv-table">${items.map((item, index) => this.renderFieldRow(`[${index}]`, item)).join('')}</div>`;
    }

    return `
      <div class="object-card-list">
        ${items.map((item, index) => this.renderArrayItemCard(item, index)).join('')}
      </div>
    `;
  }

  private renderArrayItemCard(item: unknown, index: number): string {
    if (this.isScalarLike(item)) {
      return `
        <section class="object-card">
          <header class="object-card-header">
            <span class="object-card-title">${this.escapeHtml(`[${index}]`)}</span>
          </header>
          <div class="object-card-body">
            <div class="kv-table">${this.renderFieldRow('value', item)}</div>
          </div>
        </section>
      `;
    }

    const record = this.asRecord(item);
    const title = this.arrayCardTitle(record, index);
    const entries = Object.entries(record);
    const scalarEntries = entries.filter(([, child]) => this.isScalarLike(child));
    const nestedEntries = entries.filter(([, child]) => !this.isScalarLike(child));

    return `
      <section class="object-card">
        <header class="object-card-header">
          <span class="object-card-title">${this.escapeHtml(title)}</span>
        </header>
        <div class="object-card-body">
          ${scalarEntries.length ? `<div class="kv-table">${scalarEntries.map(([key, child]) => this.renderFieldRow(key, child)).join('')}</div>` : ''}
          ${nestedEntries.length ? `<div class="nested-stack">${nestedEntries.map(([key, child]) => this.renderNestedSection(key, child)).join('')}</div>` : ''}
        </div>
      </section>
    `;
  }

  private renderNestedSection(key: string, value: unknown): string {
    if (Array.isArray(value) && value.every((item) => this.isScalar(item))) {
      return `
        <section class="nested-section">
          <div class="nested-section-title">${this.escapeHtml(this.humanizeKey(key))}</div>
          <div class="pill-row">${value.map((item) => this.renderScalar(item)).join('')}</div>
        </section>
      `;
    }

    return `
      <section class="nested-section">
        <div class="nested-section-title">${this.escapeHtml(this.humanizeKey(key))}</div>
        ${this.renderNode(key, value, true)}
      </section>
    `;
  }

  private renderFieldRow(key: string, value: unknown): string {
    if (Array.isArray(value) && value.every((item) => this.isScalar(item))) {
      return `
        <div class="kv-row wide">
          <div class="kv-key">${this.escapeHtml(this.humanizeKey(key))}</div>
          <div class="kv-value">
            <div class="pill-row">${value.map((item) => this.renderScalar(item)).join('')}</div>
          </div>
        </div>
      `;
    }

    if (typeof value === 'string' && value.length > 120) {
      return `
        <div class="kv-row wide">
          <div class="kv-key">${this.escapeHtml(this.humanizeKey(key))}</div>
          <div class="kv-value block">${this.renderTextBlock(value)}</div>
        </div>
      `;
    }

    return `
      <div class="kv-row">
        <div class="kv-key">${this.escapeHtml(this.humanizeKey(key))}</div>
        <div class="kv-value">${this.renderScalar(value)}</div>
      </div>
    `;
  }

  private renderScalarRow(label: string, value: unknown): string {
    return `
      <div class="kv-table">
        <div class="kv-row">
          <div class="kv-key">${this.escapeHtml(this.humanizeKey(label))}</div>
          <div class="kv-value">${this.renderScalar(value)}</div>
        </div>
      </div>
    `;
  }

  private renderTextBlock(value: string): string {
    return `<div class="text-block">${this.escapeHtml(value)}</div>`;
  }

  private renderEmpty(text: string): string {
    return `<div class="empty-state compact"><div class="empty-text">${this.escapeHtml(text)}</div></div>`;
  }

  private scalarTone(value: unknown): string {
    const text = this.prettyScalar(value).toLowerCase();
    if (value === null || value === undefined || text === '-') return 'muted';
    if (typeof value === 'number') {
      if (value >= 15) return 'good';
      if (value >= 8) return 'info';
      return 'warn';
    }
    if (typeof value === 'boolean') return value ? 'good' : 'muted';
    if (text.includes('error') || text.includes('failed') || text.includes('timeout')) return 'error';
    if (text.includes('completed') || text.includes('active') || text.includes('healthy') || text === 'true' || text === 'good') return 'good';
    if (text.includes('warning') || text.includes('pending') || text.includes('inactive')) return 'warn';
    if (text.length <= 18) return 'info';
    return 'plain';
  }

  private arrayCardTitle(record: JsonRecord, index: number): string {
    const preferredKeys = ['title', 'label', 'module_id', 'layer_id', 'tool_name', 'event_type', 'project_id', 'role_id'];
    for (const key of preferredKeys) {
      const value = record[key];
      if (typeof value === 'string' && value.trim()) {
        return `${value} · [${index}]`;
      }
    }
    return `[${index}]`;
  }

  private humanizeKey(key: string): string {
    return key
      .replaceAll(/[_-]+/g, ' ')
      .replaceAll(/\s+/g, ' ')
      .trim();
  }

  private isScalar(value: unknown): value is string | number | boolean | null | undefined {
    return value === null || value === undefined || ['string', 'number', 'boolean'].includes(typeof value);
  }

  private isScalarLike(value: unknown): boolean {
    return this.isScalar(value) || (Array.isArray(value) && value.every((item) => this.isScalar(item)));
  }

  private asRecord(value: unknown): JsonRecord {
    if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
    return value as JsonRecord;
  }
}
