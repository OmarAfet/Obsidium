//! Tests for registry data and NBT serialization

use obsidium::data::{GameData, dimension_types};

#[test]
fn test_game_data_loading() {
    let game_data = GameData::load().expect("Failed to load game data");

    // Test that we can get dimension type registry entries
    let entries = game_data
        .get_registry_entries("minecraft:dimension_type")
        .expect("Failed to get dimension type entries");

    assert!(
        !entries.is_empty(),
        "Dimension type registry should not be empty"
    );

    // Check that we have the expected dimension types
    let dimension_names: Vec<&String> = entries.iter().map(|(name, _)| name).collect();
    assert!(dimension_names.contains(&&"minecraft:overworld".to_string()));
    assert!(dimension_names.contains(&&"minecraft:the_nether".to_string()));
    assert!(dimension_names.contains(&&"minecraft:the_end".to_string()));
}

#[test]
fn test_dimension_type_nbt_data() {
    // Test that we can get NBT data for each dimension type
    let overworld_nbt = dimension_types::get_dimension_type_nbt("minecraft:overworld");
    assert!(overworld_nbt.is_some(), "Overworld NBT should be available");
    assert!(
        !overworld_nbt.unwrap().is_empty(),
        "Overworld NBT should not be empty"
    );

    let nether_nbt = dimension_types::get_dimension_type_nbt("minecraft:the_nether");
    assert!(nether_nbt.is_some(), "Nether NBT should be available");
    assert!(
        !nether_nbt.unwrap().is_empty(),
        "Nether NBT should not be empty"
    );

    let end_nbt = dimension_types::get_dimension_type_nbt("minecraft:the_end");
    assert!(end_nbt.is_some(), "End NBT should be available");
    assert!(!end_nbt.unwrap().is_empty(), "End NBT should not be empty");
}

#[test]
fn test_nbt_data_structure() {
    use fastnbt::from_bytes;

    // Test that our NBT data can be properly deserialized
    if let Some(overworld_bytes) = dimension_types::get_dimension_type_nbt("minecraft:overworld") {
        let result: Result<fastnbt::Value, _> = from_bytes(&overworld_bytes);
        assert!(
            result.is_ok(),
            "Overworld NBT should be valid: {:?}",
            result
        );

        if let Ok(fastnbt::Value::Compound(compound)) = result {
            // Check for essential dimension type fields
            assert!(
                compound.contains_key("ambient_light"),
                "Should contain ambient_light"
            );
            assert!(
                compound.contains_key("has_skylight"),
                "Should contain has_skylight"
            );
            assert!(compound.contains_key("height"), "Should contain height");
            assert!(compound.contains_key("min_y"), "Should contain min_y");
        } else {
            panic!("NBT should deserialize to a compound tag");
        }
    }
}

#[test]
fn test_essential_registries() {
    let game_data = GameData::load().expect("Failed to load game data");
    let essential_registries = game_data.get_essential_registries();

    assert!(
        !essential_registries.is_empty(),
        "Should have essential registries"
    );
    assert!(
        essential_registries.contains(&"minecraft:dimension_type"),
        "Should include dimension_type registry"
    );
}
