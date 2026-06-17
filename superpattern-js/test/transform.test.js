const { defineInlineTest } = require("jscodeshift/dist/testUtils");
const transform = require("../src/transform");

// =============================================================================
// BASIC VARIABLE DETECTION TESTS
// =============================================================================

defineInlineTest(
  transform,
  {},
  `var brightness = 0.5;`,
  `__state__[0] = 0.5;`,
  "transforms basic state variable declaration",
);

defineInlineTest(
  transform,
  {},
  `var brightness = 0.5;
var hue = 0.3;`,
  `__state__[0] = 0.5;
__state__[1] = 0.3;`,
  "transforms multiple state variables with correct indices",
);

defineInlineTest(
  transform,
  {},
  `currentHue = time(0.1);`,
  `__globals__[0] = time(0.1);`,
  "transforms basic global variable assignment",
);

defineInlineTest(
  transform,
  {},
  `brightness = 0.8;
currentHue = time(0.1);`,
  `__globals__[0] = 0.8;
__globals__[1] = time(0.1);`,
  "transforms multiple global variables with correct indices",
);

// =============================================================================
// FUNCTION PARAMETER INJECTION TESTS
// =============================================================================

defineInlineTest(
  transform,
  {},
  `export function render(index) {
  hsv(0, 1, 0.5);
}`,
  `export function render(__state__, __globals__, index) {
  hsv(0, 1, 0.5);
}`,
  "adds state and globals parameters to function",
);

defineInlineTest(
  transform,
  {},
  `export function beforeRender(delta) {
  // do something
}`,
  `export function beforeRender(__state__, __globals__, delta) {
  // do something
}`,
  "adds parameters to beforeRender function",
);

defineInlineTest(
  transform,
  {},
  `function render(index, x, y) {
  return index;
}`,
  `function render(__state__, __globals__, index, x, y) {
  return index;
}`,
  "preserves existing parameters when adding state/globals",
);

// =============================================================================
// VARIABLE REFERENCE TRANSFORMATION TESTS
// =============================================================================

defineInlineTest(
  transform,
  {},
  `var brightness = 0.5;
export function render(index) {
  brightness *= 0.99;
  hsv(0, 1, brightness);
}`,
  `__state__[0] = 0.5;
export function render(__state__, __globals__, index) {
  __state__[0] *= 0.99;
  hsv(0, 1, __state__[0]);
}`,
  "transforms state variable references in functions",
);

defineInlineTest(
  transform,
  {},
  `currentHue = 0;
export function render(index) {
  currentHue += 0.01;
  hsv(currentHue, 1, 0.5);
}`,
  `__globals__[0] = 0;
export function render(__state__, __globals__, index) {
  __globals__[0] += 0.01;
  hsv(__globals__[0], 1, 0.5);
}`,
  "transforms global variable references in functions",
);

defineInlineTest(
  transform,
  {},
  `var brightness = 0.5;
hueShift = 0;
export function render(index) {
  brightness *= 0.99;
  hueShift += 0.01;
  hsv(hueShift, 1, brightness);
}`,
  `__state__[0] = 0.5;
__globals__[0] = 0;
export function render(__state__, __globals__, index) {
  __state__[0] *= 0.99;
  __globals__[0] += 0.01;
  hsv(__globals__[0], 1, __state__[0]);
}`,
  "transforms mixed state and global variable references",
);

// =============================================================================
// LOCAL VARIABLE PRESERVATION TESTS
// =============================================================================

defineInlineTest(
  transform,
  {},
  `export function render(index) {
  var localVar = index * 2;
  var brightness = 0.5;
  return localVar;
}`,
  `export function render(__state__, __globals__, index) {
  var localVar = index * 2;
  var brightness = 0.5;
  return localVar;
}`,
  "preserves local variable declarations unchanged",
);

defineInlineTest(
  transform,
  {},
  `export function render(index, x, y) {
  index = index * 2;
  x = x + 1;
  return index + x;
}`,
  `export function render(__state__, __globals__, index, x, y) {
  index = index * 2;
  x = x + 1;
  return index + x;
}`,
  "preserves parameter assignments unchanged",
);

// =============================================================================
// VARIABLE SHADOWING TESTS
// =============================================================================

defineInlineTest(
  transform,
  {},
  `var brightness = 0.5;
export function render(brightness) {
  brightness = 0.8;
  return brightness;
}`,
  `__state__[0] = 0.5;
export function render(__state__, __globals__, brightness) {
  brightness = 0.8;
  return brightness;
}`,
  "parameter shadows state variable - parameter takes precedence",
);

defineInlineTest(
  transform,
  {},
  `hueValue = 0.3;
export function render(index) {
  var hueValue = 0.6;
  hueValue = 0.9;
  return hueValue;
}`,
  `__globals__[0] = 0.3;
export function render(__state__, __globals__, index) {
  var hueValue = 0.6;
  hueValue = 0.9;
  return hueValue;
}`,
  "local variable shadows global - local takes precedence",
);

defineInlineTest(
  transform,
  {},
  `var state = 42;
globalVar = 100;
export function render(state) {
  var globalVar = 200;
  state = 300;
  globalVar = 400;
}
export function other() {
  state = 500;
  globalVar = 600;
}`,
  `__state__[0] = 42;
__globals__[0] = 100;
export function render(__state__, __globals__, state) {
  var globalVar = 200;
  state = 300;
  globalVar = 400;
}
export function other(__state__, __globals__) {
  __state__[0] = 500;
  __globals__[0] = 600;
}`,
  "complex shadowing - local scope takes precedence over global scope",
);

// =============================================================================
// REALISTIC PIXELBLAZE PATTERN TESTS
// =============================================================================

