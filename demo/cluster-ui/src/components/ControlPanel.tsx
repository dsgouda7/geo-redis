import { useState } from 'react';
import { triggerSplit, triggerMerge } from '../hooks/useCluster';
import type { ClusterEvent, NodeInfo } from '../types';

interface Props {
  onEvent: (msg: string, kind: ClusterEvent['kind']) => void;
  reachable: boolean;
  nodes: NodeInfo[];
}

export default function ControlPanel({ onEvent, reachable, nodes = [] }: Props) {
  const [splitPt,  setSplitPt]  = useState('');
  const [mergeTarget, setMergeTarget] = useState('');
  const [splitting, setSplitting] = useState(false);
  const [merging,   setMerging]   = useState(false);

  // Auto-detect the best merge candidate: a node with a non-empty range that
  // is adjacent to node-0's range (its prefix_start matches node-0's prefix_end)
  const node0 = nodes.find(n => n.node_id === 'node-0');
  const mergeCandidates = nodes.filter(n =>
    n.node_id !== 'node-0' &&
    n.status !== 'dead' &&
    n.prefix_start !== '' &&   // not a pure standby with no range
    (node0 ? n.prefix_start === node0.prefix_end : true)
  );
  const autoMergeTarget = mergeTarget || mergeCandidates[0]?.addr || '';
  const canMerge = reachable && !merging && mergeCandidates.length > 0;

  const doSplit = async () => {
    setSplitting(true);
    try {
      const msg = await triggerSplit(splitPt || undefined);
      onEvent(msg, 'split');
    } catch (e) {
      onEvent(`Split failed: ${e}`, 'warn');
    } finally { setSplitting(false); }
  };

  const doMerge = async () => {
    if (!autoMergeTarget) { onEvent('No adjacent shard to merge', 'warn'); return; }
    setMerging(true);
    try {
      const msg = await triggerMerge(autoMergeTarget);
      onEvent(msg, 'ok');
    } catch (e) {
      onEvent(`Merge failed: ${e}`, 'warn');
    } finally { setMerging(false); }
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

      {/* Merge button */}
      <input
        placeholder={`absorb (${autoMergeTarget || 'no candidate'})`}
        value={mergeTarget}
        onChange={e => setMergeTarget(e.target.value)}
        disabled={!reachable}
        style={{
          background: '#0f172a', border: '1px solid #1e3a5f', borderRadius: 6,
          color: '#e2e8f0', padding: '5px 10px', fontSize: 11,
          width: 200, fontFamily: 'monospace',
        }}
      />
      <button
        onClick={doMerge}
        disabled={!canMerge || merging}
        title={canMerge ? `Absorb ${autoMergeTarget} into node-0` : 'No adjacent shard to merge'}
        style={{
          ...btnBase,
          background: merging ? '#1e3a5f' : '#06b6d4',
          color: '#0a1628',
          opacity: (!canMerge || merging) ? 0.5 : 1,
        }}
      >
        {merging ? 'Merging…' : 'Trigger Merge'}
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
