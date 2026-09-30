import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Slider } from "../../ui/Slider";
import { commands } from "../../../bindings";

/** Formats a 0..100 monitor volume as a percentage: "75%". */
function formatPct(v: number): string {
  return `${Math.round(v)}%`;
}

interface MonitorVolumeSliderProps {
  disabled?: boolean;
}

/**
 * Monitor-volume control (0–100%). Independent of the Epic-B capture gain — it
 * only sets how loud the user hears themselves. Backend stores a linear 0..1
 * fraction; this slider works in whole percent.
 */
export const MonitorVolumeSlider: React.FC<MonitorVolumeSliderProps> = ({
  disabled = false,
}) => {
  const { t } = useTranslation();
  const [pct, setPct] = useState(100);

  useEffect(() => {
    let alive = true;
    void commands.getMonitorVolume().then((res) => {
      if (alive && res.status === "ok") setPct(Math.round(res.data * 100));
    });
    return () => {
      alive = false;
    };
  }, []);

  const handleChange = (value: number) => {
    setPct(value);
    void commands.setMonitorVolume(value / 100);
  };

  return (
    <Slider
      value={pct}
      onChange={handleChange}
      min={0}
      max={100}
      step={1}
      label={t("settings.audio.monitor.volume")}
      description={t("settings.audio.monitor.volumeDescription")}
      descriptionMode="tooltip"
      grouped
      formatValue={formatPct}
      disabled={disabled}
    />
  );
};
