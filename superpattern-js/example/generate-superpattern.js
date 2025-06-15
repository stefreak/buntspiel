/**
 * Generate Superpattern Example
 *
 * This script combines multiple Pixelblaze patterns from the example/patterns
 * directory into a single superpattern using the Buntspiel Superpattern Transform.
 *
 * Usage: node generate-superpattern.js
 */

const fs = require("fs");
const path = require("path");
const { combineEpePatterns } = require("../src/index.js");

// Configuration
const PATTERNS_DIR = path.join(__dirname, "patterns");
const OUTPUT_FILE = path.join(__dirname, "superpattern.js");
const PATTERNS_TO_COMBINE = [
  "blink fade.epe",
  "color fade pulse.epe",
  "# Simple Blink.epe",
];
const BLEND_MODES = [
  "ADD", // Between pattern 0 and 1
  "MASK", // Between (0+1) and 2
];

// Ensure the patterns directory exists
if (!fs.existsSync(PATTERNS_DIR)) {
  console.error(`Patterns directory not found: ${PATTERNS_DIR}`);
  process.exit(1);
}

// Read and parse the patterns
function readPatternFile(filename) {
  try {
    const filePath = path.join(PATTERNS_DIR, filename);
    let content = fs.readFileSync(filePath, "utf8");

    // Remove BOM character if present
    if (content.charCodeAt(0) === 0xfeff) {
      content = content.slice(1);
    }

    return JSON.parse(content);
  } catch (error) {
    console.error(`Error reading pattern file ${filename}:`, error);
    process.exit(1);
  }
}

// Load the pattern files
console.log("Loading patterns...");
const patterns = PATTERNS_TO_COMBINE.map((file) => {
  console.log(`  - ${file}`);
  return readPatternFile(file);
});

// Combine the patterns
console.log(
  `Combining ${patterns.length} patterns with blend modes: ${BLEND_MODES.join(", ")}...`,
);
try {
  const combinedCode = combineEpePatterns(patterns, BLEND_MODES);

  // Write the output file
  fs.writeFileSync(OUTPUT_FILE, combinedCode, "utf8");
  console.log(`Success! Superpattern created at: ${OUTPUT_FILE}`);

  // Output pattern information
  console.log("\nCombined patterns:");
  patterns.forEach((pattern, index) => {
    console.log(`  ${index}: ${pattern.name || "Unnamed pattern"}`);
  });

  // Add usage instructions
  console.log("\nUsage in Pixelblaze:");
  console.log("1. Copy the contents of superpattern.js");
  console.log("2. Create a new pattern in Pixelblaze");
  console.log("3. Paste the code and save");
} catch (error) {
  console.error("Error combining patterns:", error);
  process.exit(1);
}
