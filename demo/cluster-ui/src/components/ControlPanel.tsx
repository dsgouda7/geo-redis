import { useState } from 'react';
import { triggerSplit } from '../hooks/useCluster';
import type { ClusterEvent } from '../types';

interface Props {
  onEvent: (msg: string, kind: ClusterEvent['kind']) => void;
  reachable: boolean;
}

export default function ControlPanel({ onEvent, reachable }: Props) {
  const [splitPt,   setSplitPt]   = useState('');
  const [splitting, setSplitting] = useState(false);

  const doSplit = async () => {
    setSplitting(true);
    try {
      const msg = await triggerSplit(splitPt || undefined);
      onEvent(msg, 'split');
    } catch (e) {
      onEvent(`Split failed: ${e}`, 'warn');
    } finally { setSplitting(false); }
  };

  const btnBase: React.CSSProperties = {
    padding: '6px 14px',
    borderRadius: 6,
    border: 'none',
    cursor: 'pointer',
    fontSize: 12,
    fontWeight: 600,
    transition: 'opacity 0.15s',
  };

  return (
    <div style={{
      display: 'flex', gap: 10, alignItems: 'center',
      padding: '8px 12px',
      background: '#0a1628',
      borderRadius: 8,
      border: '1px solid #1e3a5f',
    }}>
      <div style={{
        width: 8, height: 8, borderRadius: '50%',
        background: reachable ? '#22c55e' : '#ef4444',
        boxShadow: reachable ? '0 0 6px #22c55e' : undefined,
        flexShrink: 0,
      }} />
      <span style={{ color: '#64748b', fontSize: 11, marginRight: 8 }}>
        {reachable ? 'Cluster connected' : 'Cluster unreachable — start demo-cluster first'}
      </span>

      <input
        placeholder="split point (auto)"
        value={splitPt}
        onChange={e => setSplitPt(e.target.value)}
        disabled={!reachable}
        style={{
          background: '#0f172a', border: '1px solid #1e3a5f', borderRadius: 6,
          color: '#e2e8f0', padding: '5px 10px', fontSize: 11,
          width: 160, fontFamily: 'monospace',
        }}
      />
      <button
        onClick={doSplit}
        disabled={!reachable || splitting}
        style={{
          ...btnBase,
          background: splitting ? '#1e3a5f' : '#eab308',
          color: '#0a1628',
          opacity: (!reachable || splitting) ? 0.5 : 1,
        }}
      >
        {splitting ? 'Splitting…' : 'Trigger Split'}
      </button>

      <a
        href="http://localhost:4000/cluster"
        target="_blank" rel="noopener noreferrer"
        style={{ ...btnBase, background: '#1e3a5f', color: '#94a3b8', textDecoration: 'none' }}
      >
        Raw ring ↗
      </a>
    </div>
  );
}
