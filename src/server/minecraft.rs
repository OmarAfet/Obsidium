//! Main Minecraft server implementation
//!
//! This module contains the core server logic that ties together all
//! the other modules to create a functioning Minecraft server.

use crate::config::ServerConfig;
use crate::data::GameData;
use crate::error::{Result, ServerError};
use crate::game::{player::PlayerManager, world::World};
use crate::network::{Connection, ServerListener};
use crate::protocol::packets::{
    Packet,
    configuration::{FinishConfigurationPacket, RegistryDataPacket, RegistryEntry},
    handshaking::HandshakePacket,
    login::{LoginAcknowledgedPacket, LoginStartPacket, LoginSuccessPacket, SetCompressionPacket},
    play::LoginPlayPacket,
    status::{
        Description, PingRequestPacket, PingResponsePacket, PlayersInfo, ServerStatus,
        StatusRequestPacket, StatusResponsePacket, VersionInfo,
    },
};
use crate::protocol::{ConnectionState, PROTOCOL_VERSION};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{Duration, interval};

/// Main Minecraft server
pub struct MinecraftServer {
    /// Server configuration
    config: ServerConfig,
    /// Player manager
    players: Arc<PlayerManager>,
    /// Main world
    world: Arc<RwLock<World>>,
    /// Server status
    status: ServerStatus,
    /// Game data (registries, etc.)
    game_data: Arc<GameData>,
}

impl MinecraftServer {
    /// Create a new Minecraft server
    pub async fn new(config: ServerConfig) -> Result<Self> {
        // Create server status
        let status = ServerStatus {
            version: VersionInfo {
                name: "1.21.5".to_string(),
                protocol: PROTOCOL_VERSION,
            },
            players: PlayersInfo {
                max: config.max_players,
                online: 0,
                sample: None,
            },
            description: Description::Text(config.motd.clone()),
            favicon: None,
            enforces_secure_chat: false,
        };

        // Load game data
        let game_data = Arc::new(GameData::load()?);

        Ok(Self {
            config,
            players: Arc::new(PlayerManager::new()),
            world: Arc::new(RwLock::new(World::new("world".to_string(), 12345))),
            status,
            game_data,
        })
    }

