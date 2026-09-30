import React from "react";
import { useTranslation } from "react-i18next";
import type { ModelInfo } from "@/bindings";
import {
  getTranslatedModelName,
  getTranslatedModelDescription,
} from "../../lib/utils/modelTranslation";

interface ModelDropdownProps {
  models: ModelInfo[];
  currentModelId: string;
  onModelSelect: (modelId: string) => void;
}

const ModelDropdown: React.FC<ModelDropdownProps> = ({
  models,
  currentModelId,
  onModelSelect,
}) => {
  const { t } = useTranslation();
  const downloadedModels = models.filter((m) => m.is_downloaded);

  const handleModelClick = (modelId: string) => {
    onModelSelect(modelId);
  };

  return (
    <div className="absolute bottom-full start-0 mb-3 w-72 max-h-[60vh] overflow-y-auto bg-surface/95 border border-surface-raised rounded-2xl shadow-2xl backdrop-blur-xl py-2 z-50">
      {downloadedModels.length > 0 ? (
        <div>
          {downloadedModels.map((model) => (
            <div
              key={model.id}
              onClick={() => handleModelClick(model.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleModelClick(model.id);
                }
              }}
              tabIndex={0}
              role="button"
              className={`w-full px-4 py-3 text-start hover:bg-white/5 transition-all cursor-pointer focus:outline-none ${
                currentModelId === model.id ? "bg-accent/20" : ""
              }`}
            >
              <div className="flex items-center justify-between">
                <div className="min-w-0">
                  <div
                    className={`text-[13px] font-bold tracking-tight truncate ${currentModelId === model.id ? "text-accent" : "text-primary"}`}
                  >
                    {getTranslatedModelName(model, t)}
                    {model.is_custom && (
                      <span className="ms-2 text-[9px] font-black text-secondary uppercase tracking-widest bg-surface-raised px-1.5 py-0.5 rounded">
                        {t("modelSelector.custom")}
                      </span>
                    )}
                  </div>
                  <div className="text-[11px] text-secondary italic truncate mt-0.5">
                    {getTranslatedModelDescription(model, t)}
                  </div>
                </div>
                {currentModelId === model.id && (
                  <div className="ms-4 shrink-0">
                    <div className="w-2 h-2 rounded-full bg-accent" />
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="px-4 py-3 text-sm text-secondary italic">
          {t("modelSelector.noModelsAvailable")}
        </div>
      )}
    </div>
  );
};

export default ModelDropdown;
