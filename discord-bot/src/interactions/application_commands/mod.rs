pub mod admin;
pub mod eula;
// pub mod lfg;

#[deprecated(note = "Revisiting this later")]
pub mod lfg {}
pub mod matchmaking;

use sea_orm::prelude::*;
use serde::Serialize;
use twilight_model::{
    application::{
        command::Command,
        interaction::{
            application_command::CommandData, message_component::MessageComponentInteractionData,
            Interaction,
        },
    },
    guild::PartialMember,
    id::{marker::GuildMarker, Id},
    user::User,
};

use crate::client::RunbackClient;

/// Describes a group of commands. This is mainly used
/// for structural purposes, and for the `/help` command
#[derive(Debug, Clone, Serialize)]
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
    fn create(client: &RunbackClient) -> Self
    where
        Self: Sized;
    fn describe() -> CommandGroupDescriptor
    where
        Self: Sized;
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
    pub guild_id: Id<GuildMarker>,
    pub member: PartialMember,
    pub user: User,
}

impl ApplicationCommandData {
    pub fn new(data: CommandData, interaction: Interaction) -> anyhow::Result<Self> {
        let new_interaction = interaction.clone();

        let member = interaction.member.ok_or_else(|| {
            anyhow!("Could not get member information for user that invoked the command.")
        })?;

        let guild_id = data
            .guild_id
            .ok_or_else(|| anyhow!("Command was not run in a guild."))?;

        let user = member
            .user
            .clone()
            .ok_or_else(|| anyhow!("Could not get user information."))?;

        Ok(Self {
            id: Uuid::new_v4(),
            interaction: new_interaction,
            command: data,
            member,
            user,
            guild_id,
        })
    }
}

#[derive(Debug)]
pub struct MessageComponentData {
    pub interaction: Interaction,
    pub message: MessageComponentInteractionData,
    pub action: String,
    pub id: Uuid,
}
