import React from 'react';
import { Bar, BarChart, Cell, Line, LineChart, Pie, PieChart, ResponsiveContainer } from 'recharts';

export interface MiniChartPoint {
  name: string;
  value: number;
}

interface Props {
  type: 'line' | 'bar' | 'pie';
  data: MiniChartPoint[];
  color: string;
  palette?: string[];
  height?: number;
}

const defaultPalette = ['#1976d2', '#43a047', '#f57c00', '#8e24aa', '#00897b', '#e91e63', '#546e7a', '#6d4c41'];

const StatsMiniChart: React.FC<Props> = ({ type, data, color, palette = defaultPalette, height = 42 }) => {
  const safe = data.filter(item => Number.isFinite(item.value) && item.value >= 0).slice(0, 8);
  if (safe.length === 0 || safe.every(item => item.value === 0)) {
    return <div style={{ height, borderBottom: '1px solid rgba(0,0,0,0.12)' }} />;
  }

  return (
    <div style={{ width: '100%', height }} aria-hidden="true">
      <ResponsiveContainer width="100%" height="100%">
        {type === 'line' ? (
          <LineChart data={safe} margin={{ top: 5, right: 2, bottom: 1, left: 2 }}>
            <Line type="monotone" dataKey="value" stroke={color} strokeWidth={2.2} dot={false} isAnimationActive={false} />
          </LineChart>
        ) : type === 'bar' ? (
          <BarChart data={safe} margin={{ top: 5, right: 2, bottom: 1, left: 2 }}>
            <Bar dataKey="value" fill={color} radius={[1, 1, 0, 0]} isAnimationActive={false} />
          </BarChart>
        ) : (
          <PieChart>
            <Pie data={safe} dataKey="value" nameKey="name" innerRadius="48%" outerRadius="88%" paddingAngle={2} isAnimationActive={false}>
              {safe.map((_, index) => <Cell key={index} fill={index === 0 ? color : palette[index % palette.length]} />)}
            </Pie>
          </PieChart>
        )}
      </ResponsiveContainer>
    </div>
  );
};

export default StatsMiniChart;
