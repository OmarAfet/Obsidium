//! Game data management for Obsidium
//!
//! This module handles loading and providing various game data,
//! including Minecraft registries and NBT data conversion.

use crate::error::{Result, ServerError};

/// Registry data manager
pub struct GameData;

impl GameData {
    /// Load game data (simplified to use pre-computed data)
    pub fn load() -> Result<Self> {
        Ok(Self)
    }

    /// Get registry entries for a specific registry type
    pub fn get_registry_entries(&self, registry_name: &str) -> Result<Vec<(String, Vec<u8>)>> {
        match registry_name {
            "minecraft:dimension_type" => {
                let mut entries = Vec::new();

                if let Some(overworld_nbt) =
                    dimension_types::get_dimension_type_nbt("minecraft:overworld")
                {
                    entries.push(("minecraft:overworld".to_string(), overworld_nbt));
                }

                if let Some(nether_nbt) =
                    dimension_types::get_dimension_type_nbt("minecraft:the_nether")
                {
                    entries.push(("minecraft:the_nether".to_string(), nether_nbt));
                }

                if let Some(end_nbt) = dimension_types::get_dimension_type_nbt("minecraft:the_end")
                {
                    entries.push(("minecraft:the_end".to_string(), end_nbt));
                }

                Ok(entries)
            }
            "minecraft:chat_type" => {
                // Return empty for now - chat types are less critical for basic login
                Ok(vec![])
            }
            "minecraft:damage_type" => {
                // Return empty for now - damage types are less critical for basic login
                Ok(vec![])
            }
            "minecraft:worldgen/biome" => {
                // Return empty for now - biomes are less critical for basic login
                Ok(vec![])
            }
            _ => Err(ServerError::Protocol(format!(
                "Registry '{}' not supported",
                registry_name
            ))),
        }
    }

    /// Get the essential registries needed for login
    pub fn get_essential_registries(&self) -> Vec<&'static str> {
        vec!["minecraft:dimension_type"]
    }
}

/// Pre-computed NBT data for essential dimension types
/// This is a fallback in case the JSON parsing fails
pub mod dimension_types {
    use lazy_static::lazy_static;

