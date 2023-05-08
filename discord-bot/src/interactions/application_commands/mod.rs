pub mod admin;
pub mod eula;
// pub mod lfg;

#[deprecated(note = "Revisiting this later")]
pub mod lfg {}
pub mod matchmaking;
pub mod utils;

pub use utils::CommonUtilities;

use sea_orm::prelude::*;
use twilight_model::{
    application::{
        command::Command,
        interaction::{
            application_command::CommandData, message_component::MessageComponentInteractionData,
            Interaction,
        },
    },
    id::{marker::GuildMarker, Id},
};

/// Describes a group of commands. This is mainly used
/// for structural purposes, and for the `/help` command
#[derive(Debug, Clone)]
pub struct CommandGroupDescriptor {
    /// The name of the command group
    pub name: &'static str,
    /// The description of the command group
    pub description: &'static str,
    /// The commands that are releated to this group
    pub commands: Box<[Command]>,
}

#[async_trait]
pub trait InteractionHandler {
    fn describe(&self) -> CommandGroupDescriptor;
    async fn process_command(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()>;
    async fn process_autocomplete(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()>;
    async fn process_modal(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()>;
    async fn process_component(&self, data: Box<MessageComponentData>) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct ApplicationCommandData {
    pub interaction: Interaction,
    pub command: CommandData,
    pub id: Uuid,
    pub guild_id: Id<GuildMarker>, // pub cancellation_token
}

#[derive(Debug)]
pub struct MessageComponentData {
    pub interaction: Interaction,
    pub message: MessageComponentInteractionData,
    pub action: String,
    pub id: Uuid,
    // pub cancellation_token
}
