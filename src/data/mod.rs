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
            "minecraft:dimension_type" => Ok(dimension_types::get_all_dimension_types()),
            "minecraft:worldgen/biome" => Ok(biomes::get_all_biomes()),
            "minecraft:chat_type" => Ok(chat_types::get_all_chat_types()),
            "minecraft:damage_type" => Ok(damage_types::get_all_damage_types()),
            _ => Err(ServerError::Protocol(format!(
                "Registry '{}' not supported",
                registry_name
            ))),
        }
    }

    /// Get the essential registries needed for login
    pub fn get_essential_registries(&self) -> Vec<&'static str> {
        vec![
            "minecraft:dimension_type",
            "minecraft:worldgen/biome",
            "minecraft:chat_type",
            "minecraft:damage_type",
        ]
    }
}

/// Helper function to convert JSON to NBT bytes
fn json_to_nbt_bytes(json_value: &serde_json::Value) -> Result<Vec<u8>> {
    // Convert JSON Value to fastnbt Value
    let nbt_value = json_to_fastnbt_value(json_value)?;

    // Serialize using fastnbt's proper NBT format.
    // For registry data, the NBT is expected to be a CompoundTag
    // whose contents are directly embedded, without its tag ID or name.
    let mut buffer = Vec::new();

    match nbt_value {
        fastnbt::Value::Compound(compound) => {
            // For registry data, we expect the data field to contain the NBT contents *without*
            // the root TAG_Compound ID (0x0A) and its empty name (0x00 0x00).
            // We'll manually serialize the compound's contents.
            write_compound_contents(&mut buffer, &compound)?;
        },
        _ => {
            // If it's not a compound (which most registry entries are), or for safety
            // fall back to writing a full NBT blob, though this might still be incorrect
            // for some contexts if the client strictly expects contents of a compound.
            fastnbt::to_writer(&mut buffer, &nbt_value)
                .map_err(|e| ServerError::Protocol(format!("Failed to serialize non-compound NBT: {}", e)))?;
        }
    }

    Ok(buffer)
}

/// Write the contents of a compound tag without the tag ID and name
fn write_compound_contents(buffer: &mut Vec<u8>, compound: &std::collections::HashMap<String, fastnbt::Value>) -> Result<()> {
    for (key, value) in compound {
        // Write tag ID
        write_tag_id(buffer, value);
        
        // Write name length and name
        let name_bytes = key.as_bytes();
        buffer.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
        buffer.extend_from_slice(name_bytes);
        
        // Write value
        write_tag_payload(buffer, value)?;
    }
    
    // Write end tag (TAG_End = 0)
    buffer.push(0);
    
    Ok(())
}

/// Write the tag ID for a given NBT value
fn write_tag_id(buffer: &mut Vec<u8>, value: &fastnbt::Value) {
    let tag_id = match value {
        fastnbt::Value::Byte(_) => 1,
        fastnbt::Value::Short(_) => 2,
        fastnbt::Value::Int(_) => 3,
        fastnbt::Value::Long(_) => 4,
        fastnbt::Value::Float(_) => 5,
        fastnbt::Value::Double(_) => 6,
        fastnbt::Value::ByteArray(_) => 7,
        fastnbt::Value::String(_) => 8,
        fastnbt::Value::List(_) => 9,
        fastnbt::Value::Compound(_) => 10,
        fastnbt::Value::IntArray(_) => 11,
        fastnbt::Value::LongArray(_) => 12,
    };
    buffer.push(tag_id);
}

/// Write the payload (value) of an NBT tag
fn write_tag_payload(buffer: &mut Vec<u8>, value: &fastnbt::Value) -> Result<()> {
    match value {
        fastnbt::Value::Byte(b) => buffer.push(*b as u8),
        fastnbt::Value::Short(s) => buffer.extend_from_slice(&s.to_be_bytes()),
        fastnbt::Value::Int(i) => buffer.extend_from_slice(&i.to_be_bytes()),
        fastnbt::Value::Long(l) => buffer.extend_from_slice(&l.to_be_bytes()),
        fastnbt::Value::Float(f) => buffer.extend_from_slice(&f.to_be_bytes()),
        fastnbt::Value::Double(d) => buffer.extend_from_slice(&d.to_be_bytes()),
        fastnbt::Value::String(s) => {
            let bytes = s.as_bytes();
            buffer.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
            buffer.extend_from_slice(bytes);
        },
        fastnbt::Value::ByteArray(arr) => {
            buffer.extend_from_slice(&(arr.len() as i32).to_be_bytes());
            for byte in arr.iter() {
                buffer.push(*byte as u8);
            }
        },
        fastnbt::Value::IntArray(arr) => {
            buffer.extend_from_slice(&(arr.len() as i32).to_be_bytes());
            for int in arr.iter() {
                buffer.extend_from_slice(&int.to_be_bytes());
            }
        },
        fastnbt::Value::LongArray(arr) => {
            buffer.extend_from_slice(&(arr.len() as i32).to_be_bytes());
            for long in arr.iter() {
                buffer.extend_from_slice(&long.to_be_bytes());
            }
        },
        fastnbt::Value::List(list) => {
            if list.is_empty() {
                buffer.push(0); // TAG_End for empty list
                buffer.extend_from_slice(&0i32.to_be_bytes()); // Length 0
            } else {
                // Write the tag type of list elements
                write_tag_id(buffer, &list[0]);
                // Write list length
                buffer.extend_from_slice(&(list.len() as i32).to_be_bytes());
                // Write each element's payload (without tag ID or name)
                for item in list {
                    write_tag_payload(buffer, item)?;
                }
            }
        },
        fastnbt::Value::Compound(compound) => {
            write_compound_contents(buffer, compound)?;
        },
    }
    Ok(())
}

