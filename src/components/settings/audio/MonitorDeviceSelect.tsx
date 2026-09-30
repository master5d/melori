import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../../ui/Dropdown";
import { SettingContainer } from "../../ui/SettingContainer";
import { commands } from "../../../bindings";
import type { AudioDevice } from "@/bindings";

interface MonitorDeviceSelectProps {
  disabled?: boolean;
}

/**
 * Output-device picker for the monitor path. Reads/writes the dedicated
 * `monitor_output_device` setting via commands (so the monitor can route to
 * headphones while system sound / TTS stay on the speakers).
 */
export const MonitorDeviceSelect: React.FC<MonitorDeviceSelectProps> = ({
  disabled = false,
}) => {
  const { t } = useTranslation();
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [selected, setSelected] = useState("Default");

  useEffect(() => {
    let alive = true;
    void commands.getAvailableOutputDevices().then((res) => {
      if (alive && res.status === "ok") setDevices(res.data);
    });
    void commands.getMonitorOutputDevice().then((res) => {
      if (alive && res.status === "ok") {
        setSelected(res.data === "default" ? "Default" : res.data);
      }
    });
    return () => {
      alive = false;
    };
  }, []);

  const handleSelect = (deviceName: string) => {
    setSelected(deviceName);
    void commands.setMonitorOutputDevice(deviceName);
  };

  const options = devices.map((d) => ({ value: d.name, label: d.name }));

  return (
    <SettingContainer
      title={t("settings.audio.monitor.device")}
      description={t("settings.audio.monitor.deviceDescription")}
      descriptionMode="tooltip"
      grouped
      disabled={disabled}
    >
      <Dropdown
        options={options}
        selectedValue={selected}
        onSelect={handleSelect}
        placeholder={t("settings.audio.monitor.device")}
        disabled={disabled || devices.length === 0}
      />
    </SettingContainer>
  );
};
