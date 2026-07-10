export type NodeStatus =
  | 'active' | 'splitting' | 'merging' | 'bootstrapping'
  | 'suspect' | 'dead'     | 'standby';

export interface NodeInfo {
  node_id:        string;
  addr:           string;
  prefix_start:   string;
  prefix_end:     string;
  key_count:      number;
  mem_bytes:      number;
  status:         NodeStatus;
  last_seen_secs: number;
  generation:     number;
}

export interface ClusterSnapshot {
  ts:    number;            // Date.now()
  nodes: NodeInfo[];
}

export interface ThroughputPoint {
  ts:     number;
  total:  number;          // total keys across all active shards
  delta:  number;          // keys added since last snapshot
}

export interface ClusterEvent {
  ts:      number;
  message: string;
  kind:    'info' | 'split' | 'bootstrap' | 'warn' | 'ok';
}

// Fixed display layout for 4-node clusters
export const NODE_POSITIONS: Record<string, { x: number; y: number }> = {
  'node-0': { x: 130, y: 230 },
  'node-1': { x: 380, y: 90  },
  'node-2': { x: 630, y: 230 },
  'node-3': { x: 380, y: 370 },
};

export const STATUS_COLOR: Record<NodeStatus, string> = {
  active:        '#22c55e',
  splitting:     '#eab308',
  merging:       '#06b6d4',
  bootstrapping: '#3b82f6',
  suspect:       '#f97316',
  dead:          '#ef4444',
  standby:       '#64748b',
};
