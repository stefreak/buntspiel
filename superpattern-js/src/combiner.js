const { generateBlendFunction } = require("./blend-modes.js");

/**
 * Generate color capture and blend logic
 * @param {Array<string>} blendModes - Array of blend modes for each pattern pair
 * @returns {string} Color blending JavaScript code
 */
function generateColorBlendingCode(blendModes) {
  const blendFunctions = blendModes
    .map((mode) => generateBlendFunction(mode))
    .join("\n");

  return `
${blendFunctions}

// Color capture variables
var capturedColors = [];
var currentColorIndex = 0;

// Override hsv function to capture colors
var originalHsv = hsv;
function hsv(h, s, v) {
  // Convert HSV to RGB for blending
  var rgb = hsvToRgb(h, s, v);
  capturedColors[currentColorIndex] = rgb;
  currentColorIndex++;
}

// HSV to RGB conversion
function hsvToRgb(h, s, v) {
  var r, g, b;
  var i = floor(h * 6);
  var f = h * 6 - i;
  var p = v * (1 - s);
  var q = v * (1 - f * s);
  var t = v * (1 - (1 - f) * s);

  switch (i % 6) {
    case 0: r = v, g = t, b = p; break;
    case 1: r = q, g = v, b = p; break;
    case 2: r = p, g = v, b = t; break;
    case 3: r = p, g = q, b = v; break;
    case 4: r = t, g = p, b = v; break;
    case 5: r = v, g = p, b = q; break;
  }

  return [r, g, b];
}

// RGB to HSV conversion
function rgbToHsv(r, g, b) {
  var max = Math.max(r, g, b), min = Math.min(r, g, b);
  var h, s, v = max;
  var d = max - min;
  s = max == 0 ? 0 : d / max;

  if (max == min) {
    h = 0;
  } else {
    switch (max) {
      case r: h = (g - b) / d + (g < b ? 6 : 0); break;
      case g: h = (b - r) / d + 2; break;
      case b: h = (r - g) / d + 4; break;
    }
    h /= 6;
  }

  return [h, s, v];
}
`;
}

/**
 * Combine multiple pattern constructors with blend modes
 * @param {Array<string>} patternConstructors - Array of pattern constructor functions
 * @param {Array<string>} blendModes - Array of blend modes
 * @returns {string} Combined pattern JavaScript code
 */
function combinePatterns(patternConstructors, blendModes) {
  const colorBlendingCode = generateColorBlendingCode(blendModes);

  const patternInitCode = patternConstructors
    .map((constructor, index) => {
      return `var pattern${index} = (${constructor})();`;
    })
    .join("\n");

  const beforeRenderCode = patternConstructors
    .map((_, index) => {
      return `
  if (pattern${index}[3] !== 0) {
    pattern${index}[3](delta);
  }`;
    })
    .join("");

  const renderCode = `
function combineRender(index) {
  capturedColors = [];
  currentColorIndex = 0;

  // Call each pattern's render function
  ${patternConstructors
    .map(
      (_, index) => `
  if (pattern${index}[0] !== 0) {
    pattern${index}[0](index);
  }`,
    )
    .join("")}

  // Blend the captured colors
  if (capturedColors.length >= 2) {
    var blended = capturedColors[0];
    for (var i = 1; i < capturedColors.length; i++) {
      var currentBlendMode = '${blendModes[0]}'; // Use first blend mode for now
      switch (currentBlendMode) {
        case 'ADD':
          blended = blendAdd(blended[0], blended[1], blended[2],
                           capturedColors[i][0], capturedColors[i][1], capturedColors[i][2]);
          break;
        case 'SUB':
          blended = blendSub(blended[0], blended[1], blended[2],
                           capturedColors[i][0], capturedColors[i][1], capturedColors[i][2]);
          break;
        case 'AVG':
          blended = blendAvg(blended[0], blended[1], blended[2],
                           capturedColors[i][0], capturedColors[i][1], capturedColors[i][2]);
          break;
        case 'MASK':
          blended = blendMask(blended[0], blended[1], blended[2],
                            capturedColors[i][0], capturedColors[i][1], capturedColors[i][2]);
          break;
      }
    }

    // Convert back to HSV and output
    var hsv_result = rgbToHsv(blended[0], blended[1], blended[2]);
    originalHsv(hsv_result[0], hsv_result[1], hsv_result[2]);
  } else if (capturedColors.length === 1) {
    var hsv_result = rgbToHsv(capturedColors[0][0], capturedColors[0][1], capturedColors[0][2]);
    originalHsv(hsv_result[0], hsv_result[1], hsv_result[2]);
  }
}`;

  return `
// Buntspiel Combined Pattern
${colorBlendingCode}

// Pattern instances
${patternInitCode}

// Combined beforeRender function
export function beforeRender(delta) {${beforeRenderCode}
}

// Combined render function
export function render(index) {
  combineRender(index);
}

${renderCode}

// COMBINATOR_ADD, COMBINATOR_SUB, COMBINATOR_AVG, COMBINATOR_MASK constants
var COMBINATOR_ADD = 0;
var COMBINATOR_SUB = 1;
var COMBINATOR_AVG = 2;
var COMBINATOR_MASK = 3;

// blendColors helper function
function blendColors(color1, color2, mode) {
  switch(mode) {
    case COMBINATOR_ADD: return blendAdd(color1[0], color1[1], color1[2], color2[0], color2[1], color2[2]);
    case COMBINATOR_SUB: return blendSub(color1[0], color1[1], color1[2], color2[0], color2[1], color2[2]);
    case COMBINATOR_AVG: return blendAvg(color1[0], color1[1], color1[2], color2[0], color2[1], color2[2]);
    case COMBINATOR_MASK: return blendMask(color1[0], color1[1], color1[2], color2[0], color2[1], color2[2]);
    default: return color1;
  }
}
`;
}

module.exports = {
  combinePatterns,
  generateColorBlendingCode,
};