defineInlineTest(
  transform,
  {},
  `var brightness = 0.8;
export function render(index) {
  brightness *= 0.99;
  if (brightness < 0.1) brightness = 1.0;
  hsv(0, 1, brightness);
}`,
  `__state__[0] = 0.8;
export function render(__state__, __globals__, index) {
  __state__[0] *= 0.99;
  if (__state__[0] < 0.1) __state__[0] = 1.0;
  hsv(0, 1, __state__[0]);
}`,
  "realistic fading pattern",
);

defineInlineTest(
  transform,
  {},
  `var values = array(pixelCount);
i = 0;
export function beforeRender(delta) {
  for (i = 0; i < pixelCount; i++) {
    values[i] -= 0.01;
    if (values[i] <= 0) {
      values[i] = random(1);
    }
  }
}
export function render(index) {
  hsv(time(0.1), 1, values[index]);
}`,
  `__state__[0] = array(pixelCount);
__globals__[0] = 0;
export function beforeRender(__state__, __globals__, delta) {
  for (__globals__[0] = 0; __globals__[0] < pixelCount; __globals__[0]++) {
    __state__[0][__globals__[0]] -= 0.01;
    if (__state__[0][__globals__[0]] <= 0) {
      __state__[0][__globals__[0]] = random(1);
    }
  }
}
export function render(__state__, __globals__, index) {
  hsv(time(0.1), 1, __state__[0][index]);
}`,
  "realistic blink fade pattern with arrays",
);

defineInlineTest(
  transform,
  {},
  `export var brightness = 0.5;
export function sliderBrightness(value) {
  brightness = value;
}
export function render(index) {
  hsv(time(0.1), 1, brightness);
}`,
  `__globals__[0] = 0.5;
export function sliderBrightness(__state__, __globals__, value) {
  __globals__[0] = value;
}
export function render(__state__, __globals__, index) {
  hsv(time(0.1), 1, __globals__[0]);
}`,
  "pattern with exported variables and slider controls",
);

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

defineInlineTest(
  transform,
  {},
  `var a, b = 5, c;`,
  `__state__[0] = undefined;
__state__[1] = 5;
__state__[2] = undefined;`,
  "transforms multiple variable declarations in one statement",
);

defineInlineTest(
  transform,
  {},
  `export function render(index) {
  hsv(1, 2, 3);
}`,
  `export function render(__state__, __globals__, index) {
  hsv(1, 2, 3);
}`,
  "handles functions with no variable references",
);

defineInlineTest(
  transform,
  {},
  `var brightness = 0.5;
export function render(index) {
  if (index > 0) {
    var localBrightness = brightness * 0.5;
    brightness = localBrightness;
  }
}`,
  `__state__[0] = 0.5;
export function render(__state__, __globals__, index) {
  if (index > 0) {
    var localBrightness = __state__[0] * 0.5;
    __state__[0] = localBrightness;
  }
}`,
  "handles nested scopes correctly",
);

defineInlineTest(
  transform,
  {},
  `// Pattern: Rainbow Wave
var speed = 0.1;
/* Global hue offset */
hueOffset = 0;

export function beforeRender(delta) {
  hueOffset += delta * speed;
}

export function render(index) {
  hsv((hueOffset + index / pixelCount) % 1, 1, 1);
}`,
  `__state__[0] = 0.1;
/* Global hue offset */
__globals__[0] = 0;

export function beforeRender(__state__, __globals__, delta) {
  __globals__[0] += delta * __state__[0];
}

export function render(__state__, __globals__, index) {
  hsv((__globals__[0] + index / pixelCount) % 1, 1, 1);
}`,
  "preserves comments while transforming variables",
);

// =============================================================================
// ARRAY AND OBJECT TESTS
// =============================================================================

defineInlineTest(
  transform,
  {},
  `var colors = [1, 0, 0];
currentIndex = 0;
export function render(index) {
  colors[currentIndex] = 0.5;
  return colors[0];
}`,
  `__state__[0] = [1, 0, 0];
__globals__[0] = 0;
export function render(__state__, __globals__, index) {
  __state__[0][__globals__[0]] = 0.5;
  return __state__[0][0];
}`,
  "transforms array access with variable references",
);

defineInlineTest(
  transform,
  {},
  `config = { speed: 0.1, brightness: 0.8 };
export function render(index) {
  config.speed = 0.2;
  hsv(0, 1, config.brightness);
}`,
  `__globals__[0] = { speed: 0.1, brightness: 0.8 };
export function render(__state__, __globals__, index) {
  __globals__[0].speed = 0.2;
  hsv(0, 1, __globals__[0].brightness);
}`,
  "transforms object property access",
);

// =============================================================================
// FUNCTION DECLARATION VARIATIONS
// =============================================================================

defineInlineTest(
  transform,
  {},
  `var getValue = function(x) { return x * 2; };
export function render(index) {
  var result = getValue(index);
  hsv(0, 1, result);
}`,
  `__state__[0] = function(__state__, __globals__, x) { return x * 2; };
export function render(__state__, __globals__, index) {
  var result = __state__[0](index);
  hsv(0, 1, result);
}`,
  "transforms function expressions assigned to variables",
);

defineInlineTest(
  transform,
  {},
  `export function render(index) {
  function helper(value) {
    return value * 2;
  }
  var result = helper(index);
  hsv(0, 1, result);
}`,
  `export function render(__state__, __globals__, index) {
  function helper(__state__, __globals__, value) {
    return value * 2;
  }
  var result = helper(__state__, __globals__, index);
  hsv(0, 1, result);
}`,
  "transforms nested function declarations",
);

console.log("🧪 Test suite loaded! Run with: node tests.js");
