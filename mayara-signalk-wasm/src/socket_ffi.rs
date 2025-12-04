//! Socket FFI Wrappers
//!
//! Provides a safe Rust interface to SignalK's UDP socket FFI functions.

// =============================================================================
// Raw FFI Imports
// =============================================================================

#[link(wasm_import_module = "env")]
extern "C" {
    fn sk_udp_create(socket_type: i32) -> i32;
    fn sk_udp_bind(socket_id: i32, port: u16) -> i32;
    fn sk_udp_join_multicast(
        socket_id: i32,
        addr_ptr: *const u8,
        addr_len: usize,
        iface_ptr: *const u8,
        iface_len: usize,
    ) -> i32;
    fn sk_udp_leave_multicast(
        socket_id: i32,
        addr_ptr: *const u8,
        addr_len: usize,
        iface_ptr: *const u8,
        iface_len: usize,
    ) -> i32;
    fn sk_udp_set_multicast_ttl(socket_id: i32, ttl: i32) -> i32;
    fn sk_udp_set_multicast_loopback(socket_id: i32, enabled: i32) -> i32;
    fn sk_udp_set_broadcast(socket_id: i32, enabled: i32) -> i32;
    fn sk_udp_send(
        socket_id: i32,
        addr_ptr: *const u8,
        addr_len: usize,
        port: u16,
        data_ptr: *const u8,
        data_len: usize,
    ) -> i32;
    fn sk_udp_recv(
        socket_id: i32,
        buf_ptr: *mut u8,
        buf_max_len: usize,
        addr_out_ptr: *mut u8,
        port_out_ptr: *mut u16,
    ) -> i32;
    fn sk_udp_pending(socket_id: i32) -> i32;
    fn sk_udp_close(socket_id: i32);
}

// =============================================================================
// Socket Type
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Udp4 = 0,
    Udp6 = 1,
}

// =============================================================================
// UdpSocket Wrapper
// =============================================================================

/// A UDP socket managed by SignalK's socket manager
#[derive(Debug)]
pub struct UdpSocket {
    id: i32,
}

