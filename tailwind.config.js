/** @type {import('tailwindcss').Config} */
// Warm Studio: the source of truth for every color/font token is the
// `@theme` block in src/App.css (Tailwind v4 CSS-first config). The entries
// below are legacy JS-config aliases kept only so editors/tooling that read
// this file resolve the same names; they point at the same CSS custom
// properties App.css defines (which are themselves aliased to the new
// semantic Warm Studio tokens — see App.css for the full set).
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        text: "var(--color-text)",
        background: "var(--color-background)",
        "background-ui": "var(--color-background-ui)",
        "logo-primary": "var(--color-logo-primary)",
        "logo-stroke": "var(--color-logo-stroke)",
        "text-stroke": "var(--color-text-stroke)",
        "mid-gray": "var(--color-mid-gray)",
      },
    },
  },
  plugins: [],
};
