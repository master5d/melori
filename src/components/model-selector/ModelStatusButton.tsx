import React from "react";

type ModelStatus =
  | "ready"
  | "loading"
  | "downloading"
  | "verifying"
  | "extracting"
  | "error"
  | "unloaded"
  | "none";

interface ModelStatusButtonProps {
  status: ModelStatus;
  displayText: string;
  isDropdownOpen: boolean;
  onClick: () => void;
  className?: string;
}

const ModelStatusButton: React.FC<ModelStatusButtonProps> = ({
  status,
  displayText,
  isDropdownOpen,
  onClick,
  className = "",
}) => {
  const getStatusColor = (status: ModelStatus): string => {
    switch (status) {
      case "ready":
        return "bg-ok";
      case "loading":
        return "bg-accent-hot animate-pulse";
      case "downloading":
        return "bg-accent animate-pulse";
      case "verifying":
        return "bg-accent-hot animate-pulse";
      case "extracting":
        return "bg-accent-hot animate-pulse";
      case "error":
        return "bg-err";
      case "unloaded":
        return "bg-secondary";
      case "none":
        return "bg-err";
      default:
        return "bg-secondary";
    }
  };

  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-2.5 hover:text-accent transition-all duration-200 group ${className}`}
      title={`Model status: ${displayText}`}
    >
      <div
        className={`w-2 h-2 rounded-full transition-shadow duration-300 ${getStatusColor(status)}`}
      />
      <span className="max-w-32 truncate font-bold tracking-tight text-primary group-hover:text-accent">
        {displayText}
      </span>
      <svg
        className={`w-3.5 h-3.5 text-secondary group-hover:text-accent transition-all duration-300 ${isDropdownOpen ? "rotate-180 text-accent" : ""}`}
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={3}
          d="M19 9l-7 7-7-7"
        />
      </svg>
    </button>
  );
};

export default ModelStatusButton;
