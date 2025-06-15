/**
 * JSCodeshift transformer for Pixelblaze Superpattern System
 *
 * Transforms Pixelblaze JavaScript patterns to enable pattern combination by:
 * 1. Converting state variables (var declarations) to __state__ array references
 * 2. Converting global variables (undeclared assignments) to __globals__ array references
 * 3. Adding __state__ and __globals__ parameters to all functions
 * 4. Preserving local variables and function parameters unchanged
 *
 * @param {Object} fileInfo - Contains path and source
 * @param {Object} api - JSCodeshift API
 * @returns {string} Transformed source code
 */
module.exports = function (fileInfo, api) {
  const j = api.jscodeshift;
  const root = j(fileInfo.source);

  // =============================================================================
  // STEP 1: ANALYZE AND COLLECT VARIABLES
  // =============================================================================

  const stateVars = [];
  const globalVars = [];

  // Find state variables: var declarations at program root level
  root
    .find(j.VariableDeclaration)
    .filter((path) => path.parent.value.type === "Program")
    .forEach((path) => {
      path.value.declarations.forEach((decl) => {
        if (decl.id.type === "Identifier") {
          stateVars.push(decl.id.name);
        }
      });
    });

  // Also find exported variable declarations (these become globals)
  root
    .find(j.ExportNamedDeclaration)
    .filter(
      (path) =>
        path.value.declaration &&
        path.value.declaration.type === "VariableDeclaration",
    )
    .forEach((path) => {
      path.value.declaration.declarations.forEach((decl) => {
        if (decl.id.type === "Identifier") {
          globalVars.push(decl.id.name);
        }
      });
    });

  // Find global variables: assignments without local declarations
  const seenGlobals = new Set();

  root
    .find(j.AssignmentExpression)
    .filter((path) => {
      const left = path.value.left;
      if (left.type !== "Identifier") return false;

      const varName = left.name;

      // Skip if already processed
      if (seenGlobals.has(varName)) return false;

      // Skip if it's a state variable
      if (stateVars.includes(varName)) return false;

      // Check if declared in any scope up to program level
      let scope = path.scope;
      while (scope) {
        if (scope.declares(varName)) return false;
        scope = scope.parent;
      }

      // Must be at program level to be considered global
      let currentPath = path;
      while (
        currentPath.parent &&
        currentPath.parent.value.type !== "Program"
      ) {
        currentPath = currentPath.parent;
      }

      return currentPath.parent && currentPath.parent.value.type === "Program";
    })
    .forEach((path) => {
      const varName = path.value.left.name;
      if (!seenGlobals.has(varName)) {
        globalVars.push(varName);
        seenGlobals.add(varName);
      }
    });

  // =============================================================================
  // STEP 2: TRANSFORM VARIABLE DECLARATIONS
  // =============================================================================

  // Transform state variable declarations to __state__ assignments
  root
    .find(j.VariableDeclaration)
    .filter((path) => path.parent.value.type === "Program")
    .forEach((path) => {
      const declarations = path.value.declarations;
      const newStatements = [];

      declarations.forEach((decl) => {
        if (decl.id.type === "Identifier") {
          const varName = decl.id.name;
          const stateIndex = stateVars.indexOf(varName);

          if (stateIndex !== -1) {
            // Create __state__[index] = value
            const assignment = j.assignmentExpression(
              "=",
              j.memberExpression(
                j.identifier("__state__"),
                j.literal(stateIndex),
                true, // computed property
              ),
              decl.init || j.identifier("undefined"),
            );
            newStatements.push(j.expressionStatement(assignment));
          }
        }
      });

      // Replace original declaration with transformed assignments
      if (newStatements.length > 0) {
        j(path).replaceWith(newStatements);
      }
    });

  // Transform exported variable declarations to __globals__ assignments
  root
    .find(j.ExportNamedDeclaration)
    .filter(
      (path) =>
        path.value.declaration &&
        path.value.declaration.type === "VariableDeclaration",
    )
    .forEach((path) => {
      const declarations = path.value.declaration.declarations;
      const newStatements = [];

      declarations.forEach((decl) => {
        if (decl.id.type === "Identifier") {
          const varName = decl.id.name;
          const globalIndex = globalVars.indexOf(varName);

          if (globalIndex !== -1) {
            // Create __globals__[index] = value
            const assignment = j.assignmentExpression(
              "=",
              j.memberExpression(
                j.identifier("__globals__"),
                j.literal(globalIndex),
                true, // computed property
              ),
              decl.init || j.identifier("undefined"),
            );
            newStatements.push(j.expressionStatement(assignment));
          }
        }
      });

      // Replace original export declaration with transformed assignments
      if (newStatements.length > 0) {
        j(path).replaceWith(newStatements);
      }
    });

  // =============================================================================
  // STEP 6: TRANSFORM FUNCTION CALLS TO INCLUDE NEW PARAMETERS
  // =============================================================================

  // Add __state__ and __globals__ parameters to all functions
  root.find(j.Function).forEach((path) => {
    const stateParam = j.identifier("__state__");
    const globalsParam = j.identifier("__globals__");

    // Prepend new parameters while preserving existing ones
    path.value.params = [stateParam, globalsParam, ...path.value.params];
  });

  // =============================================================================
  // STEP 5: TRANSFORM VARIABLE REFERENCES IN FUNCTION BODIES
  // =============================================================================

  root.find(j.Function).forEach((funcPath) => {
    // Collect all locally declared variables in this function
    const localVars = new Set();

    // Add function parameters to local vars
    funcPath.value.params.forEach((param) => {
      if (param.type === "Identifier") {
        localVars.add(param.name);
      }
    });

    // Find all variable declarations within this function
    j(funcPath.get("body"))
      .find(j.VariableDeclarator)
      .forEach((declPath) => {
        if (declPath.value.id.type === "Identifier") {
          localVars.add(declPath.value.id.name);
        }
      });

    // Also find function declarations within this function
    j(funcPath.get("body"))
      .find(j.FunctionDeclaration)
      .forEach((funcDeclPath) => {
        if (
          funcDeclPath.value.id &&
          funcDeclPath.value.id.type === "Identifier"
        ) {
          localVars.add(funcDeclPath.value.id.name);
        }
      });

    // Transform identifier references within this function's body
    j(funcPath.get("body"))
      .find(j.Identifier)
      .filter((path) => {
        const name = path.value.name;

        // Skip __state__ and __globals__ (our injected parameters)
        if (name === "__state__" || name === "__globals__") return false;

        // Skip function names and property names
        const parent = path.parent.value;
        if (parent.type === "Property" && path.name === "key") return false;
        if (
          parent.type === "MemberExpression" &&
          path.name === "property" &&
          !parent.computed
        )
          return false;
        if (parent.type === "FunctionDeclaration" && path.name === "id")
          return false;
        if (parent.type === "FunctionExpression" && path.name === "id")
          return false;
        if (parent.type === "VariableDeclarator" && path.name === "id")
          return false;

        // Skip if this identifier is the left side of an assignment expression
        // (these are handled separately in assignment expression transformation)
        if (parent.type === "AssignmentExpression" && path.name === "left")
          return false;

        // Skip if this is a locally declared variable (including parameters)
        if (localVars.has(name)) return false;

        // Only transform if it's a known state or global variable
        return stateVars.includes(name) || globalVars.includes(name);
      })
      .forEach((path) => {
        const name = path.value.name;

        if (stateVars.includes(name)) {
          const stateIndex = stateVars.indexOf(name);
          j(path).replaceWith(
            j.memberExpression(
              j.identifier("__state__"),
              j.literal(stateIndex),
              true,
            ),
          );
        } else if (globalVars.includes(name)) {
          const globalIndex = globalVars.indexOf(name);
          j(path).replaceWith(
            j.memberExpression(
              j.identifier("__globals__"),
              j.literal(globalIndex),
              true,
            ),
          );
        }
      });

    // Also transform assignment expressions within this function, respecting local variables
    j(funcPath.get("body"))
      .find(j.AssignmentExpression)
      .filter((path) => {
        const left = path.value.left;
        if (left.type !== "Identifier") return false;

        const name = left.name;

        // Skip if this is a locally declared variable (including parameters)
        if (localVars.has(name)) return false;

        // Only transform if it's a known state or global variable
        return stateVars.includes(name) || globalVars.includes(name);
      })
      .forEach((path) => {
        const name = path.value.left.name;

        if (stateVars.includes(name)) {
          const stateIndex = stateVars.indexOf(name);
          path.value.left = j.memberExpression(
            j.identifier("__state__"),
            j.literal(stateIndex),
            true,
          );
        } else if (globalVars.includes(name)) {
          const globalIndex = globalVars.indexOf(name);
          path.value.left = j.memberExpression(
            j.identifier("__globals__"),
            j.literal(globalIndex),
            true,
          );
        }
      });
  });

  // =============================================================================
  // STEP 6: TRANSFORM FUNCTION CALLS TO INCLUDE NEW PARAMETERS
  // =============================================================================

  // Transform global assignments that are not inside functions
  root
    .find(j.AssignmentExpression)
    .filter((path) => {
      const left = path.value.left;
      if (left.type !== "Identifier" || !globalVars.includes(left.name)) {
        return false;
      }

      // Check if this assignment is at program level (not inside a function)
      let currentPath = path;
      while (currentPath.parent) {
        const parentType = currentPath.parent.value.type;
        if (
          parentType === "FunctionDeclaration" ||
          parentType === "FunctionExpression" ||
          parentType === "ArrowFunctionExpression"
        ) {
          return false; // Inside a function, skip
        }
        if (parentType === "Program") {
          return true; // At program level
        }
        currentPath = currentPath.parent;
      }
      return false;
    })
    .forEach((path) => {
      const varName = path.value.left.name;
      const globalIndex = globalVars.indexOf(varName);

      // Replace identifier with __globals__[index]
      path.value.left = j.memberExpression(
        j.identifier("__globals__"),
        j.literal(globalIndex),
        true,
      );
    });

  // =============================================================================
  // STEP 6: TRANSFORM FUNCTION CALLS TO INCLUDE NEW PARAMETERS
  // =============================================================================

  // Transform calls to transformed functions to include __state__ and __globals__
  root
    .find(j.CallExpression)
    .filter((path) => {
      // Only transform calls to functions that were declared in this file
      const callee = path.value.callee;
      if (callee.type === "Identifier") {
        // Check if this is a function declared in the same file
        return root
          .find(j.FunctionDeclaration)
          .some(
            (funcPath) =>
              funcPath.value.id && funcPath.value.id.name === callee.name,
          );
      } else if (callee.type === "MemberExpression") {
        // Handle calls to function expressions stored in variables
        // Check if the object is a transformed state or global variable
        if (callee.object.type === "MemberExpression") {
          const obj = callee.object;
          if (
            obj.object.type === "Identifier" &&
            (obj.object.name === "__state__" ||
              obj.object.name === "__globals__")
          ) {
            return true;
          }
        }
      }
      return false;
    })
    .forEach((path) => {
      // Prepend __state__ and __globals__ arguments
      const stateArg = j.identifier("__state__");
      const globalsArg = j.identifier("__globals__");
      path.value.arguments = [stateArg, globalsArg, ...path.value.arguments];
    });

  // =============================================================================
  // STEP 7: TRANSFORM GLOBAL VARIABLE ASSIGNMENTS AT PROGRAM LEVEL
  // =============================================================================

  // Transform global assignments that are not inside functions
  root
    .find(j.AssignmentExpression)
    .filter((path) => {
      const left = path.value.left;
      if (left.type !== "Identifier" || !globalVars.includes(left.name)) {
        return false;
      }

      // Check if this assignment is at program level (not inside a function)
      let currentPath = path;
      while (currentPath.parent) {
        const parentType = currentPath.parent.value.type;
        if (
          parentType === "FunctionDeclaration" ||
          parentType === "FunctionExpression" ||
          parentType === "ArrowFunctionExpression"
        ) {
          return false; // Inside a function, skip
        }
        if (parentType === "Program") {
          return true; // At program level
        }
        currentPath = currentPath.parent;
      }
      return false;
    })
    .forEach((path) => {
      const varName = path.value.left.name;
      const globalIndex = globalVars.indexOf(varName);

      // Replace identifier with __globals__[index]
      path.value.left = j.memberExpression(
        j.identifier("__globals__"),
        j.literal(globalIndex),
        true,
      );
    });

  // =============================================================================
  // RETURN TRANSFORMED SOURCE
  // =============================================================================

  return root.toSource({
    quote: "single",
    reuseParsers: true,
    lineTerminator: "\n",
    retainLines: true,
    format: {
      preserveBlankLines: true,
      comments: true,
    },
  });
};

// Export parser configuration for Pixelblaze's JavaScript-like language
module.exports.parser = "babel";
