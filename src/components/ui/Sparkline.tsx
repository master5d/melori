import { type FC } from "react";

interface SparklineProps {
  values: number[];
  width?: number;
  height?: number;
  color?: string;
}

/**
 * Minimal inline-SVG sparkline. No deps. Renders a polyline of the values
 * normalized to the box; empty input renders a dashed baseline.
 */
export const Sparkline: FC<SparklineProps> = ({
  values,
  width = 220,
  height = 40,
  color = "var(--color-accent)",
}) => {
  if (values.length === 0) {
    return (
      <svg width={width} height={height} role="img" aria-label="no data">
        <line
          x1={0}
          y1={height / 2}
          x2={width}
          y2={height / 2}
          stroke="var(--color-edge)"
          strokeDasharray="3 3"
        />
      </svg>
    );
  }
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const stepX = values.length > 1 ? width / (values.length - 1) : 0;
  const points = values
    .map((v, i) => {
      const x = i * stepX;
      const y = height - ((v - min) / span) * height;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <svg width={width} height={height} role="img" aria-label="trend">
      <polyline
        points={points}
        fill="none"
        stroke={color}
        strokeWidth={2}
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
};
