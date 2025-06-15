# 🛠️ Buntspiel Development Guide

This guide provides comprehensive information for developers working on the Buntspiel project, from initial setup through advanced debugging techniques.

## 🚀 Quick Start for Developers

### Prerequisites

1. **Rust Toolchain**
   ```bash
   # Install rustup if not already installed
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   
   # Install nightly Rust (required for embassy features)
   rustup default nightly
   
   # Add embedded target
   rustup target add thumbv6m-none-eabi
   
   # Install additional tools
   cargo install probe-rs --features cli
   cargo install cargo-binutils
   rustup component add llvm-tools-preview
   ```

2. **Hardware Requirements**
   - Raspberry Pi Pico W
   - Adafruit NeoTrellis 4x4
   - USB-C cable for programming
   - Breadboard and jumper wires (or custom PCB)

3. **Optional Tools**
   - Logic analyzer for I2C debugging
   - Multimeter for power analysis
   - 3D printer for custom enclosures

### Initial Setup

1. **Clone and Build**
   ```bash
   git clone <repository-url>
   cd buntspiel
   
   # Build in debug mode (faster compilation, more debug info)
   cargo build
   
   # Build release mode (optimized for performance)
   cargo build --release
   ```

2. **Hardware Connections**
   ```
   Pico W Pin → NeoTrellis Pin
   ──────────────────────────
   Pin 6 (GP6) → SDA
   Pin 7 (GP7) → SCL  
   3V3 (OUT)   → 3V
   GND         → GND
   ```

3. **Flash Firmware**
   ```bash
   # Hold BOOTSEL button while connecting USB, then:
   cargo run --release
   
   # Or using probe-rs (if you have a debug probe):
   probe-rs run --chip RP2040 target/thumbv6m-none-eabi/release/buntspiel
   ```

## 🏗️ Architecture Deep Dive

### Dual-Core Design

The Buntspiel firmware uses both cores of the RP2040:

**Core 0 (Main):**
- WiFi management and connectivity
- WebSocket client for Pixelblaze communication
- Network protocol handling
- Main application coordination

**Core 1 (LED Control):**
- I2C communication with NeoTrellis
- LED matrix updates and animations
- Button input processing (future)
- Real-time LED effects

### Module Organization

```
src/
├── main.rs           # Application entry point, core coordination
├── wifi.rs           # WiFi connectivity and network management
├── pixelblaze.rs     # WebSocket client and protocol implementation
├── neotrellis.rs     # NeoTrellis I2C driver and LED control
└── animate.rs        # Fallback animations and visual feedback
```

### Inter-Task Communication

```rust
// Channel-based communication between cores
// Core 0 → Core 1: LED frame data
neotrellis::CONTROL_CHANNEL.send(Control::SyncFrame(rgb_data)).await;

// Core 0 internal: Control commands for WebSocket
pixelblaze::PIXELBLAZE_CONTROL_CHANNEL.send(Control::GetConfig).await;
```

## 🔧 Building and Flashing

### Build Configurations

```bash
# Debug build - faster compilation, includes debug symbols
cargo build

# Release build - optimized, smaller binary
cargo build --release

# Check build without linking (faster iteration)
cargo check

# Build with specific features
cargo build --features "feature-name"
```

### Memory Layout

The `memory.x` file defines the memory layout:
```
MEMORY
{
  BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
  FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
  RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}
```

### Flashing Methods

1. **USB Bootloader (Easiest)**
   ```bash
   # Hold BOOTSEL while connecting USB
   cargo run --release
   ```

2. **Debug Probe (Advanced)**
   ```bash
   # Requires SWD debug probe
   probe-rs run --chip RP2040 target/thumbv6m-none-eabi/release/buntspiel
   ```

3. **Custom Scripts**
   ```bash
   # Using picotool (if installed)
   picotool load target/thumbv6m-none-eabi/release/buntspiel.uf2
   ```

## 🐛 Debugging and Testing

### Logging with defmt

The project uses `defmt` for efficient embedded logging:

```rust
use defmt::{info, warn, error, debug};

info!("Connection established: {}", connection_id);
warn!("Frame dropped: channel full");
error!("I2C communication failed: {}", error);
debug!("Processing frame: {:?}", frame_data);
```

### Viewing Logs

```bash
# Using probe-rs (requires debug probe)
probe-rs run --chip RP2040 target/thumbv6m-none-eabi/release/buntspiel

# Using RTT (Real-Time Transfer)
# Logs appear in the terminal when running cargo run
```