impl UdpSocket {
    /// Create a new UDP socket
    pub fn new(socket_type: SocketType) -> Result<Self, &'static str> {
        let id = unsafe { sk_udp_create(socket_type as i32) };
        if id < 0 {
            Err("Failed to create socket")
        } else {
            Ok(Self { id })
        }
    }

    /// Create a new IPv4 UDP socket
    pub fn new_v4() -> Result<Self, &'static str> {
        Self::new(SocketType::Udp4)
    }

    /// Create a new IPv6 UDP socket
    pub fn new_v6() -> Result<Self, &'static str> {
        Self::new(SocketType::Udp6)
    }

    /// Get the socket ID
    pub fn id(&self) -> i32 {
        self.id
    }

    /// Bind the socket to a port
    /// Use port 0 to bind to any available port
    pub fn bind(&self, port: u16) -> Result<(), &'static str> {
        let result = unsafe { sk_udp_bind(self.id, port) };
        if result < 0 {
            Err("Failed to bind socket")
        } else {
            Ok(())
        }
    }

    /// Join a multicast group
    pub fn join_multicast(&self, group: &str, interface: Option<&str>) -> Result<(), &'static str> {
        let (iface_ptr, iface_len) = match interface {
            Some(iface) => (iface.as_ptr(), iface.len()),
            None => (std::ptr::null(), 0),
        };

        let result = unsafe {
            sk_udp_join_multicast(self.id, group.as_ptr(), group.len(), iface_ptr, iface_len)
        };

        if result < 0 {
            Err("Failed to join multicast group")
        } else {
            Ok(())
        }
    }

    /// Leave a multicast group
    pub fn leave_multicast(
        &self,
        group: &str,
        interface: Option<&str>,
    ) -> Result<(), &'static str> {
        let (iface_ptr, iface_len) = match interface {
            Some(iface) => (iface.as_ptr(), iface.len()),
            None => (std::ptr::null(), 0),
        };

        let result = unsafe {
            sk_udp_leave_multicast(self.id, group.as_ptr(), group.len(), iface_ptr, iface_len)
        };

        if result < 0 {
            Err("Failed to leave multicast group")
        } else {
            Ok(())
        }
    }

    /// Set multicast TTL
    pub fn set_multicast_ttl(&self, ttl: u8) -> Result<(), &'static str> {
        let result = unsafe { sk_udp_set_multicast_ttl(self.id, ttl as i32) };
        if result < 0 {
            Err("Failed to set multicast TTL")
        } else {
            Ok(())
        }
    }

    /// Enable or disable multicast loopback
    pub fn set_multicast_loopback(&self, enabled: bool) -> Result<(), &'static str> {
        let result = unsafe { sk_udp_set_multicast_loopback(self.id, enabled as i32) };
        if result < 0 {
            Err("Failed to set multicast loopback")
        } else {
            Ok(())
        }
    }

    /// Enable or disable broadcast
    pub fn set_broadcast(&self, enabled: bool) -> Result<(), &'static str> {
        let result = unsafe { sk_udp_set_broadcast(self.id, enabled as i32) };
        if result < 0 {
            Err("Failed to set broadcast")
        } else {
            Ok(())
        }
    }

    /// Send data to a specific address and port
    pub fn send_to(&self, data: &[u8], addr: &str, port: u16) -> Result<usize, &'static str> {
        let result =
            unsafe { sk_udp_send(self.id, addr.as_ptr(), addr.len(), port, data.as_ptr(), data.len()) };

        if result < 0 {
            Err("Failed to send data")
        } else {
            Ok(result as usize)
        }
    }

    /// Receive data (non-blocking)
    /// Returns Ok(Some((data_len, source_addr, source_port))) if data available
    /// Returns Ok(None) if no data available
    /// Returns Err on error
    pub fn recv_from(
        &self,
        buf: &mut [u8],
        addr_buf: &mut [u8; 46],
    ) -> Result<Option<(usize, String, u16)>, &'static str> {
        let mut port: u16 = 0;

        let result = unsafe {
            sk_udp_recv(
                self.id,
                buf.as_mut_ptr(),
                buf.len(),
                addr_buf.as_mut_ptr(),
                &mut port,
            )
        };

        if result < 0 {
            Err("Failed to receive data")
        } else if result == 0 {
            Ok(None) // No data available
        } else {
            // Parse null-terminated address string
            let addr_str = {
                let null_pos = addr_buf.iter().position(|&b| b == 0).unwrap_or(addr_buf.len());
                std::str::from_utf8(&addr_buf[..null_pos])
                    .unwrap_or("")
                    .to_string()
            };

            Ok(Some((result as usize, addr_str, port)))
        }
    }

    /// Get number of pending datagrams
    pub fn pending(&self) -> usize {
        let result = unsafe { sk_udp_pending(self.id) };
        if result < 0 {
            0
        } else {
            result as usize
        }
    }

    /// Check if data is available
    pub fn has_data(&self) -> bool {
        self.pending() > 0
    }

    /// Close the socket
    pub fn close(&self) {
        unsafe { sk_udp_close(self.id) }
    }
}

// Note: We don't implement Drop because SignalK will clean up sockets
// when the plugin stops. Implementing Drop could cause double-free issues.

// =============================================================================
// Datagram Helper
// =============================================================================

/// Received datagram with source information
#[derive(Debug)]
pub struct Datagram {
    pub data: Vec<u8>,
    pub source_addr: String,
    pub source_port: u16,
}

impl UdpSocket {
    /// Receive a datagram (non-blocking, allocates)
    pub fn recv_datagram(&self) -> Result<Option<Datagram>, &'static str> {
        let mut buf = [0u8; 65536]; // Max UDP datagram size
        let mut addr_buf = [0u8; 46];

        match self.recv_from(&mut buf, &mut addr_buf)? {
            Some((len, addr, port)) => Ok(Some(Datagram {
                data: buf[..len].to_vec(),
                source_addr: addr,
                source_port: port,
            })),
            None => Ok(None),
        }
    }
}
