import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { Provenance, SessionDetail } from "../api";
import { provenanceInput, provenanceLine } from "./sessionLogic";

export function ProvenanceChip({
  provenance,
  detail,
}: {
  provenance: Provenance;
  detail: SessionDetail;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <button
      type="button"
      className="border border-rule px-2 py-1 text-start font-mono text-xs text-secondary"
      onClick={() => setOpen((value) => !value)}
      aria-expanded={open}
    >
      <span>{provenanceLine(provenance, t)}</span>
      {open && (
        <span className="mt-1 block font-sans text-secondary">
          {provenanceInput(provenance, detail, t)}
        </span>
      )}
    </button>
  );
}
