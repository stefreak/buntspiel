/**
 * Blend Mode Implementation
 *
 * This module provides functions for blending colors using different modes.
 */

/**
 * Generate blend function for specific blend mode
 * @param {string} blendMode - Blend mode ('ADD', 'SUB', 'AVG', 'MASK')
 * @returns {string} Blend function JavaScript code
 */
function generateBlendFunction(blendMode) {
  switch (blendMode) {
    case "ADD":
      return `
function blendAdd(r1, g1, b1, r2, g2, b2) {
  r = min(1, r1 + r2);
  g = min(1, g1 + g2);
  b = min(1, b1 + b2);
  return [r, g, b];
}`;

    case "SUB":
      return `
function blendSub(r1, g1, b1, r2, g2, b2) {
  r = max(0, r1 - r2);
  g = max(0, g1 - g2);
  b = max(0, b1 - b2);
  return [r, g, b];
}`;

    case "AVG":
      return `
function blendAvg(r1, g1, b1, r2, g2, b2) {
  r = (r1 + r2) / 2;
  g = (g1 + g2) / 2;
  b = (b1 + b2) / 2;
  return [r, g, b];
}`;

    case "MASK":
      return `
function blendMask(r1, g1, b1, r2, g2, b2) {
  var brightness1 = (r1 + g1 + b1) / 3;
  r = r2 * brightness1;
  g = g2 * brightness1;
  b = b2 * brightness1;
  return [r, g, b];
}`;

    default:
      return `
function blendDefault(r1, g1, b1, r2, g2, b2) {
  r = r1; g = g1; b = b1;
  return [r, g, b];
}`;
  }
}

module.exports = {
  generateBlendFunction,
};
