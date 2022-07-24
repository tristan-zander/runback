use chrono::Utc;
use dashmap::DashMap;
use entity::matchmaking::lfg;
use entity::{
    sea_orm::{prelude::Uuid, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter},
    IdWrapper,
};
use std::sync::Arc;
use twilight_model::{
    application::command::{
        ChoiceCommandOptionData, CommandOption, CommandOptionChoice, CommandOptionValue,
        CommandType, NumberCommandOptionData,
    },
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{
        marker::{GuildMarker, UserMarker},
        Id,
    },
};
use twilight_util::builder::{command::CommandBuilder, InteractionResponseDataBuilder};

use crate::handler;

use super::{
    ApplicationCommandHandler, ApplicationCommandUtilities, CommandDescriptor,
    CommandGroupDescriptor, InteractionData,
};

pub struct LfgCommandHandler {
    pub utils: Arc<ApplicationCommandUtilities>,
    pub lfg_sessions: Arc<DashMap<Uuid, Box<lfg::Model>>>,
}

#[async_trait]
impl ApplicationCommandHandler for LfgCommandHandler {
    fn register(&self) -> CommandGroupDescriptor {
        let command = CommandBuilder::new(
            "lfg".to_string(),
            "Look for games in the server".to_string(),
            CommandType::ChatInput,
        )
        .option(CommandOption::Integer(NumberCommandOptionData {
            autocomplete: false,
            choices: vec![
                CommandOptionChoice::Int {
                    name: "15 minutes".to_string(),
                    value: 15,
                    name_localizations: None,
                },
                CommandOptionChoice::Int {
                    name: "30 minutes".to_string(),
                    value: 30,
                    name_localizations: None,
                },
                CommandOptionChoice::Int {
                    name: "1 hour".to_string(),
                    value: 60,
                    name_localizations: None,
                },
                CommandOptionChoice::Int {
                    name: "2 hours".to_string(),
                    value: 60 * 2,
                    name_localizations: None,
                },
                CommandOptionChoice::Int {
                    name: "3 hours".to_string(),
                    value: 60 * 3,
                    name_localizations: None,
                },
                CommandOptionChoice::Int {
                    name: "6 hours".to_string(),
                    value: 60 * 6,
                    name_localizations: None,
                },
                CommandOptionChoice::Int {
                    name: "12 hours".to_string(),
                    value: 60 * 12,
                    name_localizations: None,
                },
                CommandOptionChoice::Int {
                    name: "1 day".to_string(),
                    value: 60 * 25,
                    name_localizations: None,
                },
                CommandOptionChoice::Int {
                    name: "Forever (default)".to_string(),
                    value: -1,
                    name_localizations: None,
                },
            ],
            description: "Start/stop looking for games after a certain amount of time".to_string(),
            name: "howlong".to_string(),
            required: false,
            max_value: Some(CommandOptionValue::Integer(60 * 24 * 7)),
            min_value: Some(CommandOptionValue::Integer(-1)),
            description_localizations: None,
            name_localizations: None,
        }))
        .option(CommandOption::String(ChoiceCommandOptionData {
            autocomplete: false,
            choices: vec![],
            description: "A short message alongside your entry".to_string(),
            name: "comment".to_string(),
            required: false,
            description_localizations: None,
            name_localizations: None,
        }))
        .validate()
        .expect("\"lfg\" command failed validation")
        .build();

        CommandGroupDescriptor {
            name: "lfg",
            description: "Helps you find someone to play games with",
            commands: Box::new([CommandDescriptor {
                command,
                handler: Some(handler!(LfgCommandHandler::execute)),
            }]),
        }
    }
}

impl LfgCommandHandler {
    async fn execute(
        utils: Arc<ApplicationCommandUtilities>,
        data: Box<InteractionData>,
    ) -> anyhow::Result<()> {
        let guild_id = data
            .command
            .guild_id
            .ok_or(anyhow!("Command was not run in a guild"))?;
        let member = data
            .command
            .member
            .as_ref()
            .ok_or(anyhow!("The command was not run in a guild"))?;
        let user = member.user.as_ref().ok_or(anyhow!(
            "Could not get user information for user \"{}\"",
            member
                .nick
                .as_ref()
                .unwrap_or(&"No nickname found".to_string())
        ))?;

        let lfg_session = Self::get_user_lfg_session(utils.clone(), guild_id, user.id).await?;

        let message = if let Some(lfg) = lfg_session {
            // De-register the sessio
            Self::delete_session(utils.clone(), lfg).await?;

            InteractionResponse {
                kind: InteractionResponseType::ChannelMessageWithSource,
                data: Some(
                    InteractionResponseDataBuilder::new()
                        .content("Stopped looking for games...".to_string())
                        .build(),
                ),
            }
        } else {
            // Start a new session
            let session = entity::matchmaking::lfg::Model {
                id: Uuid::new_v4(),
                guild_id: guild_id.into(),
                user_id: user.id.into(),
                started_at: Utc::now(),
                timeout_after: None,
            };

            Self::add_new_session(utils.clone(), session).await?;

            InteractionResponse {
                kind: InteractionResponseType::ChannelMessageWithSource,
                data: Some(
                    InteractionResponseDataBuilder::new()
                        .content("Started looking for games".to_string())
                        .build(),
                ),
            }
        };

        utils
            .send_message(&data.command, &message)
            .await
            .map_err(|e| anyhow!("Could not send message to user: {}", e))?;

        Ok(())
    }
    async fn get_user_lfg_session(
        utils: Arc<ApplicationCommandUtilities>,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> anyhow::Result<Option<entity::matchmaking::lfg::Model>> {
        let lfg_session = lfg::Entity::find()
            .filter(lfg::Column::UserId.eq(Into::<IdWrapper<_>>::into(user_id)))
            .filter(lfg::Column::GuildId.eq(Into::<IdWrapper<_>>::into(guild_id)))
            .one(utils.db_ref())
            .await?;

        Ok(lfg_session)
    }

    async fn add_new_session(
        utils: Arc<ApplicationCommandUtilities>,
        model: lfg::Model,
    ) -> anyhow::Result<()> {
        let boxed = Box::new(model);
        let _res = entity::matchmaking::lfg::Entity::insert(boxed.to_owned().into_active_model())
            .exec(utils.db_ref())
            .await?;

        // self.lfg_sessions.insert(res.last_insert_id, boxed);

        Ok(())
    }

    async fn delete_session(
        utils: Arc<ApplicationCommandUtilities>,
        model: lfg::Model,
    ) -> anyhow::Result<()> {
        let res = entity::matchmaking::lfg::Entity::delete(model.to_owned().into_active_model())
            .exec(utils.db_ref())
            .await?;

        debug_assert!(res.rows_affected == 1);

        // self.lfg_sessions.remove(&model.id);

        Ok(())
    }
}
