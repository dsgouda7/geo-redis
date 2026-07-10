import {
  LineChart, Line, BarChart, Bar, XAxis, YAxis, CartesianGrid,
  Tooltip, ResponsiveContainer, Legend,
} from 'recharts';
import type { ThroughputPoint, ClusterSnapshot } from '../types';
import { STATUS_COLOR } from '../types';

// ── Throughput chart ───────────────────────────────────────────────────────

interface ThroughputProps { data: ThroughputPoint[] }

export function ThroughputChart({ data }: ThroughputProps) {
  const windowed = data.slice(-45);
  const formatted = windowed.map(d => ({
    t:     new Date(d.ts).toLocaleTimeString('en', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }),
    total: d.total,
    rate:  d.delta,   // keys added per 2-second interval ≈ writes/s × 2
  }));

  return (
    <div style={{ width: '100%', height: 200 }}>
      <div style={{ color: '#94a3b8', fontSize: 11, marginBottom: 6 }}>
        Total keys &amp; write rate (2-second intervals)
      </div>
      <ResponsiveContainer>
        <LineChart data={formatted} margin={{ left: 0, right: 8, top: 4, bottom: 0 }}>
          <CartesianGrid stroke="#1e3a5f" strokeDasharray="3 6" />
          <XAxis dataKey="t" tick={{ fill: '#475569', fontSize: 9 }} interval="preserveStartEnd" />
          <YAxis yAxisId="total" orientation="left"
            tick={{ fill: '#475569', fontSize: 9 }}
            tickFormatter={(v: number) => v >= 1000 ? `${(v/1000).toFixed(0)}k` : String(v)} />
          <YAxis yAxisId="rate" orientation="right"
            tick={{ fill: '#475569', fontSize: 9 }} />
          <Tooltip
            contentStyle={{ background: '#0f172a', border: '1px solid #1e3a5f', borderRadius: 6 }}
            labelStyle={{ color: '#94a3b8', fontSize: 11 }}
            itemStyle={{ fontSize: 11 }} />
          <Line yAxisId="total" dataKey="total" name="Total keys"
            stroke="#38bdf8" dot={false} strokeWidth={2} />
          <Line yAxisId="rate"  dataKey="rate"  name="New keys/interval"
            stroke="#22c55e" dot={false} strokeWidth={1.5} strokeDasharray="4 3" />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}

// ── Key distribution bar chart ─────────────────────────────────────────────

interface DistProps { snapshot: ClusterSnapshot | null }

export function KeyDistribution({ snapshot }: DistProps) {
  if (!snapshot) return null;

  const data = snapshot.nodes
    .filter(n => n.status !== 'dead')
    .map(n => ({
      id:    n.node_id,
      keys:  n.key_count,
      fill:  STATUS_COLOR[n.status],
    }))
    .sort((a, b) => b.keys - a.keys);

  const maxKeys = Math.max(...data.map(d => d.keys), 1);

  return (
    <div>
      <div style={{ color: '#94a3b8', fontSize: 11, marginBottom: 10 }}>
        Key distribution per shard
      </div>
      {data.map(({ id, keys, fill }) => (
        <div key={id} style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
          <div style={{ width: 56, fontSize: 10, color: '#64748b', fontFamily: 'monospace', flexShrink: 0 }}>
            {id}
          </div>
          <div style={{ flex: 1, background: '#0f172a', borderRadius: 4, height: 18, position: 'relative' }}>
            <div style={{
              width: `${(keys / maxKeys) * 100}%`,
              background: fill + '88',
              borderLeft: `3px solid ${fill}`,
              height: '100%',
              borderRadius: 4,
              transition: 'width 0.6s ease',
            }} />
            <span style={{
              position: 'absolute', left: 8, top: '50%',
              transform: 'translateY(-50%)',
              fontSize: 9, color: '#e2e8f0', fontFamily: 'monospace',
            }}>
              {keys.toLocaleString()}
            </span>
          </div>
          <div style={{ width: 40, fontSize: 10, color: fill, fontFamily: 'monospace', textAlign: 'right', flexShrink: 0 }}>
            {((keys / Math.max(data.reduce((s, d) => s + d.keys, 0), 1)) * 100).toFixed(0)}%
          </div>
        </div>
      ))}
    </div>
  );
}