/// Convert a JSON value to a fastnbt Value
fn json_to_fastnbt_value(json_value: &serde_json::Value) -> Result<fastnbt::Value> {
    match json_value {
        serde_json::Value::Null => {
            // NBT doesn't have a direct null. Represent as empty string for registry data.
            Ok(fastnbt::Value::String("".to_string()))
        }
        serde_json::Value::Bool(b) => {
            // Booleans are almost always TAG_Byte in NBT
            Ok(fastnbt::Value::Byte(if *b { 1 } else { 0 }))
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                // Try to fit in smallest possible integer type first, as per NBT spec's typical usage.
                if i >= i8::MIN as i64 && i <= i8::MAX as i64 {
                    Ok(fastnbt::Value::Byte(i as i8))
                } else if i >= i16::MIN as i64 && i <= i16::MAX as i64 {
                    Ok(fastnbt::Value::Short(i as i16))
                } else if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    Ok(fastnbt::Value::Int(i as i32))
                } else {
                    Ok(fastnbt::Value::Long(i))
                }
            } else if let Some(f) = n.as_f64() {
                // Determine if it should be a Float or Double
                // Floats have less precision. Use if value fits float range.
                if f.abs() <= f32::MAX as f64 && f.abs() >= f32::MIN as f64 || f == 0.0 {
                    Ok(fastnbt::Value::Float(f as f32))
                } else {
                    Ok(fastnbt::Value::Double(f))
                }
            } else {
                Err(ServerError::Protocol(
                    "Invalid number format in JSON NBT conversion".to_string(),
                ))
            }
        }
        serde_json::Value::String(s) => Ok(fastnbt::Value::String(s.clone())),
        serde_json::Value::Array(arr) => {
            // Handle specific NBT array types if the context expects them,
            // otherwise, default to a generic NBT List of values.
            // This is the tricky part. Without schema information, inferring
            // Byte/Int/Long arrays from arbitrary JSON arrays is heuristic.
            // A safer default is a generic List, and let the NBT library
            // handle the element types.

            if arr.is_empty() {
                return Ok(fastnbt::Value::List(Vec::new()));
            }

            // Try to detect primitive arrays (Byte/Int/Long arrays)
            let mut all_bytes = true;
            let mut all_ints = true;
            let mut all_longs = true;
            let mut generic_list_elements = Vec::new();

            for item in arr {
                let converted_item = json_to_fastnbt_value(item)?;
                
                if !matches!(converted_item, fastnbt::Value::Byte(_)) { all_bytes = false; }
                if !matches!(converted_item, fastnbt::Value::Int(_)) { all_ints = false; }
                if !matches!(converted_item, fastnbt::Value::Long(_)) { all_longs = false; }

                generic_list_elements.push(converted_item);
            }

            if all_bytes {
                let bytes: Vec<i8> = generic_list_elements.into_iter()
                    .map(|v| match v { fastnbt::Value::Byte(b) => b, _ => 0 }) // Should not hit `_` if `all_bytes` is true
                    .collect();
                Ok(fastnbt::Value::ByteArray(fastnbt::ByteArray::new(bytes)))
            } else if all_ints {
                let ints: Vec<i32> = generic_list_elements.into_iter()
                    .map(|v| match v { fastnbt::Value::Int(i) => i, _ => 0 })
                    .collect();
                Ok(fastnbt::Value::IntArray(fastnbt::IntArray::new(ints)))
            } else if all_longs {
                let longs: Vec<i64> = generic_list_elements.into_iter()
                    .map(|v| match v { fastnbt::Value::Long(l) => l, _ => 0 })
                    .collect();
                Ok(fastnbt::Value::LongArray(fastnbt::LongArray::new(longs)))
            } else {
                // If not a primitive array, it's a generic list (TAG_List)
                Ok(fastnbt::Value::List(generic_list_elements))
            }
        }
        serde_json::Value::Object(obj) => {
            let mut nbt_compound = std::collections::HashMap::new();
            for (key, value) in obj {
                nbt_compound.insert(key.clone(), json_to_fastnbt_value(value)?);
            }
            Ok(fastnbt::Value::Compound(nbt_compound))
        }
    }
}

