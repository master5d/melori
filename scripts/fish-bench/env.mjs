import { existsSync, readFileSync } from "node:fs";

export function loadFishKey({ env = process.env, envFilePath }) {
  if (env.FISH_API_KEY) return env.FISH_API_KEY;

  if (envFilePath && existsSync(envFilePath)) {
    for (const line of readFileSync(envFilePath, "utf8").split(/\r?\n/)) {
      const match = line.match(/^FISH_API_KEY=(.+)$/);
      if (match) return match[1].trim();
    }
  }

  throw new Error(
    "FISH_API_KEY not found: set the env var or put FISH_API_KEY=... into .env.fish at the repo root",
  );
}
