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
        // The provided registry_name might be fully qualified (e.g., "minecraft:dimension_type")
        // but the JSON key might not be. We'll try both.
        let short_name = registry_name
            .split(':')
            .next_back()
            .unwrap_or(registry_name);

        let json_str = include_str!("registry_data.json");
        let full_data: serde_json::Value =
            serde_json::from_str(json_str).expect("Failed to parse registry_data.json");

        let registry_data = full_data
            .get(registry_name) // Try the full name first
            .or_else(|| full_data.get(short_name)) // Fallback to the short name
            .ok_or_else(|| {
                ServerError::Protocol(format!(
                    "Registry '{}' not found in registry_data.json",
                    registry_name
                ))
            })?;

        let registry_object = registry_data.as_object().ok_or_else(|| {
            ServerError::Protocol(format!("Registry '{}' is not a JSON object", registry_name))
        })?;

        let mut entries = Vec::new();
        for (entry_name, entry_data) in registry_object {
            match json_to_nbt_bytes(entry_data) {
                Ok(nbt_bytes) => {
                    entries.push((entry_name.clone(), nbt_bytes));
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to convert {} entry {} to NBT: {}",
                        registry_name,
                        entry_name,
                        e
                    );
                }
            }
        }

        Ok(entries)
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

    /// Get all registries that should be sent to the client.
    /// This now includes all registries found in the JSON file.
    pub fn get_all_registries(&self) -> Vec<String> {
        let json_str = include_str!("registry_data.json");
        let full_data: serde_json::Value =
            serde_json::from_str(json_str).expect("Failed to parse registry_data.json");

        full_data
            .as_object()
            .unwrap()
            .keys()
            .map(|key| {
                if key.contains(':') {
                    key.clone()
                } else {
                    format!("minecraft:{}", key)
                }
            })
            .collect()
    }
}

/// Helper function to convert JSON to NBT bytes
fn json_to_nbt_bytes(json_value: &serde_json::Value) -> Result<Vec<u8>> {
    let nbt_value = json_to_fastnbt_value(json_value)?;

    // Since Minecraft 1.20.2, NBT sent over the network for registries excludes
    // the root compound tag's name, but includes its ID (0x0A).
    let mut full_buffer = fastnbt::to_bytes(&nbt_value)
        .map_err(|e| ServerError::Protocol(format!("Failed to serialize NBT: {}", e)))?;

    // fastnbt::to_bytes produces [0x0A, 0x00, 0x00, ...payload...].
    // We need to remove the 2-byte empty name from indices 1 and 2.
    if full_buffer.len() < 3 || full_buffer[0] != 10 {
        return Err(ServerError::Protocol(
            "Expected a compound NBT tag for registry data".to_string(),
        ));
    }

    // This creates the correct [0x0A, ...payload...] format.
    full_buffer.remove(1);
    full_buffer.remove(1);

    Ok(full_buffer)
}

/// Convert a JSON value to a fastnbt Value
fn json_to_fastnbt_value(json_value: &serde_json::Value) -> Result<fastnbt::Value> {
    match json_value {
        serde_json::Value::Null => Ok(fastnbt::Value::String("".to_string())),
        serde_json::Value::Bool(b) => Ok(fastnbt::Value::Byte(if *b { 1 } else { 0 })),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                // With booleans handled separately, we can now safely assume that
                // remaining integer-like numbers in registry data are meant to be
                // TAG_Int, unless they are too large for a 32-bit signed integer.
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    Ok(fastnbt::Value::Int(i as i32))
                } else {
                    Ok(fastnbt::Value::Long(i))
                }
            } else if let Some(f) = n.as_f64() {
                // For floating-point numbers, infer Float vs Double based on precision.
                let f32_val = f as f32;
                if (f32_val as f64) == f {
                    Ok(fastnbt::Value::Float(f32_val))
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
            if arr.is_empty() {
                return Ok(fastnbt::Value::List(Vec::new()));
            }

            let mut generic_list_elements = Vec::new();
            for item in arr {
                generic_list_elements.push(json_to_fastnbt_value(item)?);
            }

            let mut all_same_type_id: Option<u8> = None;
            if !generic_list_elements.is_empty() {
                all_same_type_id = Some(get_tag_id(&generic_list_elements[0]));
            }

            let mut is_homogenous_primitive = true;
            for element in generic_list_elements.iter().skip(1) {
                if get_tag_id(element) != all_same_type_id.unwrap_or(0) {
                    is_homogenous_primitive = false;
                    break;
                }
            }

            if is_homogenous_primitive {
                match all_same_type_id {
                    Some(1) => Ok(fastnbt::Value::ByteArray(fastnbt::ByteArray::new(
                        generic_list_elements
                            .into_iter()
                            .map(|v| match v {
                                fastnbt::Value::Byte(b) => b,
                                _ => 0, // Should not happen
                            })
                            .collect(),
                    ))),
                    Some(3) => Ok(fastnbt::Value::IntArray(fastnbt::IntArray::new(
                        generic_list_elements
                            .into_iter()
                            .map(|v| match v {
                                fastnbt::Value::Int(i) => i,
                                _ => 0, // Should not happen
                            })
                            .collect(),
                    ))),
                    Some(4) => Ok(fastnbt::Value::LongArray(fastnbt::LongArray::new(
                        generic_list_elements
                            .into_iter()
                            .map(|v| match v {
                                fastnbt::Value::Long(l) => l,
                                _ => 0, // Should not happen
                            })
                            .collect(),
                    ))),
                    _ => Ok(fastnbt::Value::List(generic_list_elements)),
                }
            } else {
                Ok(fastnbt::Value::List(generic_list_elements))
            }
        }
        serde_json::Value::Object(obj) => {
            let mut nbt_compound = std::collections::HashMap::new();
            for (key, value) in obj {
                if value.is_null() {
                    continue;
                }
                nbt_compound.insert(key.clone(), json_to_fastnbt_value(value)?);
            }
            Ok(fastnbt::Value::Compound(nbt_compound))
        }
    }
}

/// Get the NBT tag ID for a given fastnbt Value
fn get_tag_id(value: &fastnbt::Value) -> u8 {
    match value {
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