    /// Start the server
    pub async fn run(mut self) -> Result<()> {
        tracing::info!("Obsidium Minecraft Server v{}", env!("CARGO_PKG_VERSION"));

        // Create connection sender for the listener
        let (connection_sender, mut connection_receiver) = mpsc::unbounded_channel();

        // Start the network listener
        let listener = ServerListener::new(self.config.clone(), connection_sender).await?;
        let _listener_addr = listener.local_addr()?;

        // Spawn listener task
        let _listener_handle = tokio::spawn(async move {
            if let Err(e) = listener.listen().await {
                tracing::error!("Listener error: {}", e);
            }
        });

        // Create update timer
        let mut update_timer = interval(Duration::from_millis(50)); // 20 TPS

        // Main server loop
        loop {
            tokio::select! {
                // Handle new connections
                Some(connection) = connection_receiver.recv() => {
                    let players = Arc::clone(&self.players);
                    let world = Arc::clone(&self.world);
                    let status = self.status.clone();
                    let config = self.config.clone();
                    let game_data = Arc::clone(&self.game_data);

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(connection, players, world, status, config, game_data).await {
                            tracing::error!("Connection error: {}", e);
                        }
                    });
                }

                // Update world and game logic
                _ = update_timer.tick() => {
                    let mut world = self.world.write().await;
                    world.update(0.05); // 50ms delta

                    // Update player count in status
                    self.status.players.online = self.players.player_count().await as u32;
                }
            }
        }
    }

    /// Handle an individual connection
    async fn handle_connection(
        mut connection: Connection,
        players: Arc<PlayerManager>,
        _world: Arc<RwLock<World>>,
        status: ServerStatus,
        config: ServerConfig,
        game_data: Arc<GameData>,
    ) -> Result<()> {
        tracing::debug!("Handling connection from {}", connection.peer_addr());

        loop {
            // Read packet
            let (packet_id, data) = match connection.read_packet().await {
                Ok((pid, pdata)) => {
                    tracing::debug!(
                        "Received packet ID: 0x{:02X}, data length: {}, state: {:?}",
                        pid.0,
                        pdata.len(),
                        connection.state()
                    );
                    (pid, pdata)
                }
                Err(e) => {
                    tracing::debug!("Connection closed: {}", e);
                    break;
                }
            };

            let should_break = match connection.state() {
                ConnectionState::Handshaking => {
                    Self::handle_handshaking_packet(&mut connection, packet_id, &data)?;
                    false
                }
                ConnectionState::Status => {
                    Self::handle_status_packet(&mut connection, packet_id, &data, &status).await?
                }
                ConnectionState::Login => {
                    Self::handle_login_packet(&mut connection, packet_id, &data, &config, &players)
                        .await?;
                    false
                }
                ConnectionState::Configuration => {
                    Self::handle_configuration_packet(
                        &mut connection,
                        packet_id,
                        &data,
                        &config,
                        &game_data,
                    )
                    .await?;
                    false
                }
                ConnectionState::Play => {
                    Self::handle_play_packet(packet_id);
                    false
                }
            };

            if should_break {
                break;
            }
        }

        // Remove player when connection closes
        players.remove_player(connection.peer_addr()).await;

        Ok(())
    }

    /// Handle handshaking state packets
    fn handle_handshaking_packet(
        connection: &mut Connection,
        packet_id: crate::protocol::VarInt,
        data: &[u8],
    ) -> Result<()> {
        if packet_id.0 == HandshakePacket::ID {
            let handshake = HandshakePacket::read(&mut std::io::Cursor::new(data))?;

            tracing::debug!(
                "Handshake: version={}, address={}, port={}, next_state={}",
                handshake.protocol_version.0,
                handshake.server_address.0,
                handshake.server_port,
                handshake.next_state.0
            );

            connection.set_protocol_version(handshake.protocol_version.0);

            match handshake.next_state.0 {
                1 => connection.set_state(ConnectionState::Status),
                2 => connection.set_state(ConnectionState::Login),
                3 => {
                    // Transfer intent - for now, treat as login
                    // TODO: Implement proper transfer handling
                    connection.set_state(ConnectionState::Login);
                    tracing::debug!("Transfer intent received, treating as login for now");
                }
                _ => {
                    return Err(ServerError::Protocol("Invalid next state".to_string()));
                }
            }
        }
        Ok(())
    }

    /// Handle status state packets
    async fn handle_status_packet(
        connection: &mut Connection,
        packet_id: crate::protocol::VarInt,
        data: &[u8],
        status: &ServerStatus,
    ) -> Result<bool> {
        if packet_id.0 == StatusRequestPacket::ID {
            // Send status response
            let json = status.to_json()?;
            let response = StatusResponsePacket {
                json_response: json.into(),
            };
            connection.write_packet(&response).await?;
        } else if packet_id.0 == PingRequestPacket::ID {
            let ping = PingRequestPacket::read(&mut std::io::Cursor::new(data))?;
            let pong = PingResponsePacket {
                payload: ping.payload,
            };
            connection.write_packet(&pong).await?;
            return Ok(true); // Close connection after ping
        }
        Ok(false)
    }

    /// Handle login state packets
    async fn handle_login_packet(
        connection: &mut Connection,
        packet_id: crate::protocol::VarInt,
        data: &[u8],
        config: &ServerConfig,
        players: &Arc<PlayerManager>,
    ) -> Result<()> {
        if packet_id.0 == LoginStartPacket::ID {
            let login_start = LoginStartPacket::read(&mut std::io::Cursor::new(data))?;

            tracing::info!(
                "Player {} ({}) logging in from {}",
                login_start.name.0,
                login_start.player_uuid,
                connection.peer_addr()
            );

            // Enable compression if configured
            if let Some(threshold) = config.compression_threshold {
                let compression_packet = SetCompressionPacket {
                    threshold: (threshold as i32).into(),
                };
                connection.write_packet(&compression_packet).await?;
                connection.enable_compression(threshold)?;
            }

            // Send login success
            let login_success = LoginSuccessPacket {
                uuid: login_start.player_uuid,
                username: login_start.name.clone(),
                properties: Vec::new(),
            };
            connection.write_packet(&login_success).await?;

            // Create player
            let player =
                crate::game::player::Player::new(login_start.player_uuid, login_start.name.0);

            players.add_player(player, connection.peer_addr()).await;

            connection.set_state(ConnectionState::Configuration);

            tracing::info!("Player logged in successfully, transitioning to configuration state");
        }
        Ok(())
    }

    /// Handle configuration state packets
    async fn handle_configuration_packet(
        connection: &mut Connection,
        packet_id: crate::protocol::VarInt,
        data: &[u8],
        config: &ServerConfig,
        game_data: &Arc<GameData>,
    ) -> Result<()> {
        if packet_id.0 == LoginAcknowledgedPacket::ID {
            let _login_ack = LoginAcknowledgedPacket::read(&mut std::io::Cursor::new(data))?;

            tracing::info!("Login acknowledged received, starting configuration phase");

            // Send registry data packets
            Self::send_registry_data(connection, game_data).await?;

            // Send finish configuration packet
            let finish_config = FinishConfigurationPacket;
            connection.write_packet(&finish_config).await?;

            tracing::info!("Configuration phase completed, sent finish configuration packet");
        } else if packet_id.0 == 0x02 {
            // Acknowledge Finish Configuration packet
            use crate::protocol::packets::configuration::AcknowledgeFinishConfigurationPacket;
            let _ack_finish =
                AcknowledgeFinishConfigurationPacket::read(&mut std::io::Cursor::new(data))?;

            tracing::info!(
                "Acknowledge finish configuration received, transitioning to play state"
            );

            connection.set_state(ConnectionState::Play);

            // Send login play packet after transitioning to play state
            let login_play = LoginPlayPacket::from_server_config(config, 1);
            connection.write_packet(&login_play).await?;

            tracing::info!("Login play packet sent, player is now in play state");
        }
        Ok(())
    }

    /// Send registry data to the client
    async fn send_registry_data(
        connection: &mut Connection,
        game_data: &Arc<GameData>,
    ) -> Result<()> {
        tracing::debug!("Sending registry data to client");

        // Send essential registries
        for registry_name in game_data.get_essential_registries() {
            match Self::create_registry_packet(game_data, registry_name) {
                Ok(packet) => {
                    tracing::debug!(
                        "Sending {} registry with {} entries",
                        registry_name,
                        packet.entries.len()
                    );

                    // Log entry details
                    for entry in &packet.entries {
                        let data_size = entry.data.as_ref().map_or(0, |d| d.len());
                        let first_bytes: Vec<u8> = entry
                            .data
                            .as_ref()
                            .map_or(vec![], |d| d.iter().take(16).cloned().collect());
                        tracing::debug!(
                            "  Entry: {}, has_data: {}, data_size: {}, first_bytes: {:02X?}",
                            entry.entry_id.0,
                            entry.has_data,
                            data_size,
                            first_bytes
                        );
                    }

                    connection.write_packet(&packet).await?;
                    tracing::debug!("Successfully sent {} registry data", registry_name);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to create registry packet for {}: {}",
                        registry_name,
                        e
                    );
                    // Use fallback data for critical registries
                    if registry_name == "minecraft:dimension_type" {
                        let fallback_packet = Self::create_fallback_dimension_type_registry()?;
                        tracing::debug!(
                            "Using fallback dimension_type registry with {} entries",
                            fallback_packet.entries.len()
                        );
                        connection.write_packet(&fallback_packet).await?;
                        tracing::debug!("Sent fallback dimension_type registry data");
                    }
                }
            }
        }

        tracing::debug!("Finished sending all registry data");
        Ok(())
    }

    /// Create a registry data packet for a specific registry
    fn create_registry_packet(
        game_data: &Arc<GameData>,
        registry_name: &str,
    ) -> Result<RegistryDataPacket> {
        let entries_data = game_data.get_registry_entries(registry_name)?;
        let mut entries = Vec::new();

        for (entry_name, nbt_bytes) in entries_data {
            entries.push(RegistryEntry {
                entry_id: entry_name.into(),
                has_data: !nbt_bytes.is_empty(),
                data: if nbt_bytes.is_empty() {
                    None
                } else {
                    Some(nbt_bytes)
                },
            });
        }

        Ok(RegistryDataPacket {
            registry_id: registry_name.into(),
            entries,
        })
    }

    /// Create a fallback dimension type registry using pre-computed NBT
    fn create_fallback_dimension_type_registry() -> Result<RegistryDataPacket> {
        use crate::data::dimension_types;

        let entries_data = dimension_types::get_all_dimension_types();
        let mut entries = Vec::new();

        for (entry_name, nbt_bytes) in entries_data {
            entries.push(RegistryEntry {
                entry_id: entry_name.into(),
                has_data: !nbt_bytes.is_empty(),
                data: if nbt_bytes.is_empty() {
                    None
                } else {
                    Some(nbt_bytes)
                },
            });
        }

        Ok(RegistryDataPacket {
            registry_id: "minecraft:dimension_type".into(),
            entries,
        })
    }

    /// Handle play state packets
    fn handle_play_packet(packet_id: crate::protocol::VarInt) {
        // Handle play packets
        tracing::debug!("Received play packet ID: 0x{:02X}", packet_id.0);

        // TODO: Implement play packet handlers
        // For now, just log them
    }
}

impl Drop for MinecraftServer {
    fn drop(&mut self) {
        tracing::info!("Obsidium Minecraft Server shutting down");
    }
}
