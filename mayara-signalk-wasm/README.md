# Mayara SignalK WASM Plugin

Marine radar plugin for SignalK Server that detects and streams data from marine radars.

This WASM plugin runs directly inside SignalK Server 3.0+, providing radar detection without requiring a separate server process. It uses SignalK's `rawSockets` capability for UDP network access.

## Supported Radars

| Brand | Status | Notes |
|-------|--------|-------|
| **Furuno** | Working | DRS4D-NXT tested, other DRS models should work |
| **Navico** | Planned | BR24, 3G, 4G, HALO series |
| **Raymarine** | Planned | Quantum, RD series |
| **Garmin** | Planned | xHD/HD series |

## Requirements

- SignalK Server 3.0+ with WASM plugin support
- `rawSockets` capability enabled
- Network interface configured for radar communication
- Rust toolchain with `wasm32-wasip1` target (for building from source)

## Network Setup

### Furuno Radars

Furuno radars use a dedicated network subnet. Configure your network interface:

| Setting | Value |
|---------|-------|
| IP Address | `172.31.3.x` (e.g., `172.31.3.1`) |
| Subnet Mask | `255.255.0.0` |
| Broadcast Address | `172.31.255.255` |
| Discovery Port | UDP `10010` |

**Linux configuration:**

```bash
# Temporary - add secondary IP to eth1
sudo ip addr add 172.31.3.1/16 dev eth1

# Permanent - edit /etc/network/interfaces
auto eth1
iface eth1 inet static
    address 172.31.3.1
    netmask 255.255.0.0
```

**Firewall:** Ensure UDP port 10010 is open for both inbound and outbound traffic.

## Building

### Prerequisites

```bash
# Install WASM target
rustup target add wasm32-wasip1
```

### Build

```bash
cd mayara-signalk-wasm

# Release build (recommended)
cargo build --target wasm32-wasip1 --release

# Copy to plugin directory
cp ../target/wasm32-wasip1/release/mayara_signalk_wasm.wasm plugin.wasm
```

Or use the npm script:

```bash
npm run build        # Windows
npm run build:unix   # Linux/macOS
```

## Installation

### Method 1: Manual Install

```bash
# Create plugin directory
mkdir -p ~/.signalk/node_modules/@signalk/mayara-radar

# Copy plugin files
cp plugin.wasm package.json README.md ~/.signalk/node_modules/@signalk/mayara-radar/
```

### Method 2: Symlink (Development)

```bash
cd ~/.signalk/node_modules
mkdir -p @signalk
ln -s /path/to/mayara/mayara-signalk-wasm @signalk/mayara-radar
```

### Restart SignalK

After installation, restart SignalK Server to load the plugin.

## Configuration

1. Open SignalK Admin UI at `http://your-server:3000`
2. Navigate to **Server > Plugin Config > Mayara Marine Radar**
3. Configure options:

| Option | Description | Default |
|--------|-------------|---------|
| Enable Radar Detection | Turn on/off radar scanning | `true` |
| Radar Brand | Select your radar manufacturer | `furuno` |
| Network Interface | Limit to specific interface (optional) | all interfaces |

## SignalK Data Paths

The plugin emits radar information to the SignalK data model:

| Path | Type | Description |
|------|------|-------------|
| `sensors.radar.<id>.brand` | string | Radar manufacturer (e.g., "Furuno") |
| `sensors.radar.<id>.model` | string | Radar model name |
| `sensors.radar.<id>.address` | string | Radar IP address |
| `sensors.radar.<id>.status` | string | Status: `scanning`, `found`, `connected`, `error` |

The `<id>` is derived from the radar's IP address (dots replaced with underscores).

**Example delta:**
```json
{
  "updates": [{
    "values": [
      {"path": "sensors.radar.172_31_6_1.brand", "value": "Furuno"},
      {"path": "sensors.radar.172_31_6_1.model", "value": "DRS4D-NXT"},
      {"path": "sensors.radar.172_31_6_1.address", "value": "172.31.6.1"},
      {"path": "sensors.radar.172_31_6_1.status", "value": "found"}
    ]
  }]
}
```

## How It Works

1. **Plugin Start**: Creates UDP socket, binds to port 10010, enables broadcast
2. **Beacon Transmission**: Sends discovery beacon to `172.31.255.255:10010`
3. **Polling**: SignalK calls `poll()` every second to check for responses
4. **Detection**: Parses radar announcement packets (11-byte header + identity data)
5. **Delta Emission**: Emits radar metadata to SignalK when new radar found

### Furuno Protocol Details

- **Beacon packet**: 16 bytes starting with `01 00 00 01...`
- **Response header**: 11 bytes `01 00 00 01 00 00 00 00 00 01 00`
- **Identity marker**: Byte 16 = `0x52` ('R' for Radar)
- **Model name**: Starts at byte 20 (null-terminated ASCII)

## Differences from Native Mayara

This WASM plugin is focused on radar discovery and status. For full functionality, use `mayara-server`:

| Feature | mayara-server | WASM Plugin |
|---------|---------------|-------------|
| Radar Discovery | Yes | Yes |
| Radar Controls | Yes | No |
| Spoke Data Streaming | Yes | Planned |
| WebSocket Server | Yes | No (uses SignalK) |
| All Brands | Yes | Furuno only* |

*Other brands planned for future releases.

## Development

### Debug Build

```bash
cargo build --target wasm32-wasip1
```

### Check Syntax

```bash
cargo check --target wasm32-wasip1
```

### View Debug Logs

Enable debug logging in SignalK:

```bash
DEBUG=signalk:wasm:* signalk-server
```

You'll see output like:
```
signalk:wasm:mayara-radar Creating radar locator for brand: furuno
signalk:wasm:mayara-radar Bound to port 10010 for beacon responses
signalk:wasm:mayara-radar Sending Furuno beacon request
signalk:wasm:mayara-radar Received 32 bytes from 172.31.6.1:10010
signalk:wasm:mayara-radar Found new radar: Furuno DRS4D-NXT at 172.31.6.1
```

### Architecture

```
mayara-signalk-wasm/
├── src/
│   ├── lib.rs        # Plugin entry point, FFI exports
│   ├── radar.rs      # Radar locator logic
│   └── socket_ffi.rs # UDP socket FFI wrappers
├── Cargo.toml
├── package.json      # SignalK plugin manifest
└── plugin.wasm       # Built WASM binary
```

## Troubleshooting

### Plugin not loading
- Check `package.json` has keywords: `signalk-node-server-plugin`, `signalk-wasm-plugin`
- Verify `plugin.wasm` exists in the plugin directory
- Check SignalK logs for WASM loading errors

### No radar detected
- Verify network configuration (IP in 172.31.x.x range)
- Check firewall allows UDP 10010
- Use `tcpdump -i eth1 port 10010` to verify traffic
- Enable debug logging to see beacon/response activity

### "rawSockets capability not granted"
- Ensure `package.json` has `"wasmCapabilities": { "rawSockets": true }`
- SignalK 3.0+ required for rawSockets support

## Contributing

See the main [Mayara repository](https://github.com/keesverruijt/mayara) for contribution guidelines.

## License

Apache License 2.0
