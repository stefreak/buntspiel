/**
 * Pattern Wrapper
 *
 * This module wraps transformed Pixelblaze patterns in constructor functions
 * for use in the combination system.
 */

/**
 * Extract state initializations from transformed pattern
 * @param {string} transformedPattern - Transformed JavaScript code
 * @returns {Array<any>} Array of initial state values
 */
function extractStateInitializations(transformedPattern) {
  const stateValues = [];
  const stateRegex = /__state__\[(\d+)\]\s*=\s*([^;]+);/g;
  let match;

  while ((match = stateRegex.exec(transformedPattern)) !== null) {
    const index = parseInt(match[1]);
    const value = match[2].trim();

    let parsedValue;
    if (value === "undefined") {
      parsedValue = undefined;
    } else if (value === "true" || value === "false") {
      parsedValue = value === "true";
    } else if (!isNaN(parseFloat(value))) {
      parsedValue = parseFloat(value);
    } else {
      parsedValue = 0; // Default fallback
    }

    stateValues[index] = parsedValue;
  }

  return stateValues;
}

/**
 * Extract and convert export functions to local variables
 * @param {string} transformedPattern - Transformed JavaScript code
 * @returns {Object} Object with function assignments and cleaned code
 */
function extractAndConvertFunctions(transformedPattern) {
  const functions = {
    render: "0",
    render2d: "0",
    render3d: "0",
    beforeRender: "0",
  };

  let cleanedCode = transformedPattern;

  const exportFunctionRegex =
    /export\s+function\s+(\w+)\s*\([^)]*\)\s*\{[^}]*\}/g;
  let match;

  while ((match = exportFunctionRegex.exec(transformedPattern)) !== null) {
    const fullMatch = match[0];
    const functionName = match[1];

    if (functions.hasOwnProperty(functionName)) {
      const localFunction = fullMatch.replace(
        /export\s+function\s+/,
        "var temp_func = function ",
      );
      functions[functionName] = functionName;

      cleanedCode = cleanedCode.replace(
        fullMatch,
        localFunction.replace("temp_func", functionName),
      );
    }
  }

  return { functions, cleanedCode };
}

/**
 * Wrap transformed pattern in constructor function
 * @param {string} transformedPattern - Transformed JavaScript code
 * @param {string} patternName - Name of the pattern
 * @returns {string} Wrapped pattern constructor
 */
function wrapPatternInConstructor(transformedPattern, patternName) {
  const stateValues = extractStateInitializations(transformedPattern);
  const { functions, cleanedCode } =
    extractAndConvertFunctions(transformedPattern);

  const stateArray =
    stateValues.length > 0
      ? `[${stateValues
          .map((v) => (v === undefined ? "undefined" : JSON.stringify(v)))
          .join(", ")}]`
      : "[]";

  let finalCode = cleanedCode.replace(/__state__\[\d+\]\s*=\s*[^;]+;\s*/g, "");

  const functionInits = Object.keys(functions)
    .map((name) => `  var ${name} = ${functions[name]};`)
    .join("\n");

  const wrapper = `/** ${patternName} **/
() => {
${functionInits}
  var __state__ = ${stateArray};
  var __globals__ = [];

${finalCode
  .split("\n")
  .map((line) => "  " + line)
  .join("\n")}

  return [render, render2d, render3d, beforeRender, __state__];
}`;

  return wrapper;
}

module.exports = {
  extractStateInitializations,
  extractAndConvertFunctions,
  wrapPatternInConstructor,
};