### Testing Strategy

1. **Unit Tests**
   ```bash
   # Test superpattern transformation
   cd superpattern
   cargo test
   
   # Test with output
   cargo test -- --nocapture
   
   # Test specific function
   cargo test test_variable_detection
   ```

2. **Integration Testing**
   - Manual hardware testing with real NeoTrellis
   - Network connectivity tests with Pixelblaze
   - Performance testing under various conditions

3. **Hardware-in-the-Loop Testing**
   ```rust
   // Example test structure for embedded testing
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_led_pattern_conversion() {
           let pattern = RGBPattern {
               r1: "x...",
               r2: ".x..",
               r3: "..x.",
               r4: "...x",
           };
           let frame: [Rgb; 16] = pattern.into();
           assert_eq!(frame[0], Rgb { r: 20, g: 20, b: 20 });
       }
   }
   ```

### Common Debugging Scenarios

1. **WiFi Connection Issues**
   ```rust
   // Add detailed logging in wifi.rs
   info!("WiFi scan results: {:?}", scan_results);
   info!("DHCP status: {}", stack.is_config_up());
   ```

2. **I2C Communication Problems**
   ```bash
   # Check I2C bus with logic analyzer
   # Verify pull-up resistors on SDA/SCL (usually internal)
   # Test with simple I2C scanner
   ```

3. **WebSocket Protocol Issues**
   ```rust
   // Log raw WebSocket frames
   info!("WS frame: type={:?} len={} payload={:?}", 
         frame_type, payload_len, &payload[..min(20, payload.len())]);
   ```

## 🎨 Working with the Superpattern System

### Overview

The superpattern system transforms Pixelblaze JavaScript patterns to enable combination and isolation.

### Development Workflow

1. **Add New Patterns**
   ```bash
   # Place .epe files in superpattern/patterns/
   cp new_pattern.epe superpattern/patterns/
   
   # Build to generate transformed versions
   cargo build
   ```

2. **Test Transformations**
   ```rust
   // Add test cases in superpattern/src/lib.rs
   #[test]
   fn test_new_pattern_feature() {
       let code = r#"
           var myVar = 42;
           function render(index) {
               myVar = sin(time(1));
               hsv(myVar, 1, 0.5);
           }
       "#;
       
       let result = transform_pattern(code);
       assert_eq!(result.state_vars, vec!["myVar"]);
       assert!(result.transformed_pattern.contains("__state__[0]"));
   }
   ```

3. **Debug AST Issues**
   ```rust
   // Enable detailed AST logging
   let mut ast_grep = JS.ast_grep(source_code);
   println!("AST: {:#?}", ast_grep.root());
   ```

### Pattern File Format

Pixelblaze patterns are stored as JSON `.epe` files:
```json
{
  "name": "Pattern Name",
  "id": "uniqueId123",
  "sources": {
    "main": "/* JavaScript pattern code */"
  },
  "preview": "base64EncodedImage..."
}
```

## 📊 Performance Optimization

### Memory Usage

1. **Static Allocation**
   ```rust
   // Prefer static allocation over dynamic
   static BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
   let buffer = BUFFER.init([0; 1024]);
   ```

2. **Stack Size Tuning**
   ```rust
   // Adjust stack sizes in main.rs
   static mut CORE1_STACK: embassy_rp::multicore::Stack<4096> = 
       embassy_rp::multicore::Stack::new();
   ```

### Network Performance

1. **Frame Rate Optimization**
   ```rust
   // Monitor frame rates and dropped frames
   info!("FPS: rx={}/s dropped={}/s", rx_fps, dropped_fps);
   ```

2. **Channel Buffer Sizing**
   ```rust
   // Balance memory usage vs. frame dropping
   const MAX_CONTROL: usize = 5; // Adjust based on performance needs
   ```

### Power Optimization

1. **WiFi Power Management**
   ```rust
   control.set_power_management(cyw43::PowerManagementMode::PowerSave).await;
   ```

2. **LED Brightness Control**
   ```rust
   const BRIGHTNESS: u8 = 20; // Reduce for battery operation
   ```

## 🔌 Hardware Integration

### Custom PCB Design

For production hardware, consider:

1. **Power Supply**
   - 3.3V regulator for stable operation
   - Battery management for portable use
   - USB-C connector for programming and power

