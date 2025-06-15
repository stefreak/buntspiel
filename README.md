# 🏮 Buntspiel - Festival Lighthouse Companion Cube

> A wireless LED companion cube for navigating crowds at Fusion Festival, featuring real-time Pixelblaze pattern visualization and VJ capabilities.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://rustup.rs/)
[![Platform](https://img.shields.io/badge/platform-Raspberry%20Pi%20Pico%20W-green.svg)](https://www.raspberrypi.com/products/raspberry-pi-pico/)

## 🎪 What is Buntspiel?

**Buntspiel** (German for "colorful game") is a companion device for a massive LED lighthouse used as a "Feierstab" (party beacon) at [Fusion Festival](https://www.fusion-festival.de/). When you're lost in a crowd of thousands, this lighthouse helps your group find each other again - and this cube lets you control it remotely!

The lighthouse runs [Pixelblaze](https://www.bhencke.com/pixelblaze), a powerful LED controller with a JavaScript-like programming language for creating stunning light patterns. Buntspiel connects wirelessly to receive real-time pattern previews and provides VJ-style controls for pattern manipulation.

## ✨ Features

### 🔮 Real-time Pattern Visualization
- **4x4 RGB LED Matrix**: Adafruit NeoTrellis displays live previews of lighthouse patterns
- **WebSocket Connection**: Direct communication with Pixelblaze at 192.168.4.1
- **Pattern preview**: 20 FPS pattern preview (limited by Pixelblaze WebSocket protocol)
- **Pattern Switching**: Remote control of active patterns on the lighthouse

### 🎛️ VJ Interface (Under Development)
- **Pattern Combination**: Mix multiple patterns using various blend modes (ADD, SUB, AVG, MASK)
- **Superpattern System**: Advanced AST transformation allows combining incompatible patterns
- **Physical Control**: 16 backlit buttons for tactile pattern manipulation
- **Real-time interaction**: Have fun on the dance floor without using your phone

### 🚀 Tech blah
- **Dual-core Architecture**: WiFi/networking on core 0, LED control on core 1
- **Embassy Framework**: Modern async Rust for embedded systems
- **Robust Networking**: Auto-reconnection, error recovery, connection monitoring
- **Memory Efficient**: `no_std` embedded Rust optimized for microcontroller constraints

## 🛠️ Hardware Requirements

### Core Components
- **Raspberry Pi Pico W** - Main microcontroller with WiFi
- **Adafruit NeoTrellis 4x4** - RGB LED matrix with buttons
- **Power Supply** - USB-C or battery pack

### Connections
```
Pico W -> NeoTrellis
Pin 6  -> SDA (I2C Data)
Pin 7  -> SCL (I2C Clock)
3.3V   -> VCC
GND    -> GND
```

## 🚀 Quick Start

### 1. Hardware Setup
1. Flash the Pico W with the latest firmware
2. Connect the NeoTrellis via I2C (pins 6 & 7)
3. Power up the system

### 2. Software Installation
```bash
# Install Rust nightly toolchain
rustup default nightly
rustup target add thumbv6m-none-eabi

# Clone and build
git clone <your-repo>
cd buntspiel
cargo build --release

# Flash to Pico W
cargo run --release
```

### 3. WiFi Configuration
Edit `src/wifi.rs` to match your network:
```rust
const WIFI_NETWORK: &str = "YourNetworkName";
const WIFI_PASSWORD: &str = "YourPassword";
```

### 4. Connect to Pixelblaze
Ensure your Pixelblaze is running on `192.168.4.1:81` or update the IP in `src/pixelblaze.rs`.

## 🏗️ Architecture

### System Overview
```
┌─────────────────┐    WiFi     ┌──────────────────┐
│   Buntspiel     │◄──────────►     │   Pixelblaze                 │
│   (Pico W)      │  WebSocket             │                              │
└─────────────────┘             └──────────────────┘
        │
        │ I2C
        ▼
┌────────────────────────────────────────────┐
│   NeoTrellis    │   NeoTrellis    │   4x4 Matrix    │   4x4 Matrix    │
│   4x4 Matrix    │   4x4 Matrix    │   4x4 Matrix    │   4x4 Matrix    │
└────────────────────────────────────────────┘
```

### Software Architecture
```
┌─────────────────────────────────────────────────────┐
│                    Core 0 (WiFi)
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐
│  │    WiFi              │  │  Pixelblaze           │  │  WebSocket           │
│  │  Manager             │  │   Protocol            │  │   Client             │
│  └─────────────┘  └──────────────┘  └─────────────┘
└─────────────────────────────────────────────────────┘
                            │
                    Channel Communication
                            │
┌─────────────────────────────────────────────────────┐
│                    Core 1 (LEDs)                    │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │ NeoTrellis           │  │    Frame              │  │  Animation           │ │
│  │   Driver             │  │   Buffer              │  │   Engine             │ │
│  └─────────────┘  └──────────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────┘
```

## 🎨 The Superpattern System

One of Buntspiel's most innovative features is the **Superpattern System** - a solution to Pixelblaze's language limitations that enables combining multiple patterns.

### The Problem
Pixelblaze's JavaScript-like language doesn't support closures or proper variable scoping. Functions can't capture variables from their surrounding scope, making it impossible to run multiple patterns simultaneously without variable name conflicts.

### The Solution
We use **AST (Abstract Syntax Tree) transformation** to rewrite Pixelblaze patterns so they can coexist:

```javascript
// Original Pixelblaze pattern
var brightness = 0.5
currentHue = time(0.1)

export function render(index) {
  brightness *= 0.99
  if (brightness < 0.1) brightness = 1.0
  hsv(currentHue, 1, brightness)
}

// Transformed for combination
__state__[0] = 0.5    // brightness isolated
__globals__[0] = time(0.1)  // currentHue isolated

export function render(__state__, __globals__, index) {
  __state__[0] *= 0.99
  if (__state__[0] < 0.1) __state__[0] = 1.0
  hsv(__globals__[0], 1, __state__[0])
}
```

### Pattern Combination Modes
- **ADD**: Additive blending - colors add together
- **SUB**: Subtractive blending - colors subtract
- **AVG**: Average blending - smooth color mixing
- **MASK**: One pattern masks another

## 📁 Project Structure

```
buntspiel/
├── src/
│   ├── main.rs           # Main application entry point
│   ├── wifi.rs           # WiFi management and connection
│   ├── pixelblaze.rs     # Pixelblaze WebSocket protocol
│   ├── neotrellis.rs     # NeoTrellis LED matrix driver
│   └── animate.rs        # Fallback animations
├── superpattern/         # AST transformation system
│   ├── src/
│   │   ├── lib.rs        # Core transformation logic
│   │   ├── main.js       # Superpattern runtime
│   │   └── pattern_wrapper.js
│   ├── patterns/         # Pixelblaze pattern collection
│   └── generated/        # Transformed patterns
├── cyw43-firmware/       # WiFi firmware blobs
├── Cargo.toml           # Rust dependencies
├── build.rs             # Build script
└── memory.x             # Memory layout
```

## 🔧 Development

### Building
```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Flash to device
cargo run --release
```

### Testing Superpattern System
```bash
cd superpattern
cargo test
```

### Monitoring
Use `defmt` for debugging:
```bash
# View logs via probe-rs
probe-rs run --chip RP2040 target/thumbv6m-none-eabi/release/buntspiel
```

## 🌐 Network Protocol

### Pixelblaze WebSocket Messages

#### Binary Messages (Pattern Data)
- **Type 5**: Preview Frame - RGB data for LED preview
- **Message Format**: `[type_byte, r1, g1, b1, r2, g2, b2, ...]`

#### Text Messages (Control)
```json
{"sendUpdates": true}          // Subscribe to preview frames
{"getConfig": true}            // Get current configuration
{"setActivePattern": "id"}     // Switch active pattern
{"pause": false}               // Resume pattern playback
```

### Frame Processing
- **Frame Rate**: 60+ FPS target
- **Frame Dropping**: Automatic when processing can't keep up
- **Color Mapping**: RGB888 -> NeoTrellis RGB
- **Pixel Mapping**: 16 pixels from Pixelblaze -> 4x4 NeoTrellis

## 🎪 Festival Usage

### Setup at Fusion
1. **Lighthouse Deployment**: Set up main Pixelblaze lighthouse with high-power LEDs
2. **Network**: Create WiFi hotspot or use festival network
3. **Companion Cubes**: Distribute Buntspiel devices to group members
4. **Pattern Library**: Load custom patterns for your group's visual identity

### Crowd Navigation
- **Beacon Mode**: Lighthouse displays bright, recognizable patterns
- **Group Coordination**: Multiple cubes can synchronize patterns
- **Emergency Signaling**: Special patterns for "come back to base"

## 🚧 Current Status

### ✅ Completed
- [x] Basic WiFi connectivity
- [x] Pixelblaze WebSocket protocol implementation
- [x] NeoTrellis LED control
- [x] Real-time pattern preview
- [x] Frame rate monitoring and optimization
- [x] Superpattern AST transformation foundation

### 🔄 In Progress
- [ ] Complete superpattern variable scoping
- [ ] NeoTrellis task integration (see TODO in main.rs)
- [ ] Pattern combination VJ interface
- [ ] Button input handling

### 🎯 Planned Features
- [ ] Battery level monitoring
- [ ] Pattern playlist management
- [ ] Audio reactive patterns
- [ ] Spontaneous VJ fun

## 🤝 Contributing

This project welcomes contributions! Whether you're interested in:
- **Embedded Rust development**
- **LED art and festival culture**
- **JavaScript AST transformation**
- **Hardware hacking**
- **Music visualization**

### Development Setup
1. Install Rust nightly with `thumbv6m-none-eabi` target
2. Get a Raspberry Pi Pico W and NeoTrellis
3. Join the fun!

## 📜 License

Licensed under:
- MIT License ([LICENSE-MIT](LICENSE-MIT))

## 🙏 Acknowledgments

- **Pixelblaze** - For the amazing LED controller and pattern language
- **Embassy** - For making async Rust embedded development possible
- **Adafruit** - For the fantastic NeoTrellis hardware
- **Fusion Festival** - For inspiring creative expression and community
- **Rust Embedded Community** - For the excellent tooling and support
