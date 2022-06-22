use chrono::Utc;
use entity::{
    sea_orm::{prelude::Uuid, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set},
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

use super::{ApplicationCommandHandler, ApplicationCommandUtilities};

pub struct LfgCommandHandler {
    pub utils: Arc<ApplicationCommandUtilities>,
}

#[async_trait]
impl ApplicationCommandHandler for LfgCommandHandler {
    fn name(&self) -> String {
        "lfg".to_string()
    }

    fn register(&self) -> super::CommandHandlerType {
        super::CommandHandlerType::TopLevel(
            CommandBuilder::new(
                self.name(),
                "Look for games in the server".to_string(),
                CommandType::ChatInput,
            )
            .option(CommandOption::Integer(NumberCommandOptionData {
                autocomplete: false,
                choices: vec![
                    CommandOptionChoice::Int {
                        name: "15 minutes".to_string(),
                        value: 15,
                    },
                    CommandOptionChoice::Int {
                        name: "30 minutes".to_string(),
                        value: 30,
                    },
                    CommandOptionChoice::Int {
                        name: "1 hour".to_string(),
                        value: 60,
                    },
                    CommandOptionChoice::Int {
                        name: "2 hours".to_string(),
                        value: 60 * 2,
                    },
                    CommandOptionChoice::Int {
                        name: "3 hours".to_string(),
                        value: 60 * 3,
                    },
                    CommandOptionChoice::Int {
                        name: "6 hours".to_string(),
                        value: 60 * 6,
                    },
                    CommandOptionChoice::Int {
                        name: "12 hours".to_string(),
                        value: 60 * 12,
                    },
                    CommandOptionChoice::Int {
                        name: "1 day".to_string(),
                        value: 60 * 25,
                    },
                    CommandOptionChoice::Int {
                        name: "Forever (default)".to_string(),
                        value: -1,
                    },
                ],
                description: "Start/stop looking for games after a certain amount of time"
                    .to_string(),
                name: "howlong".to_string(),
                required: false,
                max_value: Some(CommandOptionValue::Integer(60 * 24 * 7)),
                min_value: Some(CommandOptionValue::Integer(-1)),
            }))
            .option(CommandOption::String(ChoiceCommandOptionData {
                autocomplete: false,
                choices: vec![],
                description: "A short message alongside your entry".to_string(),
                name: "comment".to_string(),
                required: false,
            }))
            .validate()
            .unwrap()
            .build(),
        )
    }

    async fn execute(&self, data: &super::InteractionData) -> anyhow::Result<()> {
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

        let lfg_session = self.get_user_lfg_session(guild_id, user.id).await?;

        if let Some(lfg) = lfg_session {
            // De-register the session
            let res = entity::matchmaking::lfg::Entity::delete(lfg.into_active_model())
                .exec(self.utils.db_ref())
                .await?;

            debug_assert!(res.rows_affected == 1);

            let message = InteractionResponse {
                kind: InteractionResponseType::ChannelMessageWithSource,
                data: Some(
                    InteractionResponseDataBuilder::new()
                        .content("Stopped looking for games...".to_string())
                        .build(),
                ),
            };

            self.utils
                .send_message(data.command, &message)
                .await
                .map_err(|e| anyhow!("Could not send message to user: {}", e))?;
        } else {
            // Start a new session
            let session = entity::matchmaking::lfg::ActiveModel {
                id: Set(Uuid::new_v4()),
                guild_id: Set(guild_id.into()),
                user_id: Set(user.id.into()),
                started_at: Set(Utc::now()),
                timeout_after: Set(None),
                ..Default::default()
            };

            entity::matchmaking::lfg::Entity::insert(session)
                .exec(self.utils.db_ref())
                .await?;

            let message = InteractionResponse {
                kind: InteractionResponseType::ChannelMessageWithSource,
                data: Some(
                    InteractionResponseDataBuilder::new()
                        .content("Started looking for games".to_string())
                        .build(),
                ),
            };

            self.utils
                .send_message(data.command, &message)
                .await
                .map_err(|e| anyhow!("Could not send message to user: {}", e))?;
        }

        Ok(())
    }
}

impl LfgCommandHandler {
    async fn get_user_lfg_session(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> anyhow::Result<Option<entity::matchmaking::lfg::Model>> {
        use entity::matchmaking::lfg;
        let lfg_session = lfg::Entity::find()
            .filter(lfg::Column::UserId.eq(Into::<IdWrapper<_>>::into(user_id)))
            .filter(lfg::Column::GuildId.eq(Into::<IdWrapper<_>>::into(guild_id)))
            .one(self.utils.db_ref())
            .await?;

        Ok(lfg_session)
    }
}
