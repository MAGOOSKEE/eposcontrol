import { copyFileSync, mkdirSync } from "node:fs";
mkdirSync("ui/fonts", { recursive: true });
copyFileSync(
  "node_modules/material-symbols/material-symbols-rounded.woff2",
  "ui/fonts/material-symbols-rounded.woff2"
);
