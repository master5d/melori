import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const tokens = JSON.parse(
  readFileSync(join(root, "design/tokens.json"), "utf8"),
);

function luminance(hex) {
  const channels = [0, 2, 4].map(
    (offset) => Number.parseInt(hex.slice(1 + offset, 3 + offset), 16) / 255,
  );
  const linear = channels.map((c) =>
    c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4,
  );
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrast(foreground, background) {
  const a = luminance(foreground);
  const b = luminance(background);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

let failed = false;
for (const theme of ["day", "evening"]) {
  for (const [foreground, background, minimum] of tokens.contrast) {
    const ratio = contrast(
      tokens.color[theme][foreground],
      tokens.color[theme][background],
    );
    const status = ratio >= minimum ? "PASS" : "FAIL";
    console.log(
      `${theme} ${foreground}/${background} ${ratio.toFixed(2)} ${minimum.toFixed(1)} ${status}`,
    );
    if (status === "FAIL") failed = true;
  }
}
if (failed) process.exitCode = 1;
