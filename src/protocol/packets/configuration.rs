//! Configuration state packets
//!
//! Configuration packets are used during the configuration phase that occurs
//! between login and play states. This phase allows the server to send
//! various configuration data to the client before gameplay begins.

use crate::error::Result;
use crate::protocol::packets::{ClientboundPacket, Packet, ServerboundPacket};
use crate::protocol::types::{McString, VarInt};
use std::io::{Read, Write};

/// Finish Configuration packet (clientbound)
///
/// Sent by the server to notify the client that the configuration process has finished.
/// The client answers with Acknowledge Finish Configuration whenever it is ready to continue.
#[derive(Debug, Clone)]
pub struct FinishConfigurationPacket;

impl Packet for FinishConfigurationPacket {
    const ID: i32 = 0x03;

    fn read<R: Read>(_reader: &mut R) -> Result<Self> {
        Ok(FinishConfigurationPacket)
    }

    fn write<W: Write>(&self, _writer: &mut W) -> Result<()> {
        Ok(())
    }
}

impl ClientboundPacket for FinishConfigurationPacket {}

/// Acknowledge Finish Configuration packet (serverbound)
///
/// Sent by the client to notify the server that the configuration process has finished.
/// It is sent in response to the server's Finish Configuration packet.
#[derive(Debug, Clone)]
pub struct AcknowledgeFinishConfigurationPacket;

impl Packet for AcknowledgeFinishConfigurationPacket {
    const ID: i32 = 0x02;

    fn read<R: Read>(_reader: &mut R) -> Result<Self> {
        Ok(AcknowledgeFinishConfigurationPacket)
    }

    fn write<W: Write>(&self, _writer: &mut W) -> Result<()> {
        Ok(())
    }
}

impl ServerboundPacket for AcknowledgeFinishConfigurationPacket {}

/// Registry Data packet (clientbound)
///
/// Contains registry data for the client to understand game objects.
#[derive(Debug, Clone)]
pub struct RegistryDataPacket {
    /// Registry identifier
    pub registry_id: McString,
    /// Registry entries
    pub entries: Vec<RegistryEntry>,
}

/// Registry entry
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    /// Entry identifier
    pub entry_id: McString,
    /// Whether the entry has data
    pub has_data: bool,
    /// Entry data (if present)
    pub data: Option<Vec<u8>>,
}

impl Packet for RegistryDataPacket {
    const ID: i32 = 0x07;

    fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let registry_id = McString::read(reader)?;
        let entry_count = VarInt::read(reader)?;

        let mut entries = Vec::new();
        for _ in 0..entry_count.0 {
            let entry_id = McString::read(reader)?;
            let has_data = crate::protocol::types::read_bool(reader)?;
            let data = if has_data {
                let data_length = VarInt::read(reader)?;
                let mut data_bytes = vec![0u8; data_length.0 as usize];
                reader.read_exact(&mut data_bytes)?;
                Some(data_bytes)
            } else {
                None
            };

            entries.push(RegistryEntry {
                entry_id,
                has_data,
                data,
            });
        }

        Ok(RegistryDataPacket {
            registry_id,
            entries,
        })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.registry_id.write(writer)?;
        VarInt(self.entries.len() as i32).write(writer)?;

        for entry in &self.entries {
            entry.entry_id.write(writer)?;
            crate::protocol::types::write_bool(entry.has_data, writer)?;
            if let Some(ref data) = entry.data {
                writer.write_all(data)?;
            }
        }

        Ok(())
    }
}

impl ClientboundPacket for RegistryDataPacket {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_registry_data_packet_empty() {
        let packet = RegistryDataPacket {
            registry_id: "minecraft:dimension_type".into(),
            entries: Vec::new(),
        };

        let mut buffer = Vec::new();
        packet.write(&mut buffer).unwrap();

        let mut cursor = Cursor::new(buffer);
        let decoded = RegistryDataPacket::read(&mut cursor).unwrap();

        assert_eq!(packet.registry_id.0, decoded.registry_id.0);
        assert_eq!(packet.entries.len(), decoded.entries.len());
    }

    #[test]
    fn test_finish_configuration_packet() {
        let packet = FinishConfigurationPacket;

        let mut buffer = Vec::new();
        packet.write(&mut buffer).unwrap();

        let mut cursor = Cursor::new(buffer);
        let _decoded = FinishConfigurationPacket::read(&mut cursor).unwrap();

        // Should be empty packet
        assert_eq!(cursor.position(), 0);
    }

    #[test]
    fn test_acknowledge_finish_configuration_packet() {
        let packet = AcknowledgeFinishConfigurationPacket;

        let mut buffer = Vec::new();
        packet.write(&mut buffer).unwrap();

        let mut cursor = Cursor::new(buffer);
        let _decoded = AcknowledgeFinishConfigurationPacket::read(&mut cursor).unwrap();

        // Should be empty packet
        assert_eq!(cursor.position(), 0);
    }
}
