//! Simplified Radar Locator for WASM
//!
//! This is a simplified version of mayara-lib's radar locator that uses
//! SignalK's socket FFI instead of async networking.

use crate::socket_ffi::UdpSocket;
use crate::{debug, emit_delta};

// =============================================================================
// Constants (from mayara-lib/src/brand/furuno/mod.rs)
// =============================================================================

// Furuno
const FURUNO_BEACON_PORT: u16 = 10010;
const FURUNO_BEACON_ADDRESS: &str = "172.31.255.255";

const FURUNO_REQUEST_BEACON_PACKET: [u8; 16] = [
    0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x08, 0x01, 0x00, 0x00, 0x00,
];

const FURUNO_RADAR_REPORT_HEADER: [u8; 11] = [
    0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
];

// Navico (for future expansion)
// const NAVICO_LOCATOR_ADDRESS: &str = "236.6.7.8";
// const NAVICO_LOCATOR_PORT: u16 = 6678;

// =============================================================================
// Radar Info
// =============================================================================

#[derive(Debug, Clone)]
pub struct RadarInfo {
    pub brand: String,
    pub model: String,
    pub serial: String,
    pub address: String,
}

// =============================================================================
// Radar Locator
// =============================================================================

pub struct RadarLocator {
    brand: String,
    socket: UdpSocket,
    radars_found: Vec<RadarInfo>,
    beacon_sent: bool,
}

impl RadarLocator {
    pub fn new(brand: &str, _interface: Option<&str>) -> Result<Self, &'static str> {
        debug(&format!("Creating radar locator for brand: {}", brand));

        let socket = UdpSocket::new_v4()?;

        // Bind to the beacon port to receive radar broadcasts
        // The radar sends responses to 172.31.255.255:10010 (broadcast)
        // so we need to listen on port 10010 to receive them
        socket.bind(FURUNO_BEACON_PORT)?;
        debug(&format!("Bound to port {} for beacon responses", FURUNO_BEACON_PORT));

        // Set broadcast option (SignalK defers this until bind completes)
        socket.set_broadcast(true)?;

        Ok(Self {
            brand: brand.to_lowercase(),
            socket,
            radars_found: Vec::new(),
            beacon_sent: false,
        })
    }

    /// Send beacon request to discover radars
    fn send_beacon(&mut self) -> Result<(), &'static str> {
        match self.brand.as_str() {
            "furuno" => {
                debug("Sending Furuno beacon request");
                self.socket
                    .send_to(&FURUNO_REQUEST_BEACON_PACKET, FURUNO_BEACON_ADDRESS, FURUNO_BEACON_PORT)?;
                self.beacon_sent = true;
                Ok(())
            }
            "navico" => {
                debug("Navico radar discovery not yet implemented in WASM");
                Err("Navico not yet implemented")
            }
            "raymarine" => {
                debug("Raymarine radar discovery not yet implemented in WASM");
                Err("Raymarine not yet implemented")
            }
            "garmin" => {
                debug("Garmin radar discovery not yet implemented in WASM");
                Err("Garmin not yet implemented")
            }
            _ => Err("Unknown radar brand"),
        }
    }

    /// Process received data
    fn process_data(&mut self, data: &[u8], source: &str) -> Option<RadarInfo> {
        match self.brand.as_str() {
            "furuno" => self.process_furuno_data(data, source),
            _ => None,
        }
    }

    fn process_furuno_data(&mut self, data: &[u8], source: &str) -> Option<RadarInfo> {
        // Debug: show first 32 bytes of packet in hex
        let hex_preview: String = data.iter()
            .take(32)
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");
        debug(&format!("Packet from {}: [{}]", source, hex_preview));

        // Check minimum length for a radar report
        if data.len() < FURUNO_RADAR_REPORT_HEADER.len() + 10 {
            debug(&format!("Packet too short: {} bytes", data.len()));
            return None;
        }

        // Check for Furuno radar report header
        if !data.starts_with(&FURUNO_RADAR_REPORT_HEADER) {
            debug("Packet doesn't match Furuno header");
            return None;
        }

        // Additional check: byte 16 should be 'R' (0x52) for radar identity
        if data.len() > 16 && data[16] != b'R' {
            debug(&format!("Byte 16 is {:02x}, not 'R' (52)", data[16]));
            return None;
        }

        debug(&format!(
            "Matched Furuno radar report from {}, {} bytes",
            source,
            data.len()
        ));

        // Parse model name from the packet (simplified)
        // In the real implementation, this would parse the full FurunoRadarReport struct
        let model = if data.len() > 30 {
            // Try to extract model string - this is simplified
            let model_bytes: Vec<u8> = data[20..30]
                .iter()
                .take_while(|&&b| b != 0 && b.is_ascii())
                .cloned()
                .collect();
            String::from_utf8_lossy(&model_bytes).to_string()
        } else {
            "DRS".to_string()
        };

        Some(RadarInfo {
            brand: "Furuno".to_string(),
            model,
            serial: source.to_string(), // Use source IP as identifier for now
            address: source.to_string(),
        })
    }

    /// Poll for radar data - call this periodically
    pub fn poll(&mut self) -> Result<usize, &'static str> {
        // Send beacon periodically (every poll cycle) to discover radars
        // Furuno radars respond to beacon requests, so we need to keep sending them
        self.send_beacon()?;

        // Check for incoming data
        let mut buf = [0u8; 2048];
        let mut addr_buf = [0u8; 46];
        let mut packets_received = 0;

        while let Some((len, addr, port)) = self.socket.recv_from(&mut buf, &mut addr_buf)? {
            packets_received += 1;
            debug(&format!("Received {} bytes from {}:{}", len, addr, port));
            if let Some(radar) = self.process_data(&buf[..len], &addr) {
                // Check if we already know about this radar
                if !self.radars_found.iter().any(|r| r.address == radar.address) {
                    debug(&format!(
                        "Found new radar: {} {} at {}",
                        radar.brand, radar.model, radar.address
                    ));

                    // Emit SignalK delta
                    let delta = format!(
                        r#"{{"updates":[{{"values":[
                            {{"path":"sensors.radar.{}.brand","value":"{}"}},
                            {{"path":"sensors.radar.{}.model","value":"{}"}},
                            {{"path":"sensors.radar.{}.address","value":"{}"}},
                            {{"path":"sensors.radar.{}.status","value":"found"}}
                        ]}}]}}"#,
                        radar.serial.replace('.', "_"),
                        radar.brand,
                        radar.serial.replace('.', "_"),
                        radar.model,
                        radar.serial.replace('.', "_"),
                        radar.address,
                        radar.serial.replace('.', "_")
                    );
                    emit_delta(&delta);

                    self.radars_found.push(radar);
                }
            }
        }

        Ok(self.radars_found.len())
    }

    pub fn radars(&self) -> &[RadarInfo] {
        &self.radars_found
    }
}

impl Drop for RadarLocator {
    fn drop(&mut self) {
        // Socket cleanup is handled by SignalK when plugin stops
        debug("Radar locator dropped");
    }
}
