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

    // For registry data, the NBT is expected to be a root Compound tag.
    // fastnbt::to_bytes correctly handles the root tag ID (0x0A) and empty name (0x00 0x00).
    let buffer = fastnbt::to_bytes(&nbt_value)
        .map_err(|e| ServerError::Protocol(format!("Failed to serialize NBT: {}", e)))?;

    Ok(buffer)
}

/// Convert a JSON value to a fastnbt Value
fn json_to_fastnbt_value(json_value: &serde_json::Value) -> Result<fastnbt::Value> {
    match json_value {
        serde_json::Value::Null => {
            // NBT doesn't have a direct null.
            // If `registry_data.json` uses `null` for truly optional NBT fields,
            // they should ideally be excluded from the NBT compound, not converted to an empty string.
            // However, `serde_json::from_str` to `Value` will include them.
            // A common convention is to map null to a default value, or filter it out.
            // For now, let's stick with empty string, but keep this in mind for future debugging.
            Ok(fastnbt::Value::String("".to_string()))
        }
        serde_json::Value::Bool(b) => {
            Ok(fastnbt::Value::Byte(if *b { 1 } else { 0 }))
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                // Prioritize smallest integer type
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
                // Ensure float/double conversion is precise
                // `Float` (f32) has limited precision. If the number in JSON has many decimal places
                // but is stored as f32, it might lose precision.
                // However, NBT is explicit about Float vs Double.
                // It's safer to generally use Double unless we're certain it's meant to be Float.
                // `wiki.vg` often specifies which one to use. Let's assume double is default unless float is explicit.
                // For dimension type data (e.g. `coordinate_scale`), it's `Double`.
                Ok(fastnbt::Value::Double(f)) // Default to Double for floating points unless absolutely sure it's Float
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

            // This heuristic can still be problematic if the client expects a specific primitive array (e.g., TAG_Int_Array)
            // but the JSON might not strictly contain only ints (e.g. mixed numbers, or numbers that convert to bytes).
            // NBT arrays (Byte, Int, Long) must be homogenous. Generic List can contain mixed NBT Value types.

            // To make this more robust, we would need to know the *expected NBT tag type* for each array in the registry JSON.
            // For example, if it's "TAG_Int_Array" for `heights`, or "TAG_List of Compounds" for `sections`.
            // Without a schema, the safest is a generic List if it's not explicitly a byte/int/long array.

            // Let's refine the primitive array check to be more strict:
            let mut all_same_type: Option<u8> = None; // 1: Byte, 2: Short, 3: Int, 4: Long, 5: Float, 6: Double, 7: ByteArray, etc.
            if !generic_list_elements.is_empty() {
                all_same_type = Some(get_tag_id(&generic_list_elements[0]));
            }

            let mut is_primitive_array = true;
            for element in generic_list_elements.iter().skip(1) {
                if get_tag_id(element) != all_same_type.unwrap_or(0) {
                    is_primitive_array = false;
                    break;
                }
            }
            
            // Try specific NBT Array types if all elements match and it's a primitive type
            if is_primitive_array {
                match all_same_type {
                    Some(1) => Ok(fastnbt::Value::ByteArray(fastnbt::ByteArray::new(generic_list_elements.into_iter().map(|v| match v {fastnbt::Value::Byte(b)=>b, _=>0}).collect()))),
                    Some(3) => Ok(fastnbt::Value::IntArray(fastnbt::IntArray::new(generic_list_elements.into_iter().map(|v| match v {fastnbt::Value::Int(i)=>i, _=>0}).collect()))),
                    Some(4) => Ok(fastnbt::Value::LongArray(fastnbt::LongArray::new(generic_list_elements.into_iter().map(|v| match v {fastnbt::Value::Long(l)=>l, _=>0}).collect()))),
                    _ => Ok(fastnbt::Value::List(generic_list_elements)), // Fallback to generic list
                }
            } else {
                Ok(fastnbt::Value::List(generic_list_elements))
            }
        }
        serde_json::Value::Object(obj) => {
            let mut nbt_compound = std::collections::HashMap::new();
            for (key, value) in obj {
                // Filter out null values from the compound if they are truly optional and not meant to be a TAG_String("")
                if value.is_null() {
                    continue; // Skip null values from the NBT compound
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
