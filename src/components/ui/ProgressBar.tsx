import { type FC } from "react";

interface ProgressBarProps {
  /** 0–100, or null for an indeterminate animation. */
  percent: number | null;
  label: string;
}

export const ProgressBar: FC<ProgressBarProps> = ({ percent, label }) => {
  const determinate = percent !== null && Number.isFinite(percent);
  const width = determinate
    ? Math.max(0, Math.min(100, percent as number))
    : 100;
  return (
    <div className="w-full space-y-1">
      <div className="flex justify-between text-xs text-text/60">
        <span>{label}</span>
        {determinate && <span>{Math.round(width)}%</span>}
      </div>
      <div className="h-2 w-full overflow-hidden rounded bg-edge/40">
        <div
          className={`h-full rounded bg-accent transition-[width] duration-200 ${
            determinate ? "" : "animate-pulse w-full"
          }`}
          style={determinate ? { width: `${width}%` } : undefined}
        />
      </div>
    </div>
  );
};
