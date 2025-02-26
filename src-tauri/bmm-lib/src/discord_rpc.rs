use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Add Clone derive
#[derive(Clone)]
pub struct DiscordRpcManager {
    enabled: Arc<Mutex<bool>>,
    client: Arc<Mutex<Option<DiscordIpcClient>>>,
}

impl Default for DiscordRpcManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscordRpcManager {
    pub fn new() -> Self {
        log::info!("Initializing Discord RPC Manager");
        let manager = Self {
            enabled: Arc::new(Mutex::new(false)),
            client: Arc::new(Mutex::new(None)),
        };

        let manager_clone = manager.clone();

        thread::spawn(move || {
            log::info!("Discord RPC thread started");
            let mut last_connection_attempt = std::time::Instant::now() - Duration::from_secs(30);

            loop {
                thread::sleep(Duration::from_secs(5));

                let enabled = match manager_clone.enabled.lock() {
                    Ok(guard) => *guard,
                    Err(e) => {
                        log::error!("Failed to lock Discord RPC enabled status: {}", e);
                        false
                    }
                };

                if !enabled {
                    // If disabled, close any existing connection
                    if let Ok(mut client_guard) = manager_clone.client.lock() {
                        if let Some(client) = client_guard.as_mut() {
                            log::info!("Closing Discord connection (disabled)");
                            if let Err(e) = client.close() {
                                log::error!("Failed to close connection: {}", e);
                            }
                            *client_guard = None;
                        }
                    }
                    continue;
                }

                // Get client lock
                let mut client_guard = match manager_clone.client.lock() {
                    Ok(guard) => guard,
                    Err(e) => {
                        log::error!("Failed to lock Discord RPC client: {}", e);
                        continue;
                    }
                };

                // Check if we need to create a new client
                if client_guard.is_none() {
                    // Limit connection attempts to avoid spamming
                    let now = std::time::Instant::now();
                    if now.duration_since(last_connection_attempt) < Duration::from_secs(15) {
                        continue;
                    }

                    last_connection_attempt = now;
                    log::info!("Creating new Discord RPC client");

                    match DiscordIpcClient::new("1344296383145967669") {
                        Ok(mut client) => {
                            log::info!("Created client, connecting...");
                            if let Err(e) = client.connect() {
                                log::error!("Failed to connect: {}", e);
                            } else {
                                log::info!("Connected to Discord!");
                                *client_guard = Some(client);
                            }
                        }
                        Err(e) => log::error!("Failed to create client: {}", e),
                    }
                }

                // Update activity for existing client
                if let Some(client) = client_guard.as_mut() {
                    log::debug!("Updating Discord activity");
                    let activity = activity::Activity::new()
                        .state("Managing Balatro mods")
                        .details("Using Balatro Mod Manager")
                        .assets(
                            activity::Assets::new()
                                .large_image("bmm-icon")
                                .large_text("Balatro Mod Manager"),
                        );

                    // Try to update the activity
                    if let Err(e) = client.set_activity(activity) {
                        log::error!("Failed to update activity, reconnecting: {}", e);
                        // Connection might be dead, close it so we recreate next time
                        if let Err(e) = client.close() {
                            log::error!("Failed to close broken connection: {}", e);
                        }
                        *client_guard = None;
                    } else {
                        log::debug!("Activity updated successfully");
                    }
                }
            }
        });

        manager
    }

    pub fn set_enabled(&self, enabled: bool) {
        log::info!("Setting Discord RPC enabled status to: {}", enabled);
        if let Ok(mut enabled_guard) = self.enabled.lock() {
            *enabled_guard = enabled;

            if !enabled {
                log::info!("Disabling Discord RPC, closing connection");
                if let Ok(mut client_guard) = self.client.lock() {
                    if let Some(client) = client_guard.as_mut() {
                        if let Err(e) = client.close() {
                            log::error!("Failed to close Discord RPC connection: {}", e);
                        }
                    }
                    *client_guard = None;
                }
            }
        }
    }

    pub fn update_activity(&self, state: &str, details: &str) {
        // Check if Discord RPC is enabled
        let enabled = match self.enabled.lock() {
            Ok(guard) => *guard,
            Err(_) => return, // Exit early if we can't get the lock
        };

        if !enabled {
            return;
        }

        // Try to get the client lock
        let mut client_guard = match self.client.lock() {
            Ok(guard) => guard,
            Err(_) => return, // Exit early if we can't get the lock
        };

        if let Some(client) = client_guard.as_mut() {
            log::info!("Updating Discord activity: {} - {}", state, details);
            let activity = activity::Activity::new()
                .state(state)
                .details(details)
                .assets(
                    activity::Assets::new()
                        .large_image("bmm-icon")
                        .large_text("Balatro"),
                );

            if let Err(e) = client.set_activity(activity) {
                log::error!("Failed to update Discord activity: {}", e);
            }
        }
    }
}