/// Registry data modules
/// Dimension type registry data
pub mod dimension_types {
    use super::*;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref REGISTRY_DATA: std::collections::HashMap<&'static str, serde_json::Value> = {
            let json_str = include_str!("registry_data.json");
            let full_data: serde_json::Value =
                serde_json::from_str(json_str).expect("Failed to parse registry_data.json");

            let dimension_data = full_data["minecraft:dimension_type"]
                .as_object()
                .expect("dimension_type registry not found");

            let mut map = std::collections::HashMap::new();
            for (key, value) in dimension_data {
                let static_key: &'static str = Box::leak(key.clone().into_boxed_str());
                map.insert(static_key, value.clone());
            }
            map
        };
    }

    /// Get all dimension type registry entries as NBT data
    pub fn get_all_dimension_types() -> Vec<(String, Vec<u8>)> {
        let mut entries = Vec::new();

        for (name, data) in REGISTRY_DATA.iter() {
            match json_to_nbt_bytes(data) {
                Ok(nbt_bytes) => {
                    entries.push((name.to_string(), nbt_bytes));
                }
                Err(e) => {
                    tracing::warn!("Failed to convert dimension type {} to NBT: {}", name, e);
                }
            }
        }

        entries
    }
}

/// Biome registry data
pub mod biomes {
    use super::*;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref REGISTRY_DATA: std::collections::HashMap<&'static str, serde_json::Value> = {
            let json_str = include_str!("registry_data.json");
            let full_data: serde_json::Value =
                serde_json::from_str(json_str).expect("Failed to parse registry_data.json");

            let biome_data = full_data["minecraft:worldgen/biome"]
                .as_object()
                .expect("worldgen/biome registry not found");

            let mut map = std::collections::HashMap::new();
            for (key, value) in biome_data {
                let static_key: &'static str = Box::leak(key.clone().into_boxed_str());
                map.insert(static_key, value.clone());
            }
            map
        };
    }

    /// Get all biome registry entries as NBT data
    pub fn get_all_biomes() -> Vec<(String, Vec<u8>)> {
        let mut entries = Vec::new();

        for (name, data) in REGISTRY_DATA.iter() {
            match json_to_nbt_bytes(data) {
                Ok(nbt_bytes) => {
                    entries.push((name.to_string(), nbt_bytes));
                }
                Err(e) => {
                    tracing::warn!("Failed to convert biome {} to NBT: {}", name, e);
                }
            }
        }

        entries
    }
}

/// Chat type registry data
pub mod chat_types {
    use super::*;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref REGISTRY_DATA: std::collections::HashMap<&'static str, serde_json::Value> = {
            let json_str = include_str!("registry_data.json");
            let full_data: serde_json::Value =
                serde_json::from_str(json_str).expect("Failed to parse registry_data.json");

            let chat_data = full_data["minecraft:chat_type"]
                .as_object()
                .expect("chat_type registry not found");

            let mut map = std::collections::HashMap::new();
            for (key, value) in chat_data {
                let static_key: &'static str = Box::leak(key.clone().into_boxed_str());
                map.insert(static_key, value.clone());
            }
            map
        };
    }

    /// Get all chat type registry entries as NBT data
    pub fn get_all_chat_types() -> Vec<(String, Vec<u8>)> {
        let mut entries = Vec::new();

        for (name, data) in REGISTRY_DATA.iter() {
            match json_to_nbt_bytes(data) {
                Ok(nbt_bytes) => {
                    entries.push((name.to_string(), nbt_bytes));
                }
                Err(e) => {
                    tracing::warn!("Failed to convert chat type {} to NBT: {}", name, e);
                }
            }
        }

        entries
    }
}

/// Damage type registry data
pub mod damage_types {
    use super::*;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref REGISTRY_DATA: std::collections::HashMap<&'static str, serde_json::Value> = {
            let json_str = include_str!("registry_data.json");
            let full_data: serde_json::Value =
                serde_json::from_str(json_str).expect("Failed to parse registry_data.json");

            let damage_data = full_data["minecraft:damage_type"]
                .as_object()
                .expect("damage_type registry not found");

            let mut map = std::collections::HashMap::new();
            for (key, value) in damage_data {
                let static_key: &'static str = Box::leak(key.clone().into_boxed_str());
                map.insert(static_key, value.clone());
            }
            map
        };
    }

    /// Get all damage type registry entries as NBT data
    pub fn get_all_damage_types() -> Vec<(String, Vec<u8>)> {
        let mut entries = Vec::new();

        for (name, data) in REGISTRY_DATA.iter() {
            match json_to_nbt_bytes(data) {
                Ok(nbt_bytes) => {
                    entries.push((name.to_string(), nbt_bytes));
                }
                Err(e) => {
                    tracing::warn!("Failed to convert damage type {} to NBT: {}", name, e);
                }
            }
        }

        entries
    }
}