2. **Signal Integrity**
   - Proper I2C pull-up resistors (typically 4.7kΩ)
   - Decoupling capacitors for power supplies
   - ESD protection on exposed connectors

3. **Mechanical Design**
   - Enclosure design for festival durability
   - Button accessibility and tactile feedback
   - Heat dissipation for LED operation

### Alternative Hardware

The codebase can be adapted for other platforms:

1. **ESP32 Variant**
   - Modify WiFi drivers for ESP32-specific APIs
   - Adjust memory layout and GPIO assignments

2. **Different LED Matrices**
   - Modify `neotrellis.rs` for other I2C LED controllers
   - Adjust pixel count and layout constants

## 🚀 Continuous Integration

### Automated Testing

```yaml
# Example GitHub Actions workflow
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: nightly
          target: thumbv6m-none-eabi
      - name: Build
        run: cargo build --release
      - name: Test
        run: cd superpattern && cargo test
```

### Code Quality

1. **Formatting**
   ```bash
   cargo fmt --check
   ```

2. **Linting**
   ```bash
   cargo clippy -- -D warnings
   ```

3. **Security Audit**
   ```bash
   cargo audit
   ```

## 🤝 Contributing Guidelines

### Code Style

1. **Follow Rust conventions**
   - Use `snake_case` for functions and variables
   - Use `PascalCase` for types and structs
   - Prefer explicit types for public APIs

2. **Documentation**
   ```rust
   /// Brief description of the function
   ///
   /// Longer description with examples if needed.
   ///
   /// # Arguments
   /// * `param` - Description of parameter
   ///
   /// # Returns
   /// Description of return value
   pub fn example_function(param: u32) -> Result<(), Error> {
       // Implementation
   }
   ```

3. **Error Handling**
   ```rust
   // Prefer Result types over panics
   fn fallible_operation() -> Result<Value, Error> {
       // Use ? operator for error propagation
       let result = operation_that_might_fail()?;
       Ok(result)
   }
   ```

### Pull Request Process

1. **Branch Naming**
   - `feature/description` - New features
   - `fix/description` - Bug fixes
   - `docs/description` - Documentation updates

2. **Commit Messages**
   ```
   feat: add support for pattern combination modes
   
   - Implement ADD, SUB, AVG, MASK blend modes
   - Add tests for pattern combination logic
   - Update documentation with usage examples
   ```

3. **Testing Requirements**
   - All new code must include tests
   - Hardware tests should be documented
   - Performance impact should be measured

## 🔍 Troubleshooting

### Common Issues

1. **Build Failures**
   ```bash
   # Clear cargo cache
   cargo clean
   
   # Update dependencies
   cargo update
   
   # Check toolchain version
   rustup show
   ```

2. **Flashing Problems**
   ```bash
   # Reset Pico W to bootloader mode
   # Hold BOOTSEL while connecting USB
   
   # Verify USB connection
   lsusb | grep "Raspberry Pi"
   ```

3. **Runtime Issues**
   ```rust
   // Add debug logging to identify problems
   info!("System state: wifi={} leds={} frames={}", 
         wifi_connected, leds_active, frame_count);
   ```

### Performance Profiling

1. **Memory Usage**
   ```bash
   # Check binary size
   cargo size --release -- -A
   
   # Analyze memory layout
   cargo nm --release | grep -E "(bss|data|rodata)"
   ```

2. **Timing Analysis**
   ```rust
   // Add timing measurements
   let start = embassy_time::Instant::now();
   perform_operation();
   let duration = start.elapsed();
   info!("Operation took: {}ms", duration.as_millis());
   ```

## 📚 Additional Resources

### Documentation Links
- [Embassy Documentation](https://embassy.dev/)
- [Rust Embedded Book](https://doc.rust-lang.org/embedded-book/)
- [RP2040 Datasheet](https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf)
- [Pixelblaze API Documentation](https://github.com/simap/pixelblaze)

### Community Resources
- [Rust Embedded Matrix Chat](https://matrix.to/#/#rust-embedded:matrix.org)
- [Embassy Discord](https://discord.gg/dN6sZjQ)
- [Pixelblaze Community Forum](https://forum.electromage.com/)

### Development Tools
- [probe-rs](https://probe.rs/) - Debugging and flashing
- [RTT Viewer](https://www.segger.com/products/debug-probes/j-link/tools/rtt-viewer/) - Real-time logging
- [Logic Analyzer Software](https://www.saleae.com/) - Protocol debugging