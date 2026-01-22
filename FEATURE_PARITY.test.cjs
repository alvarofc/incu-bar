const fs = require("node:fs");
const path = require("node:path");

const getSpec = () => {
  const specPath = path.join(__dirname, "FEATURE_PARITY.md");
  return fs.readFileSync(specPath, "utf-8");
};

const assertContains = (spec, title) => {
  if (!spec.includes(title)) {
    throw new Error(`Missing section: ${title}`);
  }
};

const spec = getSpec();
assertContains(spec, "## Living Spec Rules");
assertContains(spec, "## Feature Parity Baseline");
assertContains(spec, "### Parity Rules");
assertContains(spec, "## Parity Matrices");
assertContains(spec, "### Provider Parity Matrix");
assertContains(spec, "### App Parity Matrix");

console.log("Feature parity spec checks passed.");
