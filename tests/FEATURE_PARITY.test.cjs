const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const getSpec = () => {
  const specPath = path.join(root, "FEATURE_PARITY.md");
  return fs.readFileSync(specPath, "utf-8");
};

const assertContains = (spec, title) => {
  if (!spec.includes(title)) {
    throw new Error(`Missing section: ${title}`);
  }
};

const extractProvidersSection = (spec) => {
  const startIndex = spec.indexOf("### Provider Parity Matrix");
  if (startIndex === -1) {
    throw new Error("Missing Provider Parity Matrix section");
  }
  const rest = spec.slice(startIndex);
  const appIndex = rest.indexOf("### App Parity Matrix");
  return appIndex === -1 ? rest : rest.slice(0, appIndex);
};

const assertContainsProviderIds = (spec, providerIds) => {
  const section = extractProvidersSection(spec);
  providerIds.forEach((id) => {
    if (!section.includes(`id: ${id}`)) {
      throw new Error(`Provider parity matrix missing id: ${id}`);
    }
  });
};

const spec = getSpec();
assertContains(spec, "## Living Spec Rules");
assertContains(spec, "## Feature Parity Baseline");
assertContains(spec, "### Baseline Checklist");
assertContains(spec, "### Parity Rules");
assertContains(spec, "### Baseline Evidence");
assertContains(spec, "## Parity Matrices");
assertContains(spec, "### Provider Parity Matrix");
assertContains(spec, "### App Parity Matrix");
assertContainsProviderIds(spec, [
  "codex",
  "claude",
  "cursor",
  "copilot",
  "gemini",
  "zai",
  "kimi_k2",
  "synthetic",
  "factory",
  "augment",
  "kimi",
  "minimax",
  "amp",
  "opencode",
  "kiro",
  "jetbrains",
  "vertexai",
  "antigravity",
]);

console.log("Feature parity spec checks passed.");
