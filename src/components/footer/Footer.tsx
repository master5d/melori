import React, { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";

import ModelSelector from "../model-selector";

const Footer: React.FC = () => {
  const [version, setVersion] = useState("");

  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const appVersion = await getVersion();
        setVersion(appVersion);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setVersion("0.1.2");
      }
    };

    fetchVersion();
  }, []);

  return (
    <div className="w-full border-t border-rule pt-4 bg-ground">
      <div className="flex justify-between items-center text-[11px] px-6 pb-4 text-secondary font-medium">
        <div className="flex items-center gap-4">
          <ModelSelector />
        </div>

        {/* Update Status */}
        <div className="flex items-center gap-2">
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="font-bold text-secondary tracking-tight">
            v{version}
          </span>
        </div>
      </div>
    </div>
  );
};

export default Footer;
