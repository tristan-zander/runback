use std::sync::Arc;

use chrono::Utc;
use entity::sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
use twilight_model::{
    application::interaction::{
        application_command::CommandOptionValue, MessageComponentInteraction,
    },
    channel::message::MessageFlags,
    guild::{PartialMember, Permissions},
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{
        marker::{ChannelMarker, GuildMarker, RoleMarker},
        Id,
    },
};
use twilight_util::builder::InteractionResponseDataBuilder;

use crate::interactions::application_commands::{
    ApplicationCommandData, ApplicationCommandUtilities, CommandGroupDescriptor,
    InteractionHandler, MessageComponentData,
};

pub struct MatchmakingSettingsHandler {
    utils: Arc<ApplicationCommandUtilities>,
}

#[async_trait]
impl InteractionHandler for MatchmakingSettingsHandler {
    fn describe(&self) -> CommandGroupDescriptor {
        // This is not a top-level command handler.
        // This function should never be registered into the InteractionProcessor/
        CommandGroupDescriptor {
            name: "settings",
            description: "View/update admin matchmaking settings",
            commands: Box::new([]),
        }
    }

    async fn process_command(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        let command = &data.command;
        // VERIFY: Is it possible that we can send the information of other guilds here?
        let guild_id = match command.guild_id {
            Some(id) => id,
            None => {
                return Err(anyhow!("Can't find a guild id for this command."));
            }
        };

        debug!(data = ?format!("{:?}", data.command.data));

        let group = data
            .command
            .data
            .options
            .into_iter()
            .nth(0)
            .ok_or_else(|| anyhow!("could not get any command options"))?;

        let group_options = if let CommandOptionValue::SubCommandGroup(group) = group.value {
            group
        } else {
            return Err(anyhow!(
                "sub-command found when it should be a sub-command group"
            ));
        };

        let subcommand = if let Some(sub) = group_options
            .into_iter()
            .filter(|o| {
                if let CommandOptionValue::SubCommand(_) = o.value {
                    return true;
                } else {
                    return false;
                }
            })
            .nth(0)
        {
            sub
        } else {
            return Err(anyhow!("no root-level sub-commands or arguments provided"));
        };

        let subcommand_options = if let CommandOptionValue::SubCommand(options) = subcommand.value {
            options
        } else {
            // realistically, we should never get here
            return Err(anyhow!("first argument is not a subcommand."));
        };

        match subcommand.name.as_str() {
            "matchmaking-channel" => {
                // Creates the guild settings object if it doens't exist
                let settings = self
                    .utils
                    .get_guild_settings(
                        data.command
                            .guild_id
                            .ok_or_else(|| anyhow!("this command cannot be used in a DM"))?,
                    )
                    .await?;

                let mut model = entity::matchmaking::settings::ActiveModel {
                    guild_id: Set(settings.guild_id),
                    ..Default::default()
                };

                let message;

                if let Some(channel) = subcommand_options
                    .iter()
                    .filter_map(|o| {
                        if let CommandOptionValue::Channel(chan) = o.value {
                            return Some(chan);
                        } else {
                            None
                        }
                    })
                    .nth(0)
                {
                    // TODO: Ensure the value of the option is a valid channel id.

                    model.channel_id = Set(Some(channel.into()));
                    message = format!(
                        "Successfully set the matchmaking channel to <#{}>.",
                        channel
                    );
                } else {
                    // There's no channel, disable the matchmaking channel
                    model.channel_id = Set(None);
                    message = "Successfully disabled matchmaking channel.".to_string();
                }

                entity::matchmaking::Setting::update(model)
                    .exec(self.utils.db_ref())
                    .await?;

                self.utils
                    .http_client
                    .interaction(self.utils.application_id)
                    .create_followup(data.command.token.as_str())
                    .content(message.as_str())?
                    .flags(MessageFlags::EPHEMERAL)
                    .exec()
                    .await?;
            }
            "admin-role" => {
                // Creates the guild settings object if it doens't exist
                let settings = self
                    .utils
                    .get_guild_settings(data.command.guild_id.unwrap())
                    .await?;

                let mut model = entity::matchmaking::settings::ActiveModel {
                    guild_id: Set(settings.guild_id),
                    ..Default::default()
                };

                let message;

                if let Some(role) = subcommand_options
                    .iter()
                    .filter_map(|o| {
                        if let CommandOptionValue::Role(role) = o.value {
                            return Some(role);
                        } else {
                            None
                        }
                    })
                    .nth(0)
                {
                    model.admin_role = Set(Some(role.into()));
                    message = format!("Successfully set the admin role to <@&{}>.", role);
                } else {
                    // Disable the admin role
                    model.admin_role = Set(None);
                    message = "Successfully removed the admin role.".to_string();
                }

                entity::matchmaking::Setting::update(model)
                    .exec(self.utils.db_ref())
                    .await?;

                self.utils
                    .http_client
                    .interaction(self.utils.application_id)
                    .create_followup(data.command.token.as_str())
                    .content(message.as_str())?
                    .flags(MessageFlags::EPHEMERAL)
                    .exec()
                    .await?;
            }
            _ => {
                return Err(anyhow!(
                    "unmatched command option found: {}",
                    subcommand.name
                ))
            }
        }

        Ok(())
    }

