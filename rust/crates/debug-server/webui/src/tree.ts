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
    return `<div class="scalar-block">${this.escapeHtml(this.prettyScalar(value))}</div>`;
  }

  renderRow(key: string, value: unknown): string {
    if (value === null || value === undefined || typeof value !== 'object') {
      return `<div class="tree-row"><div class="tree-key">${this.escapeHtml(key)}</div><div class="tree-value">${this.renderScalar(value)}</div></div>`;
    }
    return `<div class="tree-row"><div class="tree-key">${this.escapeHtml(key)}</div><div class="tree-value">${this.renderNode(key, value, false)}</div></div>`;
  }

  renderNode(label: string, value: unknown, open = true): string {
    if (value === null || value === undefined || typeof value !== 'object') {
      return `<div class="tree-row"><div class="tree-key">${this.escapeHtml(label)}</div><div class="tree-value">${this.renderScalar(value)}</div></div>`;
    }

    const rows = Array.isArray(value)
      ? value.map((item, index) => this.renderRow(`[${index}]`, item)).join('')
      : Object.entries(value).map(([key, child]) => this.renderRow(key, child)).join('');
    const size = Array.isArray(value) ? value.length : Object.keys(value).length;
    const sizeLabel = Array.isArray(value) ? `[${size}]` : `{${size}}`;
    const emptyRow = `<div class="tree-row"><div class="tree-key">empty</div><div class="tree-value">${this.renderScalar('(empty)')}</div></div>`;

    return `
      <details class="tree-node" ${open ? 'open' : ''}>
        <summary>${this.escapeHtml(label)} <span class="muted">${sizeLabel}</span></summary>
        <div class="tree-body">${rows || emptyRow}</div>
      </details>
    `;
  }
}
