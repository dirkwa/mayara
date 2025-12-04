//! Mayara SignalK WASM Plugin
//!
//! Marine radar plugin for SignalK Server that detects and streams radar data
//! from Furuno, Navico, Raymarine, and Garmin marine radars.

mod socket_ffi;
mod radar;

use std::alloc::{alloc, dealloc, Layout};
use std::slice;

// =============================================================================
// SignalK FFI Imports
// =============================================================================

#[link(wasm_import_module = "env")]
extern "C" {
    fn sk_debug(ptr: *const u8, len: usize);
    fn sk_set_status(ptr: *const u8, len: usize);
    fn sk_set_error(ptr: *const u8, len: usize);
    fn sk_handle_message(ptr: *const u8, len: usize);
    fn sk_has_capability(ptr: *const u8, len: usize) -> i32;
}

// =============================================================================
// Helper Functions
// =============================================================================

fn debug(msg: &str) {
    unsafe {
        sk_debug(msg.as_ptr(), msg.len());
    }
}

fn set_status(msg: &str) {
    unsafe {
        sk_set_status(msg.as_ptr(), msg.len());
    }
}

fn set_error(msg: &str) {
    unsafe {
        sk_set_error(msg.as_ptr(), msg.len());
    }
}

fn emit_delta(delta_json: &str) {
    unsafe {
        sk_handle_message(delta_json.as_ptr(), delta_json.len());
    }
}

fn has_capability(cap: &str) -> bool {
    unsafe { sk_has_capability(cap.as_ptr(), cap.len()) == 1 }
}

fn write_string_to_buffer(s: &str, out_ptr: *mut u8, max_len: usize) -> i32 {
    let bytes = s.as_bytes();
    let len = bytes.len().min(max_len);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_ptr, len);
    }
    len as i32
}

// =============================================================================
// Plugin State
// =============================================================================

static mut PLUGIN_STATE: Option<PluginState> = None;

struct PluginState {
    config: PluginConfig,
    radar_locator: Option<radar::RadarLocator>,
}

#[derive(Default, serde::Deserialize)]
struct PluginConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_brand")]
    brand: String,
    #[serde(default)]
    interface: Option<String>,
}

fn default_brand() -> String {
    "furuno".to_string()
}

// =============================================================================
// Required Plugin Exports
// =============================================================================

const PLUGIN_ID: &str = "mayara-radar";
const PLUGIN_NAME: &str = "Mayara Marine Radar";

const PLUGIN_SCHEMA: &str = r#"{
  "type": "object",
  "title": "Mayara Radar Configuration",
  "properties": {
    "enabled": {
      "type": "boolean",
      "title": "Enable Radar Detection",
      "default": true
    },
    "brand": {
      "type": "string",
      "title": "Radar Brand",
      "enum": ["furuno", "navico", "raymarine", "garmin"],
      "default": "furuno"
    },
    "interface": {
      "type": "string",
      "title": "Network Interface (optional)",
      "description": "Leave empty to use all interfaces"
    }
  }
}"#;

#[no_mangle]
pub extern "C" fn plugin_id(out_ptr: *mut u8, max_len: usize) -> i32 {
    write_string_to_buffer(PLUGIN_ID, out_ptr, max_len)
}

#[no_mangle]
pub extern "C" fn plugin_name(out_ptr: *mut u8, max_len: usize) -> i32 {
    write_string_to_buffer(PLUGIN_NAME, out_ptr, max_len)
}

#[no_mangle]
pub extern "C" fn plugin_schema(out_ptr: *mut u8, max_len: usize) -> i32 {
    write_string_to_buffer(PLUGIN_SCHEMA, out_ptr, max_len)
}

#[no_mangle]
pub extern "C" fn plugin_start(config_ptr: *const u8, config_len: usize) -> i32 {
    debug("Mayara radar plugin starting...");

    // Check rawSockets capability
    if !has_capability("rawSockets") {
        set_error("rawSockets capability not granted - cannot detect radars");
        return 1;
    }
    debug("rawSockets capability confirmed");

    // Parse configuration
    let config_str = unsafe {
        let slice = slice::from_raw_parts(config_ptr, config_len);
        match std::str::from_utf8(slice) {
            Ok(s) => s,
            Err(_) => {
                set_error("Invalid UTF-8 in configuration");
                return 1;
            }
        }
    };

    let config: PluginConfig = match serde_json::from_str(config_str) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Failed to parse config: {}", e);
            set_error(&msg);
            return 1;
        }
    };

    debug(&format!("Config: brand={}, enabled={}", config.brand, config.enabled));

    if !config.enabled {
        set_status("Disabled");
        return 0;
    }

    // Initialize radar locator
    let locator = match radar::RadarLocator::new(&config.brand, config.interface.as_deref()) {
        Ok(l) => l,
        Err(e) => {
            let msg = format!("Failed to initialize radar locator: {}", e);
            set_error(&msg);
            return 1;
        }
    };

    // Store state
    unsafe {
        PLUGIN_STATE = Some(PluginState {
            config,
            radar_locator: Some(locator),
        });
    }

    set_status("Scanning for radars...");

    // Emit initial delta
    let delta = r#"{"updates":[{"values":[{"path":"sensors.radar.mayara.status","value":"scanning"}]}]}"#;
    emit_delta(delta);

    // Do an initial poll to send the first beacon request immediately
    debug("Sending initial beacon request...");
    unsafe {
        if let Some(ref mut state) = PLUGIN_STATE {
            if let Some(ref mut locator) = state.radar_locator {
                match locator.poll() {
                    Ok(_) => debug("Initial beacon sent"),
                    Err(e) => debug(&format!("Initial poll error: {}", e)),
                }
            }
        }
    }

    debug("Mayara radar plugin started successfully");
    0
}

#[no_mangle]
pub extern "C" fn plugin_stop() -> i32 {
    debug("Mayara radar plugin stopping...");

    unsafe {
        if let Some(state) = PLUGIN_STATE.take() {
            // Radar locator will be dropped, sockets closed by SignalK
            drop(state);
        }
    }

    set_status("Stopped");
    debug("Mayara radar plugin stopped");
    0
}

// =============================================================================
// Memory Allocation (required for SignalK string passing)
// =============================================================================

#[no_mangle]
pub extern "C" fn allocate(size: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::null_mut();
    }
    let layout = Layout::from_size_align(size, 1).unwrap();
    unsafe { alloc(layout) }
}

#[no_mangle]
pub extern "C" fn deallocate(ptr: *mut u8, size: usize) {
    if ptr.is_null() || size == 0 {
        return;
    }
    let layout = Layout::from_size_align(size, 1).unwrap();
    unsafe { dealloc(ptr, layout) }
}

// =============================================================================
// Polling Function (called periodically by host)
// =============================================================================

/// Generic poll function - called periodically by SignalK host (every 1 second)
/// This is a standard WASM plugin export for plugins that need periodic execution.
/// Returns 0 on success, non-zero on error or to signal events
#[no_mangle]
pub extern "C" fn poll() -> i32 {
    unsafe {
        if let Some(ref mut state) = PLUGIN_STATE {
            if let Some(ref mut locator) = state.radar_locator {
                match locator.poll() {
                    Ok(radar_count) => {
                        if radar_count > 0 {
                            set_status(&format!("Found {} radar(s)", radar_count));
                        }
                        return radar_count as i32;
                    }
                    Err(e) => {
                        debug(&format!("Poll error: {}", e));
                        return -1;
                    }
                }
            }
        }
    }
    0
}
