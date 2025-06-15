# Buntspiel Superpattern System

This directory contains the JavaScript implementation of the Buntspiel Superpattern System, a powerful tool for combining multiple Pixelblaze patterns into a single, unified superpattern. This system is designed to overcome the language limitations of Pixelblaze's JavaScript-like environment, enabling advanced VJ-style pattern mixing and real-time creative control for festival LED art.

## 🚀 Features

- **AST Transformation**: Uses JSCodeshift to transform Pixelblaze patterns, isolating variable scopes and preventing conflicts.
- **Pattern Combination**: Combines multiple patterns with various blend modes (ADD, SUB, AVG, MASK).
- **Function Collision Resolution**: Automatically detects and resolves function name collisions between patterns.
- **State Management**: Preserves the state of each pattern independently, even when combined.
- **Microcontroller Optimized**: Generates compact, efficient code suitable for Raspberry Pi Pico W and other microcontrollers.
- **Festival Ready**: Designed for real-world use at events like Fusion Festival, enabling complex and dynamic LED art.

## 📁 Directory Structure

```
superpattern-js/
├── src/
│   ├── transform.js          # Core JSCodeshift AST transformer
│   ├── parser.js             # .epe file parsing and JS extraction
│   ├── collision-resolver.js # Function collision detection and resolution
│   ├── pattern-wrapper.js    # Wraps transformed patterns in constructors
│   ├── blend-modes.js        # Blend mode implementations
│   ├── combiner.js           # Main pattern combination logic
│   └── index.js              # Main API exports and orchestration
├── test/
│   ├── transform.test.js     # Unit tests for the AST transformer
│   └── combination.test.js   # Integration tests for pattern combination
├── examples/
│   ├── demo.js               # Demonstrates real-world usage
│   └── patterns/             # Example .epe pattern files for testing
├── package.json              # Project configuration and dependencies
├── .gitignore                # Git ignore configuration
└── README.md                 # This file
```

## 🛠️ Usage

### Installation

```bash
npm install
```

### Running the Demo

The demo showcases the full capabilities of the system, from parsing and transforming patterns to combining them with different blend modes. It also generates output files for inspection.

```bash
npm run demo
```

This will:
1. Parse example patterns from the `examples/patterns` directory.
2. Transform each pattern to isolate its scope.
3. Resolve any function name collisions.
4. Combine the patterns into multiple superpatterns with different blend modes.
5. Write the generated superpatterns to the `examples/generated-patterns` directory.

### Running Tests

The project includes a comprehensive test suite to ensure the reliability and correctness of the transformation and combination logic.

```bash
npm test
```

## 🎨 How It Works

The system works in several stages:

1.  **Parsing**: Extracts JavaScript code from `.epe` (Pixelblaze pattern) files.
2.  **Transformation**: Uses a JSCodeshift transformer (`transform.js`) to rewrite the JavaScript AST. This involves:
    -   Converting `var` declarations at the top level to `__state__` array references.
    -   Converting undeclared global variable assignments to `__globals__` array references.
    -   Adding `__state__` and `__globals__` parameters to all functions.
    -   Respecting local variable scope and shadowing.
3.  **Collision Resolution**: Detects function name collisions between patterns and resolves them by adding unique prefixes.
4.  **Wrapping**: Wraps each transformed pattern in a constructor function that initializes its state and returns its render functions.
5.  **Combination**: Combines multiple pattern constructors into a single superpattern. This involves:
    -   Overriding the `hsv()` function to capture the color output of each pattern.
    -   Blending the captured colors using the specified blend mode (ADD, SUB, AVG, MASK).
    -   Generating a unified `render()` and `beforeRender()` function that calls the corresponding functions of each sub-pattern.

## 🏮 Festival Integration

The generated superpatterns can be uploaded directly to a Pixelblaze controller. The Buntspiel hardware project, which uses a Raspberry Pi Pico W and a NeoTrellis keypad, can then be used to:

-   Switch between different superpatterns.
-   Change the active blend mode in real-time.
-   Control pattern parameters (e.g., speed, brightness, color) using sliders and buttons.

This enables a dynamic and interactive VJ experience for controlling large-scale LED installations like the Fusion Festival lighthouse.