    lazy_static! {
        /// Pre-computed NBT data for the overworld dimension type
        pub static ref OVERWORLD_NBT: Vec<u8> = {
            // Manually constructed NBT for overworld dimension type
            let mut nbt_data = std::collections::HashMap::new();
            nbt_data.insert("ambient_light".to_string(), fastnbt::Value::Float(0.0));
            nbt_data.insert("bed_works".to_string(), fastnbt::Value::Byte(1));
            nbt_data.insert("coordinate_scale".to_string(), fastnbt::Value::Double(1.0));
            nbt_data.insert("effects".to_string(), fastnbt::Value::String("minecraft:overworld".to_string()));
            nbt_data.insert("has_ceiling".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("has_raids".to_string(), fastnbt::Value::Byte(1));
            nbt_data.insert("has_skylight".to_string(), fastnbt::Value::Byte(1));
            nbt_data.insert("height".to_string(), fastnbt::Value::Int(384));
            nbt_data.insert("infiniburn".to_string(), fastnbt::Value::String("#minecraft:infiniburn_overworld".to_string()));
            nbt_data.insert("logical_height".to_string(), fastnbt::Value::Int(384));
            nbt_data.insert("min_y".to_string(), fastnbt::Value::Int(-64));
            nbt_data.insert("monster_spawn_block_light_limit".to_string(), fastnbt::Value::Int(0));
            nbt_data.insert("monster_spawn_light_level".to_string(), fastnbt::Value::Int(0));
            nbt_data.insert("natural".to_string(), fastnbt::Value::Byte(1));
            nbt_data.insert("piglin_safe".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("respawn_anchor_works".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("ultrawarm".to_string(), fastnbt::Value::Byte(0));

            let compound = fastnbt::Value::Compound(nbt_data);
            fastnbt::to_bytes(&compound).expect("Failed to serialize overworld NBT")
        };

        /// Pre-computed NBT data for the nether dimension type
        pub static ref NETHER_NBT: Vec<u8> = {
            let mut nbt_data = std::collections::HashMap::new();
            nbt_data.insert("ambient_light".to_string(), fastnbt::Value::Float(0.1));
            nbt_data.insert("bed_works".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("coordinate_scale".to_string(), fastnbt::Value::Double(8.0));
            nbt_data.insert("effects".to_string(), fastnbt::Value::String("minecraft:the_nether".to_string()));
            nbt_data.insert("has_ceiling".to_string(), fastnbt::Value::Byte(1));
            nbt_data.insert("has_raids".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("has_skylight".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("height".to_string(), fastnbt::Value::Int(256));
            nbt_data.insert("infiniburn".to_string(), fastnbt::Value::String("#minecraft:infiniburn_nether".to_string()));
            nbt_data.insert("logical_height".to_string(), fastnbt::Value::Int(128));
            nbt_data.insert("min_y".to_string(), fastnbt::Value::Int(0));
            nbt_data.insert("monster_spawn_block_light_limit".to_string(), fastnbt::Value::Int(15));
            nbt_data.insert("monster_spawn_light_level".to_string(), fastnbt::Value::Int(7));
            nbt_data.insert("natural".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("piglin_safe".to_string(), fastnbt::Value::Byte(1));
            nbt_data.insert("respawn_anchor_works".to_string(), fastnbt::Value::Byte(1));
            nbt_data.insert("ultrawarm".to_string(), fastnbt::Value::Byte(1));

            let compound = fastnbt::Value::Compound(nbt_data);
            fastnbt::to_bytes(&compound).expect("Failed to serialize nether NBT")
        };

        /// Pre-computed NBT data for the end dimension type
        pub static ref END_NBT: Vec<u8> = {
            let mut nbt_data = std::collections::HashMap::new();
            nbt_data.insert("ambient_light".to_string(), fastnbt::Value::Float(0.0));
            nbt_data.insert("bed_works".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("coordinate_scale".to_string(), fastnbt::Value::Double(1.0));
            nbt_data.insert("effects".to_string(), fastnbt::Value::String("minecraft:the_end".to_string()));
            nbt_data.insert("has_ceiling".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("has_raids".to_string(), fastnbt::Value::Byte(1));
            nbt_data.insert("has_skylight".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("height".to_string(), fastnbt::Value::Int(256));
            nbt_data.insert("infiniburn".to_string(), fastnbt::Value::String("#minecraft:infiniburn_end".to_string()));
            nbt_data.insert("logical_height".to_string(), fastnbt::Value::Int(256));
            nbt_data.insert("min_y".to_string(), fastnbt::Value::Int(0));
            nbt_data.insert("monster_spawn_block_light_limit".to_string(), fastnbt::Value::Int(0));
            nbt_data.insert("monster_spawn_light_level".to_string(), fastnbt::Value::Int(0));
            nbt_data.insert("natural".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("piglin_safe".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("respawn_anchor_works".to_string(), fastnbt::Value::Byte(0));
            nbt_data.insert("ultrawarm".to_string(), fastnbt::Value::Byte(0));

            let compound = fastnbt::Value::Compound(nbt_data);
            fastnbt::to_bytes(&compound).expect("Failed to serialize end NBT")
        };
    }

    /// Get NBT bytes for a dimension type
    pub fn get_dimension_type_nbt(dimension_name: &str) -> Option<Vec<u8>> {
        match dimension_name {
            "minecraft:overworld" => Some(OVERWORLD_NBT.clone()),
            "minecraft:the_nether" => Some(NETHER_NBT.clone()),
            "minecraft:the_end" => Some(END_NBT.clone()),
            _ => None,
        }
    }
}
