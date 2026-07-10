import type { ClusterEvent } from '../types';

const KIND_COLOR: Record<ClusterEvent['kind'], string> = {
  info:      '#64748b',
  split:     '#eab308',
  bootstrap: '#3b82f6',
  warn:      '#f97316',
  ok:        '#22c55e',
};

const KIND_ICON: Record<ClusterEvent['kind'], string> = {
  info:      '○',
  split:     '⟿',
  bootstrap: '↻',
  warn:      '⚠',
  ok:        '✓',
};

interface Props { events: ClusterEvent[] }

export default function EventLog({ events }: Props) {
  const shown = [...events].reverse();

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 0 }}>
      {shown.length === 0 && (
        <div style={{ color: '#334155', fontSize: 12, padding: '8px 0' }}>
          Waiting for cluster activity…
        </div>
      )}
      {shown.map((e, i) => (
        <div key={i} style={{
          display: 'flex', gap: 8, alignItems: 'baseline',
          padding: '4px 6px',
          background: i === 0 ? KIND_COLOR[e.kind] + '11' : 'transparent',
          borderRadius: 4,
          borderLeft: i === 0 ? `2px solid ${KIND_COLOR[e.kind]}` : '2px solid transparent',
          transition: 'background 0.3s',
        }}>
          <span style={{ color: KIND_COLOR[e.kind], fontSize: 12, flexShrink: 0 }}>
            {KIND_ICON[e.kind]}
          </span>
          <span style={{ color: '#475569', fontSize: 10, fontFamily: 'monospace', flexShrink: 0 }}>
            {new Date(e.ts).toLocaleTimeString('en', { hour12: false })}
          </span>
          <span style={{ color: i === 0 ? '#e2e8f0' : '#94a3b8', fontSize: 11, lineHeight: 1.5 }}>
            {e.message}
          </span>
        </div>
      ))}
    </div>
  );
}
