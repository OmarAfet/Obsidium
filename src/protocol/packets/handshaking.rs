//! Handshaking state packets
//!
//! The handshaking state is the initial state of every connection.
//! Only one packet is sent in this state.

use crate::error::Result;
use crate::protocol::packets::{Packet, ServerboundPacket};
use crate::protocol::types::{McString, VarInt};
use std::io::{Read, Write};

/// Handshake packet sent by client to initiate connection
#[derive(Debug, Clone)]
pub struct HandshakePacket {
    /// Protocol version used by the client
    pub protocol_version: VarInt,
    /// Server address (hostname or IP)
    pub server_address: McString,
    /// Server port
    pub server_port: u16,
    /// Next state (1 for status, 2 for login)
    pub next_state: VarInt,
}

impl Packet for HandshakePacket {
    const ID: i32 = 0x00;

    fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let protocol_version = VarInt::read(reader)?;
        let server_address = McString::read(reader)?;

        let mut port_bytes = [0u8; 2];
        reader.read_exact(&mut port_bytes)?;
        let server_port = u16::from_be_bytes(port_bytes);

        let next_state = VarInt::read(reader)?;

        Ok(HandshakePacket {
            protocol_version,
            server_address,
            server_port,
            next_state,
        })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.protocol_version.write(writer)?;
        self.server_address.write(writer)?;
        writer.write_all(&self.server_port.to_be_bytes())?;
        self.next_state.write(writer)?;
        Ok(())
    }
}

impl ServerboundPacket for HandshakePacket {}

/// Possible next states after handshake
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextState {
    /// Status request (server list ping)
    Status = 1,
    /// Login process
    Login = 2,
    /// Transfer process
    Transfer = 3,
}

impl TryFrom<VarInt> for NextState {
    type Error = crate::error::ServerError;

    fn try_from(value: VarInt) -> Result<Self> {
        match value.0 {
            1 => Ok(NextState::Status),
            2 => Ok(NextState::Login),
            3 => Ok(NextState::Transfer),
            _ => Err(crate::error::ServerError::Protocol(format!(
                "Invalid next state: {}",
                value.0
            ))),
        }
    }
}

impl From<NextState> for VarInt {
    fn from(state: NextState) -> Self {
        VarInt(state as i32)
    }
}

/// Legacy Server List Ping packet (serverbound)
///
/// This packet uses a nonstandard format. It is never length-prefixed,
/// and the packet ID is an Unsigned Byte instead of a VarInt.
/// This packet is sent by legacy clients to initiate Server List Ping.
#[derive(Debug, Clone)]
pub struct LegacyServerListPingPacket {
    /// Always 1 (0x01)
    pub payload: u8,
}

impl Packet for LegacyServerListPingPacket {
    const ID: i32 = 0xFE; // Legacy packet ID

    fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let payload = crate::protocol::types::read_unsigned_byte(reader)?;
        Ok(LegacyServerListPingPacket { payload })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        crate::protocol::types::write_unsigned_byte(self.payload, writer)?;
        Ok(())
    }
}

impl ServerboundPacket for LegacyServerListPingPacket {}

impl LegacyServerListPingPacket {
    /// Create a new legacy server list ping packet
    pub fn new() -> Self {
        Self { payload: 1 }
    }
}

impl Default for LegacyServerListPingPacket {
    fn default() -> Self {
        Self::new()
    }
}
