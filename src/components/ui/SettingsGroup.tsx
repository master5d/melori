import React from "react";

interface SettingsGroupProps {
  title?: string;
  description?: string;
  children: React.ReactNode;
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  title,
  description,
  children,
}) => {
  return (
    <div className="space-y-4 mb-8">
      {title && (
        <div className="px-5">
          <h2 className="text-[11px] font-black text-secondary uppercase tracking-[0.25em]">
            {title}
          </h2>
          {description && (
            <p className="text-xs text-secondary mt-1.5 leading-relaxed">
              {description}
            </p>
          )}
        </div>
      )}
      <div className="bg-surface/40 border border-surface-raised/80 rounded-[2rem] overflow-hidden backdrop-blur-md shadow-2xl shadow-black/20">
        <div className="divide-y divide-surface-raised/40">{children}</div>
      </div>
    </div>
  );
};
