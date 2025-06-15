/**
 * Function Name Collision Resolver
 *
 * This module detects and resolves function name collisions between
 * multiple Pixelblaze patterns.
 */
const jscodeshift = require("jscodeshift");

/**
 * Extract function names from JavaScript source code
 * @param {string} jsCode - JavaScript source code
 * @returns {Array<string>} Array of function names
 */
function extractFunctionNames(jsCode) {
  const ast = jscodeshift(jsCode);
  const functionNames = [];

  // Find function declarations
  ast.find(jscodeshift.FunctionDeclaration).forEach((path) => {
    if (path.value.id && path.value.id.name) {
      functionNames.push(path.value.id.name);
    }
  });

  // Find exported function declarations
  ast.find(jscodeshift.ExportNamedDeclaration).forEach((path) => {
    if (
      path.value.declaration &&
      path.value.declaration.type === "FunctionDeclaration"
    ) {
      if (path.value.declaration.id && path.value.declaration.id.name) {
        functionNames.push(path.value.declaration.id.name);
      }
    }
  });

  // Find function expressions assigned to variables
  ast.find(jscodeshift.VariableDeclarator).forEach((path) => {
    if (
      path.value.init &&
      (path.value.init.type === "FunctionExpression" ||
        path.value.init.type === "ArrowFunctionExpression") &&
      path.value.id &&
      path.value.id.name
    ) {
      functionNames.push(path.value.id.name);
    }
  });

  return functionNames;
}

/**
 * Detect function name collisions between patterns
 * @param {Array<string>} patterns - Array of JavaScript source code strings
 * @returns {Array<string>} Array of colliding function names
 */
function detectFunctionCollisions(patterns) {
  const functionCounts = {};
  const collisions = [];

  patterns.forEach((pattern) => {
    const functionNames = extractFunctionNames(pattern);
    functionNames.forEach((name) => {
      functionCounts[name] = (functionCounts[name] || 0) + 1;
    });
  });

  Object.keys(functionCounts).forEach((name) => {
    if (functionCounts[name] > 1) {
      collisions.push(name);
    }
  });

  return collisions;
}

/**
 * Resolve function name collisions by adding prefixes
 * @param {Array<string>} patterns - Array of JavaScript source code strings
 * @param {Array<string>} patternNames - Array of pattern names for prefixes
 * @returns {Array<string>} Array of patterns with resolved function names
 */
function resolveFunctionCollisions(patterns, patternNames) {
  const collisions = detectFunctionCollisions(patterns);

  return patterns.map((pattern, index) => {
    let resolvedPattern = pattern;
    const prefix = patternNames[index];

    collisions.forEach((functionName) => {
      // Replace function declarations
      const functionDeclRegex = new RegExp(
        `(export\\s+)?function\\s+${functionName}\\b`,
        "g",
      );
      resolvedPattern = resolvedPattern.replace(
        functionDeclRegex,
        `$1function ${prefix}_${functionName}`,
      );

      // Replace function calls
      const functionCallRegex = new RegExp(`\\b${functionName}\\s*\\(`, "g");
      resolvedPattern = resolvedPattern.replace(
        functionCallRegex,
        `${prefix}_${functionName}(`,
      );
    });

    return resolvedPattern;
  });
}

module.exports = {
  extractFunctionNames,
  detectFunctionCollisions,
  resolveFunctionCollisions,
};
