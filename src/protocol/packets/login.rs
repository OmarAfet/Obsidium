//! Login state packets
//!
//! Login packets handle player authentication and encryption.

use crate::error::Result;
use crate::protocol::packets::{ClientboundPacket, Packet, ServerboundPacket};
use crate::protocol::types::{McString, McUuid, VarInt};
use std::io::{Read, Write};

/// Login start packet (serverbound)
#[derive(Debug, Clone)]
pub struct LoginStartPacket {
    /// Player name
    pub name: McString,
    /// Player UUID
    pub player_uuid: McUuid,
}

impl Packet for LoginStartPacket {
    const ID: i32 = 0x00;

    fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let name = McString::read(reader)?;
        let player_uuid = crate::protocol::types::read_uuid(reader)?;
        Ok(LoginStartPacket { name, player_uuid })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.name.write(writer)?;
        crate::protocol::types::write_uuid(&self.player_uuid, writer)?;
        Ok(())
    }
}

impl ServerboundPacket for LoginStartPacket {}

/// Login success packet (clientbound)
#[derive(Debug, Clone)]
pub struct LoginSuccessPacket {
    /// Player UUID
    pub uuid: McUuid,
    /// Player username
    pub username: McString,
    /// Player properties
    pub properties: Vec<Property>,
}

impl Packet for LoginSuccessPacket {
    const ID: i32 = 0x02;

    fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let uuid = crate::protocol::types::read_uuid(reader)?;
        let username = McString::read(reader)?;

        let properties_count = VarInt::read(reader)?;
        let mut properties = Vec::new();

        for _ in 0..properties_count.0 {
            properties.push(Property::read(reader)?);
        }

        Ok(LoginSuccessPacket {
            uuid,
            username,
            properties,
        })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        crate::protocol::types::write_uuid(&self.uuid, writer)?;
        self.username.write(writer)?;

        VarInt(self.properties.len() as i32).write(writer)?;
        for property in &self.properties {
            property.write(writer)?;
        }

        Ok(())
    }
}

impl ClientboundPacket for LoginSuccessPacket {}

/// Set compression packet (clientbound)
#[derive(Debug, Clone)]
pub struct SetCompressionPacket {
    /// Compression threshold
    pub threshold: VarInt,
}

impl Packet for SetCompressionPacket {
    const ID: i32 = 0x03;

    fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let threshold = VarInt::read(reader)?;
        Ok(SetCompressionPacket { threshold })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.threshold.write(writer)?;
        Ok(())
    }
}

impl ClientboundPacket for SetCompressionPacket {}

/// Login acknowledged packet (serverbound)
#[derive(Debug, Clone)]
pub struct LoginAcknowledgedPacket;

impl Packet for LoginAcknowledgedPacket {
    const ID: i32 = 0x03;

    fn read<R: Read>(_reader: &mut R) -> Result<Self> {
        Ok(LoginAcknowledgedPacket)
    }

    fn write<W: Write>(&self, _writer: &mut W) -> Result<()> {
        Ok(())
    }
}

impl ServerboundPacket for LoginAcknowledgedPacket {}

/// Player property (used in login success)
#[derive(Debug, Clone)]
pub struct Property {
    /// Property name
    pub name: McString,
    /// Property value
    pub value: McString,
    /// Optional signature
    pub signature: Option<McString>,
}

impl Property {
    /// Read a property from a reader
    pub fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let name = McString::read(reader)?;
        let value = McString::read(reader)?;

        let is_signed = crate::protocol::types::read_bool(reader)?;
        let signature = if is_signed {
            Some(McString::read(reader)?)
        } else {
            None
        };

        Ok(Property {
            name,
            value,
            signature,
        })
    }

    /// Write a property to a writer
    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.name.write(writer)?;
        self.value.write(writer)?;

        crate::protocol::types::write_bool(self.signature.is_some(), writer)?;
        if let Some(ref signature) = self.signature {
            signature.write(writer)?;
        }

        Ok(())
    }
}
