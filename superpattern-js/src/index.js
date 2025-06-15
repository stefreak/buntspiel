const jscodeshift = require("jscodeshift");
const transform = require("./transform.js");
const { extractJavaScriptFromEpe, parseEpeFile } = require("./parser.js");
const {
  detectFunctionCollisions,
  resolveFunctionCollisions,
} = require("./collision-resolver.js");
const { wrapPatternInConstructor } = require("./pattern-wrapper.js");
const { combinePatterns } = require("./combiner.js");

/**
 * Buntspiel Superpattern System
 *
 * Main API for combining multiple Pixelblaze patterns into a single
 * superpattern, handling variable isolation, function name collisions,
 * and different blend modes.
 */

/**
 * Transform and wrap a single pattern
 * @param {Object} epePattern - Parsed .epe pattern
 * @returns {string} Transformed and wrapped pattern constructor
 */
function transformAndWrapPattern(epePattern) {
  const js = extractJavaScriptFromEpe(epePattern);
  const transformed = transform({ source: js }, { jscodeshift });
  const patternName = epePattern.name
    .replace(/\s+/g, "")
    .replace(/[^a-zA-Z0-9_]/g, "");
  return wrapPatternInConstructor(transformed, patternName);
}

/**
 * Combine .epe patterns end-to-end
 * @param {Array<Object>} epePatterns - Array of parsed .epe patterns
 * @param {Array<string>} blendModes - Array of blend modes
 * @returns {string} Combined superpattern JavaScript code
 */
function combineEpePatterns(epePatterns, blendModes) {
  // Extract and transform each pattern
  const patterns = epePatterns.map((epe) => extractJavaScriptFromEpe(epe));
  const patternNames = epePatterns.map((epe) =>
    epe.name.replace(/\s+/g, "").replace(/[^a-zA-Z0-9_]/g, ""),
  );

  // Resolve function name collisions
  const resolvedPatterns = resolveFunctionCollisions(patterns, patternNames);

  // Transform each pattern
  const transformedPatterns = resolvedPatterns.map((pattern) =>
    transform({ source: pattern }, { jscodeshift }),
  );

  // Wrap each pattern in a constructor
  const wrappedPatterns = transformedPatterns.map((pattern, index) =>
    wrapPatternInConstructor(pattern, patternNames[index]),
  );

  // Combine the patterns
  return combinePatterns(wrappedPatterns, blendModes);
}

module.exports = {
  // Core API
  combineEpePatterns,
  transformAndWrapPattern,

  // Individual components
  extractJavaScriptFromEpe,
  parseEpeFile,
  detectFunctionCollisions,
  resolveFunctionCollisions,
  wrapPatternInConstructor,
  combinePatterns,
  transform,
};
