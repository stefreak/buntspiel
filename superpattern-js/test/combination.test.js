const jscodeshift = require("jscodeshift");
const transform = require("../src/transform.js");
const { extractJavaScriptFromEpe } = require("../src/parser.js");
const {
  detectFunctionCollisions,
  resolveFunctionCollisions,
} = require("../src/collision-resolver.js");
const { wrapPatternInConstructor } = require("../src/pattern-wrapper.js");
const { combinePatterns } = require("../src/combiner.js");
const { generateBlendFunction } = require("../src/blend-modes.js");
const {
  combineEpePatterns,
  transformAndWrapPattern,
} = require("../src/index.js");

/**
 * Comprehensive Test Suite for Buntspiel Superpattern Combination System
 *
 * This test suite covers:
 * 1. Pattern parsing from .epe files
 * 2. Function name collision resolution
 * 3. Pattern wrapping and constructor generation
 * 4. Pattern combination with blend modes
 * 5. Variable isolation between combined patterns
 */

describe("Pattern Combination System", () => {
  // =============================================================================
  // 1. PATTERN PARSING TESTS
  // =============================================================================

  describe("Pattern Parsing", () => {
    test("parses simple .epe file format", () => {
      const epeContent = {
        name: "Test Pattern",
        id: "test123",
        sources: {
          main: "var brightness = 0.5;\nexport function render(index) {\n  hsv(0.2, 1, brightness);\n}",
        },
      };

      const expectedJavaScript =
        "var brightness = 0.5;\nexport function render(index) {\n  hsv(0.2, 1, brightness);\n}";
      expect(extractJavaScriptFromEpe(epeContent)).toBe(expectedJavaScript);
    });

    test("handles patterns with beforeRender function", () => {
      const epeContent = {
        name: "Animated Pattern",
        sources: {
          main: "var t = 0;\nexport function beforeRender(delta) {\n  t += delta;\n}\nexport function render(index) {\n  hsv(t, 1, 1);\n}",
        },
      };

      const js = extractJavaScriptFromEpe(epeContent);
      expect(js).toContain("beforeRender");
      expect(js).toContain("render");
    });

    test("handles patterns with exported variables and sliders", () => {
      const epeContent = {
        sources: {
          main: "export var speed = 0.1;\nexport function sliderSpeed(v) {\n  speed = v;\n}\nexport function render(index) {\n  hsv(time(speed), 1, 1);\n}",
        },
      };

      const js = extractJavaScriptFromEpe(epeContent);
      expect(js).toContain("export var speed");
      expect(js).toContain("sliderSpeed");
    });
  });

  // =============================================================================
  // 2. FUNCTION NAME COLLISION TESTS
  // =============================================================================

  describe("Function Name Collision Resolution", () => {
    test("detects render function collision between patterns", () => {
      const pattern1 = "export function render(index) { hsv(0.2, 1, 1); }";
      const pattern2 = "export function render(index) { hsv(0.8, 1, 1); }";

      const collisions = detectFunctionCollisions([pattern1, pattern2]);
      expect(collisions).toContain("render");
    });

    test("detects beforeRender function collision", () => {
      const pattern1 =
        "var t1 = 0;\nexport function beforeRender(delta) { t1 += delta; }";
      const pattern2 =
        "var t2 = 0;\nexport function beforeRender(delta) { t2 += delta * 2; }";

      const collisions = detectFunctionCollisions([pattern1, pattern2]);
      expect(collisions).toContain("beforeRender");
    });

    test("detects custom function name collisions", () => {
      const pattern1 =
        "function updateColor() { return 0.5; }\nexport function render(index) { hsv(updateColor(), 1, 1); }";
      const pattern2 =
        "function updateColor() { return 0.8; }\nexport function render(index) { hsv(updateColor(), 1, 1); }";

      const collisions = detectFunctionCollisions([pattern1, pattern2]);
      expect(collisions).toContain("updateColor");
    });

    test("resolves function name collisions with prefixes", () => {
      const pattern1 = "export function render(index) { hsv(0.2, 1, 1); }";
      const pattern2 = "export function render(index) { hsv(0.8, 1, 1); }";

      const resolved = resolveFunctionCollisions(
        [pattern1, pattern2],
        ["pattern1", "pattern2"],
      );

      expect(resolved[0]).toContain("pattern1_render");
      expect(resolved[1]).toContain("pattern2_render");
    });

    test("preserves non-colliding function names", () => {
      const pattern1 =
        "function helper1() { return 0.5; }\nexport function render(index) { hsv(helper1(), 1, 1); }";
      const pattern2 =
        "function helper2() { return 0.8; }\nexport function render(index) { hsv(helper2(), 1, 1); }";

      const resolved = resolveFunctionCollisions(
        [pattern1, pattern2],
        ["p1", "p2"],
      );

      expect(resolved[0]).toContain("helper1"); // Should not be prefixed
      expect(resolved[1]).toContain("helper2"); // Should not be prefixed
      expect(resolved[0]).toContain("p1_render"); // render should be prefixed
      expect(resolved[1]).toContain("p2_render"); // render should be prefixed
    });
  });

  // =============================================================================
  // 3. PATTERN WRAPPING TESTS
  // =============================================================================

  describe("Pattern Wrapping and Constructor Generation", () => {
    test("wraps simple pattern in constructor function", () => {
      const transformedPattern =
        "__state__[0] = 0.5;\nexport function render(__state__, __globals__, index) {\n  hsv(0.2, 1, __state__[0]);\n}";
      const patternName = "TestPattern";

      const wrapped = wrapPatternInConstructor(transformedPattern, patternName);

      expect(wrapped).toContain(`/** ${patternName} **/`);
      expect(wrapped).toContain("() => {");
      expect(wrapped).toContain("var render = render");
      expect(wrapped).toContain(
        "return [render, render2d, render3d, beforeRender, __state__]",
      );
    });

    test("handles pattern with beforeRender function", () => {
      const transformedPattern =
        "__state__[0] = 0;\nexport function beforeRender(__state__, __globals__, delta) {\n  __state__[0] += delta;\n}\nexport function render(__state__, __globals__, index) {\n  hsv(__state__[0], 1, 1);\n}";
      const patternName = "AnimatedPattern";

      const wrapped = wrapPatternInConstructor(transformedPattern, patternName);

      expect(wrapped).toContain("var beforeRender = beforeRender");
      expect(wrapped).toMatch(/beforeRender = function.*delta.*{/);
    });

    test("handles pattern with render2d function", () => {
      const transformedPattern =
        "export function render2d(__state__, __globals__, index, x, y) {\n  hsv(x, y, 1);\n}";
      const patternName = "2DPattern";

      const wrapped = wrapPatternInConstructor(transformedPattern, patternName);

      expect(wrapped).toContain("var render2d = render2d");
      expect(wrapped).toMatch(/render2d = function.*index, x, y.*{/);
    });

    test("initializes state array correctly", () => {
      const transformedPattern =
        "__state__[0] = 0.5;\n__state__[1] = 0.8;\nexport function render(__state__, __globals__, index) {\n  hsv(__state__[0], 1, __state__[1]);\n}";
      const patternName = "StatefulPattern";

      const wrapped = wrapPatternInConstructor(transformedPattern, patternName);

      expect(wrapped).toContain("var __state__ = [0.5, 0.8]");
    });
  });

  // =============================================================================
  // 4. PATTERN COMBINATION TESTS
  // =============================================================================

  describe("Pattern Combination", () => {
    test("combines two simple patterns with ADD blend mode", () => {
      const pattern1Constructor =
        "() => { var render = (index) => { hsv(0.2, 1, 0.5); }; return [render, 0, 0, 0, []]; }";
      const pattern2Constructor =
        "() => { var render = (index) => { hsv(0.8, 1, 0.5); }; return [render, 0, 0, 0, []]; }";

      const combined = combinePatterns(
        [pattern1Constructor, pattern2Constructor],
        ["ADD"],
      );

      expect(combined).toContain("COMBINATOR_ADD");
      expect(combined).toContain("blendColors");
    });

    test("combines patterns with different blend modes", () => {
      const pattern1 = "() => { return [() => hsv(0.2, 1, 1), 0, 0, 0, []]; }";
      const pattern2 = "() => { return [() => hsv(0.8, 1, 1), 0, 0, 0, []]; }";

      const combined = combinePatterns([pattern1, pattern2], ["SUB"]);

      expect(combined).toContain("COMBINATOR_SUB");
    });

    test("handles pattern combination with beforeRender functions", () => {
      const pattern1 =
        "() => { var beforeRender = (delta) => { t += delta; }; return [0, 0, 0, beforeRender, []]; }";
      const pattern2 =
        "() => { var beforeRender = (delta) => { s += delta * 2; }; return [0, 0, 0, beforeRender, []]; }";

      const combined = combinePatterns([pattern1, pattern2], ["AVG"]);

      expect(combined).toContain("beforeRender(delta)");
    });

    test("preserves pattern state isolation in combination", () => {
      const pattern1 =
        "() => { var __state__ = [0.5]; var render = (index) => { __state__[0] *= 0.9; hsv(0.2, 1, __state__[0]); }; return [render, 0, 0, 0, __state__]; }";
      const pattern2 =
        "() => { var __state__ = [0.8]; var render = (index) => { __state__[0] *= 0.95; hsv(0.8, 1, __state__[0]); }; return [render, 0, 0, 0, __state__]; }";

      const combined = combinePatterns([pattern1, pattern2], ["MASK"]);

      expect(combined).toContain("pattern0");
      expect(combined).toContain("pattern1");
    });
  });

  // =============================================================================
  // 5. BLEND MODE TESTS
  // =============================================================================

  describe("Blend Mode Implementation", () => {
    test("ADD blend mode combines RGB values additively", () => {
      const blendFunction = generateBlendFunction("ADD");

      expect(blendFunction).toContain("r = min(1, r1 + r2)");
      expect(blendFunction).toContain("g = min(1, g1 + g2)");
      expect(blendFunction).toContain("b = min(1, b1 + b2)");
    });

    test("SUB blend mode subtracts RGB values", () => {
      const blendFunction = generateBlendFunction("SUB");

      expect(blendFunction).toContain("max(0, r1 - r2)");
      expect(blendFunction).toContain("max(0, g1 - g2)");
      expect(blendFunction).toContain("max(0, b1 - b2)");
    });

    test("AVG blend mode averages RGB values", () => {
      const blendFunction = generateBlendFunction("AVG");

      expect(blendFunction).toContain("(r1 + r2) / 2");
      expect(blendFunction).toContain("(g1 + g2) / 2");
      expect(blendFunction).toContain("(b1 + b2) / 2");
    });

    test("MASK blend mode uses first pattern as mask", () => {
      const blendFunction = generateBlendFunction("MASK");

      expect(blendFunction).toContain("brightness1");
      expect(blendFunction).toContain("r2 * brightness1");
      expect(blendFunction).toContain("g2 * brightness1");
      expect(blendFunction).toContain("b2 * brightness1");
    });
  });

  // =============================================================================
  // 6. INTEGRATION TESTS
  // =============================================================================

  describe("End-to-End Pattern Combination", () => {
    test("combines real Pixelblaze patterns from .epe files", () => {
      const epe1 = {
        name: "Red Pulse",
        sources: {
          main: "var brightness = 0.5;\nexport function beforeRender(delta) {\n  brightness = 0.5 + 0.5 * sin(time(0.1));\n}\nexport function render(index) {\n  hsv(0, 1, brightness);\n}",
        },
      };

      const epe2 = {
        name: "Blue Fade",
        sources: {
          main: "var fade = 1;\nexport function beforeRender(delta) {\n  fade *= 0.99;\n  if (fade < 0.1) fade = 1;\n}\nexport function render(index) {\n  hsv(0.6, 1, fade);\n}",
        },
      };

      const combined = combineEpePatterns([epe1, epe2], ["ADD"]);

      expect(combined).toContain("RedPulse_render");
      expect(combined).toContain("BlueFade_render");
      expect(combined).toContain("COMBINATOR_ADD");
      expect(combined).toContain("__state__");
      expect(combined).toContain("__globals__");
    });

    test("handles complex patterns with multiple functions and variables", () => {
      const complexEpe = {
        name: "Complex Pattern",
        sources: {
          main: "var speed = 0.1;\nvar hue = 0;\nexport var brightness = 1;\nexport function sliderSpeed(v) {\n  speed = v;\n}\nexport function beforeRender(delta) {\n  hue += delta * speed;\n}\nexport function render(index) {\n  hsv(hue % 1, 1, brightness);\n}\nexport function render2d(index, x, y) {\n  hsv((hue + x) % 1, y, brightness);\n}",
        },
      };

      const result = transformAndWrapPattern(complexEpe);

      expect(result).toContain("__state__");
      expect(result).toContain("__globals__");
      expect(result).toContain("var render = render");
      expect(result).toContain("var render2d = render2d");
      expect(result).toContain("var beforeRender = beforeRender");
    });
  });
});

// Export for use in other test files
module.exports = {
  extractJavaScriptFromEpe,
  detectFunctionCollisions,
  resolveFunctionCollisions,
  wrapPatternInConstructor,
  combinePatterns,
  generateBlendFunction,
  combineEpePatterns,
  transformAndWrapPattern,
};