    async fn process_autocomplete(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        todo!()
    }

    async fn process_modal(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        todo!()
    }

    async fn process_component(&self, data: Box<MessageComponentData>) -> anyhow::Result<()> {
        let guild_id = data
            .message
            .guild_id
            .ok_or_else(|| anyhow!("you must run this command in a guild"))?
            .to_owned();

        let user = data
            .message
            .user
            .as_ref()
            .ok_or_else(|| anyhow!("cannot get user data"))?;

        let member = data
            .message
            .member
            .as_ref()
            .ok_or_else(|| anyhow!("cannot get member data"))?;

        let guild = self.utils.get_guild_settings(guild_id).await?;

        // validate that the user has the proper permissions
        if !self.is_authorized_admin(member, guild.admin_role) {
            return Err(anyhow!("you are not authorized to use this panel"));
        }

        match data.action.as_str() {
            "channel" => {
                self.set_matchmaking_channel(&data.message).await?;
                return Ok(());
            }
            "role" => {
                // set the admin role for this guild
                let role: Id<RoleMarker> = Id::new_checked(str::parse::<u64>(
                    data.message
                        .data
                        .values
                        .iter()
                        .nth(0)
                        .ok_or_else(|| anyhow!("no role provided in message"))?,
                )?)
                .ok_or_else(|| anyhow!("could not convert role into an ID"))?;

                self.set_admin_role(guild_id, role).await?;

                return Ok(());
            }
            _ => {
                return Err(anyhow!(
                    "Unknown field given to admin settings: {}",
                    &data.action
                ))
            }
        }
    }
}

impl MatchmakingSettingsHandler {
    pub fn new(utils: Arc<ApplicationCommandUtilities>) -> Self {
        Self { utils }
    }

    async fn set_matchmaking_channel(
        &self,
        component: &MessageComponentInteraction,
    ) -> anyhow::Result<()> {
        let channel_id: Id<ChannelMarker> = Id::new(
            component
                .data
                .values
                .get(0)
                .ok_or_else(|| anyhow!("No component values provided."))?
                .parse::<u64>()
                .map_err(|e| anyhow!(e))?,
        );

        let guild_id = component
            .guild_id
            .ok_or_else(|| anyhow!("You cannot use Runback in a DM."))?;

        let setting = entity::matchmaking::Setting::find_by_id(guild_id.into())
            .one(self.utils.db_ref())
            .await?;

        let _setting = if setting.is_some() {
            let mut setting = unsafe { setting.unwrap_unchecked() }.into_active_model();
            setting.channel_id = entity::sea_orm::Set(Some(channel_id.into()));
            setting.update(self.utils.db_ref()).await?
        } else {
            let setting = entity::matchmaking::settings::Model {
                guild_id: guild_id.into(),
                last_updated: Utc::now(),
                channel_id: Some(channel_id.into()),
                has_accepted_eula: None,
                threads_are_private: false,
                admin_role: None,
            }
            .into_active_model();
            setting
                .into_active_model()
                .insert(self.utils.db_ref())
                .await?
        };

        // TODO: Produce a Kafka message, saying that this guild's settings have been updated
        let _message = InteractionResponse { kind: InteractionResponseType::UpdateMessage, data: Some(
            InteractionResponseDataBuilder::new()
                .flags(MessageFlags::EPHEMERAL)
                .content("Successfully set the matchmaking channel. Please wait a few moments for changes to take effect.".into())
                .build()
        )};

        let _res =
            self.utils
            .http_client
            .interaction(self.utils.application_id)
            .update_response(component.token.as_str())
            .content(Some("Successfully set the matchmaking channel. Please wait a few moments for changes to take effect"))?
            // .map_err(|e| RunbackError { message: "Could not set content for response message during set_matchmaking_channel()".to_owned(), inner: Some(Box::new(e)) })?
            // .(component.id, component.token.as_str(), &message)
            .exec()
            .await?;

        Ok(())
    }

    async fn set_admin_role(
        &self,
        guild: Id<GuildMarker>,
        role: Id<RoleMarker>,
    ) -> anyhow::Result<entity::matchmaking::settings::Model> {
        Ok(
            entity::matchmaking::Setting::update(entity::matchmaking::settings::ActiveModel {
                guild_id: Set(guild.into()),
                admin_role: Set(Some(role.into())),
                ..Default::default()
            })
            .exec(self.utils.db_ref())
            .await?,
        )
    }

    fn is_authorized_admin(
        &self,
        member: &PartialMember,
        role: Option<entity::IdWrapper<RoleMarker>>,
    ) -> bool {
        if let Some(perms) = member.permissions {
            if perms.contains(Permissions::ADMINISTRATOR) {
                return true;
            }

            debug!(permissions = ?perms, "user does not have permissions to call admin commands");
        }

        if role.map(|r| member.roles.contains(&r.into())).is_some() {
            return true;
        }

        false
    }
}
