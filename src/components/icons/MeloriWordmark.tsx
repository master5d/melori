import React from "react";

/** Product name — not UI copy, never translated. */
const PRODUCT_NAME = "melori";

/** melori wordmark — Literata set in the accent, the same letterform as the app icon. */
const MeloriWordmark: React.FC<{ size?: number; className?: string }> = ({
  size = 32,
  className = "",
}) => (
  <span
    className={`font-serif font-semibold leading-none text-accent ${className}`}
    style={{ fontSize: size }}
  >
    {PRODUCT_NAME}
  </span>
);

export default MeloriWordmark;
