import { useMemo } from 'react';
import type { NodeInfo } from '../types';
import { NODE_POSITIONS, STATUS_COLOR } from '../types';

interface Props {
  nodes: NodeInfo[];
}

// Stable node order for positions
const ID_ORDER = ['node-0', 'node-1', 'node-2', 'node-3'];
const FALLBACK_POS = { x: 380, y: 230 };

function pos(id: string) { return NODE_POSITIONS[id] ?? FALLBACK_POS; }

export default function Topology({ nodes }: Props) {
  const nodeMap = useMemo(
    () => new Map(nodes.map(n => [n.node_id, n])),
    [nodes]
  );

  // Gossip edges: connect every pair of nodes that are not dead/standby
  const edges = useMemo(() => {
    const active = nodes.filter(n => n.status !== 'dead' && n.status !== 'standby');
    const pairs: [string, string][] = [];
    for (let i = 0; i < active.length; i++)
      for (let j = i + 1; j < active.length; j++)
        pairs.push([active[i].node_id, active[j].node_id]);
    return pairs;
  }, [nodes]);

  // Animated flow edges: splitting → target (node-3), bootstrapping ← source (node-0)
  const flowEdges = useMemo(() => {
    const flows: { from: string; to: string; kind: 'split' | 'bootstrap' | 'delta' }[] = [];
    nodes.forEach(n => {
      if (n.status === 'splitting')     flows.push({ from: n.node_id, to: 'node-3', kind: 'split' });
      if (n.status === 'bootstrapping') flows.push({ from: 'node-0', to: n.node_id, kind: 'delta' });
    });
    return flows;
  }, [nodes]);

  const W = 760, H = 480;

  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" style={{ maxHeight: 420 }}>
      <defs>
        {/* Animated gradient for flow arrows */}
        <marker id="arrow-split"    markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
          <polygon points="0 0, 8 3, 0 6" fill="#eab308" />
        </marker>
        <marker id="arrow-delta"    markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
          <polygon points="0 0, 8 3, 0 6" fill="#3b82f6" />
        </marker>
        <marker id="arrow-gossip"   markerWidth="6" markerHeight="5" refX="6" refY="2.5" orient="auto">
          <polygon points="0 0, 6 2.5, 0 5" fill="#334155" />
        </marker>
        <filter id="glow">
          <feGaussianBlur stdDeviation="3" result="blur" />
          <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
      </defs>

      {/* Background grid */}
      <rect width={W} height={H} fill="#0a1628" rx="12" />
      <text x={W / 2} y={22} textAnchor="middle" fill="#1e3a5f" fontSize={11} fontFamily="monospace">
        S2 token ring — geographic shard distribution
      </text>

      {/* S2 ring arc hint */}
      <ellipse cx={W / 2} cy={240} rx={290} ry={180}
        fill="none" stroke="#0f2d4a" strokeWidth={1} strokeDasharray="4 8" />

      {/* Gossip edges */}
      {edges.map(([a, b]) => {
        const pa = pos(a), pb = pos(b);
        return (
          <line key={`${a}-${b}`}
            x1={pa.x} y1={pa.y} x2={pb.x} y2={pb.y}
            stroke="#1e3a5f" strokeWidth={1.5} strokeDasharray="4 6"
            markerEnd="url(#arrow-gossip)" opacity={0.6} />
        );
      })}

      {/* Animated flow arrows */}
      {flowEdges.map(({ from, to, kind }, i) => {
        const pf = pos(from), pt = pos(to);
        const color = kind === 'split' ? '#eab308' : '#3b82f6';
        const mid = { x: (pf.x + pt.x) / 2, y: (pf.y + pt.y) / 2 };
        const curve = `M${pf.x},${pf.y} Q${mid.x - 40},${mid.y - 40} ${pt.x},${pt.y}`;
        return (
          <g key={`flow-${i}`}>
            {/* Static path */}
            <path d={curve} fill="none" stroke={color} strokeWidth={2}
              strokeDasharray="8 6" opacity={0.5}
              markerEnd={`url(#arrow-${kind === 'split' ? 'split' : 'delta'})`} />
            {/* Animated pulse dot */}
            <circle r={5} fill={color} filter="url(#glow)" opacity={0.9}>
              <animateMotion dur={`${1.4 + i * 0.3}s`} repeatCount="indefinite">
                <mpath href={`#flow-path-${i}`} />
              </animateMotion>
            </circle>
            <path id={`flow-path-${i}`} d={curve} fill="none" />
            {/* Label */}
            <text x={mid.x + 10} y={mid.y - 10} fill={color} fontSize={10}
              fontFamily="monospace" fontWeight="600">
              {kind === 'split' ? 'migrating' : 'Δ sync'}
            </text>
          </g>
        );
      })}

      {/* Nodes */}
      {ID_ORDER.map(id => {
        const n   = nodeMap.get(id);
        const { x, y } = pos(id);
        const col = n ? STATUS_COLOR[n.status] : '#334155';
        const label = id.replace('node-', 'N');
        const name = n?.node_id ?? id;
        const pfx  = n ? `[${n.prefix_start || '∅'}, ${n.prefix_end || '∅'})` : '—';
        const keys = n ? n.key_count.toLocaleString() : '—';
        const pulsing = n && (n.status === 'splitting' || n.status === 'bootstrapping');

        return (
          <g key={id} transform={`translate(${x},${y})`}>
            {/* Pulse ring for active transitions */}
            {pulsing && (
              <circle r={46} fill="none" stroke={col} strokeWidth={1.5} opacity={0.4}>
                <animate attributeName="r" from="38" to="60" dur="1.5s" repeatCount="indefinite" />
                <animate attributeName="opacity" from="0.5" to="0" dur="1.5s" repeatCount="indefinite" />
              </circle>
            )}
            {/* Shadow */}
            <circle r={40} fill="#0a1628" />
            {/* Node circle */}
            <circle r={38} fill={col + '22'} stroke={col} strokeWidth={2.5} />
            {/* Short id */}
            <text y={-8} textAnchor="middle" fill={col} fontSize={15} fontWeight="700"
              fontFamily="monospace">{label}</text>
            {/* Status */}
            <text y={8} textAnchor="middle" fill={col} fontSize={9} fontFamily="monospace">
              {n?.status ?? 'unknown'}
            </text>
            {/* Key count */}
            <text y={22} textAnchor="middle" fill="#94a3b8" fontSize={9} fontFamily="monospace">
              {keys} keys
            </text>
            {/* Node name below circle */}
            <text y={54} textAnchor="middle" fill="#64748b" fontSize={10}>{name}</text>
            {/* Prefix range */}
            <text y={67} textAnchor="middle" fill="#334155" fontSize={9} fontFamily="monospace">
              {pfx}
            </text>
          </g>
        );
      })}

      {/* Legend */}
      {(['active','splitting','bootstrapping','standby'] as const).map((s, i) => (
        <g key={s} transform={`translate(${16 + i * 130}, ${H - 22})`}>
          <circle cx={6} cy={0} r={5} fill={STATUS_COLOR[s]} />
          <text x={14} y={4} fill="#64748b" fontSize={10}>{s}</text>
        </g>
      ))}
    </svg>
  );
}
