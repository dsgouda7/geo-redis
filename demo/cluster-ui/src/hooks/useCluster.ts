import { useEffect, useRef, useState, useCallback } from 'react';
import type { ClusterSnapshot, ClusterEvent, NodeInfo, ThroughputPoint } from '../types';

const NODES = ['http://localhost:4000','http://localhost:4001',
               'http://localhost:4002','http://localhost:4003'];
const MAX_HISTORY  = 90;   // 90 s rolling window
const POLL_INTERVAL = 2000;

async function fetchRing(): Promise<NodeInfo[]> {
  for (const base of NODES) {
    try {
      const r = await fetch(`${base}/cluster`, { signal: AbortSignal.timeout(2000) });
      if (r.ok) return r.json();
    } catch {}
  }
  return [];
}

export function useCluster() {
  const [snapshots,   setSnapshots]   = useState<ClusterSnapshot[]>([]);
  const [throughput,  setThroughput]  = useState<ThroughputPoint[]>([]);
  const [events,      setEvents]      = useState<ClusterEvent[]>([]);
  const [reachable,   setReachable]   = useState(false);
  const prevNodesRef = useRef<Map<string, NodeInfo>>(new Map());

  const addEvent = useCallback((message: string, kind: ClusterEvent['kind'] = 'info') => {
    setEvents(prev => {
      const entry: ClusterEvent = { ts: Date.now(), message, kind };
      return [...prev.slice(-49), entry];
    });
  }, []);

  useEffect(() => {
    let cancelled = false;

    const poll = async () => {
      const nodes = await fetchRing();
      if (cancelled) return;

      const now = Date.now();
      setReachable(nodes.length > 0);

      if (nodes.length === 0) {
        addEvent('Cluster unreachable — waiting for nodes on :4000–:4003', 'warn');
        return;
      }

      // Detect status changes and emit events
      const prevMap = prevNodesRef.current;
      nodes.forEach(n => {
        const prev = prevMap.get(n.node_id);
        if (prev && prev.status !== n.status) {
          const kind: ClusterEvent['kind'] =
            n.status === 'splitting'     ? 'split' :
            n.status === 'bootstrapping' ? 'bootstrap' :
            n.status === 'active'        ? 'ok' :
            n.status === 'dead'          ? 'warn' : 'info';
          addEvent(`${n.node_id}: ${prev.status} → ${n.status}`, kind);
        }
        prevMap.set(n.node_id, n);
      });
      prevNodesRef.current = new Map(nodes.map(n => [n.node_id, n]));

      setSnapshots(prev => {
        const next = [...prev.slice(-(MAX_HISTORY - 1)), { ts: now, nodes }];
        return next;
      });

      // Throughput: sum of active nodes' key counts
      const total = nodes.filter(n => n.status === 'active' || n.status === 'splitting')
                         .reduce((s, n) => s + n.key_count, 0);
      setThroughput(prev => {
        const last  = prev[prev.length - 1];
        const delta = last ? Math.max(0, total - last.total) : 0;
        return [...prev.slice(-(MAX_HISTORY - 1)), { ts: now, total, delta }];
      });
    };

    poll();
    const id = setInterval(poll, POLL_INTERVAL);
    return () => { cancelled = true; clearInterval(id); };
  }, [addEvent]);

  const current = snapshots[snapshots.length - 1] ?? null;
  return { current, snapshots, throughput, events, reachable };
}

// ── Control helpers (trigger split from the UI) ────────────────────────────
export async function triggerSplit(splitPoint?: string): Promise<string> {
  const body = splitPoint
    ? JSON.stringify({ target: 'geo-node-3:4003', split_point: splitPoint })
    : JSON.stringify({ target: 'geo-node-3:4003' });
  const r = await fetch('http://localhost:4000/split', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body,
  });
  const data = await r.json();
  return `Split at '${data.split_point}' — migrated ${data.migrated_keys?.toLocaleString()} keys`;
}
