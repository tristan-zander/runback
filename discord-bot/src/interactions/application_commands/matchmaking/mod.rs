mod lobby;

use crate::{
    db::RunbackDB,
    entity::{self, prelude::*, IdWrapper},
};
use chrono::Utc;
use futures::StreamExt;
use sea_orm::{prelude::*, Set};
use tokio::task::JoinHandle;
use twilight_gateway::Event;
use twilight_model::{
    application::{
        command::{CommandOption, CommandOptionType, CommandType},
        interaction::application_command::{CommandDataOption, CommandOptionValue},
    },
    channel::{
        message::{
            allowed_mentions::AllowedMentionsBuilder,
            component::{ActionRow, Button, ButtonStyle},
            Component,
        },
        thread::ThreadMember,
        Channel, ChannelType, Message,
    },
    gateway::payload::incoming::ChannelDelete,
    guild::Guild,
    id::{marker::UserMarker, Id},
    user::User,
};
use twilight_standby::Standby;
use twilight_util::builder::{
    command::{CommandBuilder, SubCommandBuilder, SubCommandGroupBuilder},
    embed::{EmbedBuilder, EmbedFieldBuilder},
};

use crate::{
    client::{DiscordClient, RunbackClient},
    interactions::application_commands::matchmaking::lobby::LobbyData,
};

use self::lobby::LobbyCommandHandler;

use super::{
    ApplicationCommandData, CommandGroupDescriptor, InteractionHandler, MessageComponentData,
};

use std::{
    ops::Add,
    sync::Arc,
    time::{Duration, Instant},
};

pub struct MatchmakingCommandHandler {
    db: RunbackDB,
    client: DiscordClient,
    lobby: LobbyCommandHandler,
    _background_task: JoinHandle<()>,
}

#[async_trait]
impl InteractionHandler for MatchmakingCommandHandler {
    fn create(client: &RunbackClient) -> Self {
        let bg = BackgroundLoop::new(client);
        let _background_task = tokio::task::spawn(async move {
            loop {
                if let Err(e) = bg.background_loop().await {
                    error!(error = ?e, "background loop update failed");
                }
            }
        });

        let lobby = LobbyCommandHandler::new(client.discord_client.clone(), client.db());

        Self {
            db: client.db(),
            client: client.discord_client.clone(),
            lobby,
            _background_task,
        }
    }

    fn describe() -> CommandGroupDescriptor {
        let builder = CommandBuilder::new(
            "matchmaking".to_string(),
            "Matchmaking commands".to_string(),
            CommandType::ChatInput,
        )
        .dm_permission(false)
        .option(
            SubCommandGroupBuilder::new("lobby", "Start, join, or change settings for a lobby.")
                .subcommands([
                    SubCommandBuilder::new("open", "Start a new lobby."),
                    SubCommandBuilder::new("close", "Close an existing lobby."),
                    SubCommandBuilder::new("settings", "Change lobby settings."),
                    SubCommandBuilder::new("invite", "Invite a person to this lobby.").option(
                        CommandOption {
                            autocomplete: Some(false),
                            channel_types: None,
                            choices: None,
                            name: "user".to_string(),
                            description: "The user that you wish to invite.".to_string(),
                            description_localizations: None,
                            name_localizations: None,
                            kind: CommandOptionType::User,
                            max_value: None,
                            max_length: None,
                            min_value: None,
                            min_length: None,
                            options: None,
                            required: Some(true),
                        },
                    ),
                ])
                .build(),
        );

        let command = builder.build();
        CommandGroupDescriptor {
            name: "matchmaking",
            description: "Commands that are related to matchmaking",
            commands: Box::new([command]),
        }
    }

    async fn process_command(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        if let Some(_target) = data.command.target_id {
            // Then start a mm session with that user. It's not chat message command,
            // but a click interaction.
        }

        // Copy the data to send to the other class for processing.
        // Pretty sure there's never going to be a data race but this is okay.
        let data_copy = data.clone();

        let member = data
            .interaction
            .member
            .ok_or_else(|| anyhow!("command cannot be run in a DM"))?;

        let member_copy = member.clone();

        let user = member
            .user
            .ok_or_else(|| anyhow!("could not get user data for caller"))?;

        let subcommand = data
            .command
            .options
            .get(0)
            .ok_or_else(|| anyhow!("could not get subcommand option"))?;

        let action = subcommand.name.clone();

        match action.as_str() {
            "lobby" => {
                if let CommandOptionValue::SubCommandGroup(group) = &subcommand.value {
                    let option = group
                        .get(0)
                        .ok_or_else(|| anyhow!("could not get lobby subcommand"))?
                        .to_owned();
                    let action = option.name.clone();
                    self.lobby
                        .process_command(LobbyData {
                            action,
                            data: data_copy,
                            option,
                            member: member_copy,
                        })
                        .await?;
                } else {
                    return Err(anyhow!(
                        "Somehow, the lobby command did not send a command group"
                    ));
                }
                Ok(())
            }
            "report-score" => {
                let chan_id = data
                    .interaction
                    .channel_id
                    .ok_or_else(|| anyhow!("command was not run in a channel"))?;

                let lobby = MatchmakingLobbies::find()
                    .filter(matchmaking_lobbies::Column::ChannelId.eq(IdWrapper::from(chan_id)))
                    .one(self.db.connection())
                    .await?;

                if lobby.is_none() {
                    return Err(anyhow!(
                        "You must run this command in a valid matchmaking thread."
                    ));
                }

                // TODO: Refactor get resolved user from app command data to a function.
                let resolved = data
                    .command
                    .resolved
                    .ok_or_else(|| anyhow!("cannot get the resolved command user data"))?;
                let opponent = resolved.users.values().next().ok_or_else(|| {
                    anyhow!("cannot get the user specified in \"report-score\" command")
                })?;

                let response = self.client.channel(chan_id).await?;

                if !response.status().is_success() {
                    return Err(anyhow!("failed to get thread"));
                }

                let channel: Channel = response.model().await?;

                let thread_members: Vec<ThreadMember> = self
                    .client
                    .thread_members(channel.id)
                    .await?
                    .models()
                    .await?;

                let mut reporter_is_member: bool = false;
                let mut opponent_is_member: bool = false;

                for member in thread_members {
                    let member_user_id = member.user_id.ok_or_else(|| {
                        anyhow!("cannot get the user specified in \"report-score\" command")
                    })?;
                    if user.id == member_user_id {
                        reporter_is_member = true;
                    } else if opponent.id == member_user_id {
                        opponent_is_member = true;
                    } else if reporter_is_member && opponent_is_member {
                        break;
                    }
                }

                // TODO: Check if opponent is the bot, because that would be invalid.
                if !reporter_is_member {
                    return Err(anyhow!("user isn't part of the lobby"));
                } else if !opponent_is_member {
                    return Err(anyhow!("opponent isn't part of the lobby"));
                }

                let options: Vec<CommandDataOption>;
                match subcommand.value.clone() {
                    CommandOptionValue::SubCommand(x) => options = x,
                    _ => options = Vec::<CommandDataOption>::new(),
                }

                let wins_option_value = options
                    .get(1)
                    .ok_or_else(|| anyhow!("could not get wins option"))?
                    .value
                    .clone();

                let loses_option_value = options
                    .get(2)
                    .ok_or_else(|| anyhow!("could not get loses option"))?
                    .value
                    .clone();

                let wins: i64 = match wins_option_value {
                    CommandOptionValue::Integer(x) => x as i64,
                    _ => 0 as i64,
                };

                let loses: i64 = match loses_option_value {
                    CommandOptionValue::Integer(x) => x as i64,
                    _ => 0 as i64,
                };

                // TODO: Refactor all this message stuff to reusable functions with parameters?
                // TODO: Add an embed with options for the opponent to accept or dispute the score report.
                // TODO: Format the message so it tags both the User & Opponent.
                //       And show the wins for both of them in a nice looking way.
                let guild_settings = self.db.get_guild_settings(data.guild_id).await?;
                let channel;
                if let Some(cid) = guild_settings.channel_id {
                    channel = cid.into_id();
                } else {
                    channel = data
                        .interaction
                        .channel_id
                        .ok_or_else(|| anyhow!("command was not run in a channel"))?;
                }
                let _msg = self
                    .client
                    .interaction()
                    .create_followup(data.interaction.token.as_str())
                    .content(format!("**<@{}> vs <@{}>**", user.id, opponent.id).as_str())?
                    .embeds(&[EmbedBuilder::new()
                        .title("Score report")
                        .description("If the reported score is correct press the Accept button, if not you can press the Dispute button to resolve the conflict.")
                        .field(EmbedFieldBuilder::new(&user.name, wins.to_string()).inline())
                        .field(EmbedFieldBuilder::new(&opponent.name, loses.to_string()).inline())
                        .validate()?
                        .build()])?
                    .components(&[Component::ActionRow(ActionRow {
                        components: vec![
                            Component::Button(Button {
                                custom_id: Some("matchmaking:accept-score-report".to_string()),
                                disabled: false,
                                emoji: None,
                                label: Some("Accept".to_string()),
                                style: ButtonStyle::Primary,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("matchmaking:deny-score-report".to_string()),
                                disabled: false,
                                emoji: None,
                                label: Some("Dispute".to_string()),
                                style: ButtonStyle::Danger,
                                url: None,
                            }),
                        ],
                    })])?
                    .allowed_mentions(Some(
                        &AllowedMentionsBuilder::new()
                            .user_ids([user.id, opponent.id])
                            .build(),
                    ))
                    .await?
                    .model()
                    .await?;

                return Ok(());
            }
            "done" => {
                let chan_id = data
                    .interaction
                    .channel_id
                    .ok_or_else(|| anyhow!("command was not run in a channel"))?;

                let lobby = MatchmakingLobbies::find()
                    .filter(matchmaking_lobbies::Column::ChannelId.eq(IdWrapper::from(chan_id)))
                    .one(self.db.connection())
                    .await?;

                if let Some(lobby) = lobby {
                    // TODO: Validate that the user is a part of the lobby.

                    match self
                        .client
                        .interaction()
                        .create_followup(data.interaction.token.as_str())
                        .content("Closing the lobby soon. Thanks for using runback!")?
                        .await
                    {
                        Ok(_) => tokio::time::sleep(Duration::from_secs(3)).await,
                        Err(e) => {
                            error!(error = ?e, "closing matchmaking channel success message failed to send")
                        }
                    }

                    // self.client.delete_channel(id).exec().await?;
                    self.client
                        .update_thread(chan_id)
                        .archived(true)
                        .locked(true)
                        .await?;

                    MatchmakingLobbies::update(matchmaking_lobbies::ActiveModel {
                        id: Set(lobby.id),
                        ended_at: Set(Some(Utc::now())),
                        ..Default::default()
                    })
                    .filter(matchmaking_lobbies::Column::TimeoutAfter.gte(Utc::now()))
                    .exec(self.db.connection())
                    .await?;
                } else {
                    return Err(anyhow!(
                        "You must run this command in a valid matchmaking thread."
                    ));
                }

                Ok(())
            }
            _ => return Err(anyhow!("command handler for \"{}\" not found.", action)),
        }
    }

    async fn process_autocomplete(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        unreachable!()
    }

    async fn process_modal(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        unreachable!()
    }

    async fn process_component(&self, data: Box<MessageComponentData>) -> anyhow::Result<()> {
        let member = data
            .interaction
            .member
            .ok_or_else(|| anyhow!("command cannot be run in a DM"))?;

        let _user = member
            .user
            .ok_or_else(|| anyhow!("could not get user data for caller"))?;

        let _guild_id = data
            .interaction
            .guild_id
            .ok_or_else(|| anyhow!("Command cannot be run in a DM"))?;

        match data.action.as_str() {
            "accept" => {
                // let chan_id = data
                //     .interaction
                //     .channel_id
                //     .ok_or_else(|| anyhow!("could not get channel of message component"))?;
                // let msg_id = data
                //     .interaction
                //     .message
                //     .ok_or_else(|| anyhow!("interaction not run on a message component"))?
                //     .id;
                //
                // let invitation = MatchmakingInvitation::find()
                //     .filter(matchmaking_invitation::Column::MessageId.eq(IdWrapper::from(msg_id)))
                //     .filter(matchmaking_invitation::Column::ChannelId.eq(IdWrapper::from(chan_id)))
                //     .one(self.db.connection()())
                //     .await?
                //     .ok_or_else(|| anyhow!("could not find a valid invitation."))?;
                //
                // let user_model = self.utils.find_or_create_user(user.id).await?;
                //
                // if invitation.extended_to != user_model.user_id {
                //     self.utils
                //         .http_client
                //         .interaction(self.utils.application_id)
                //         .create_response(
                //             data.interaction.id,
                //             data.interaction.token.as_str(),
                //             &InteractionResponse {
                //                 kind: InteractionResponseType::ChannelMessageWithSource,
                //                 data: Some(
                //                     InteractionResponseDataBuilder::new()
                //                         .content("You were not invited to this match.".to_string())
                //                         .flags(MessageFlags::EPHEMERAL)
                //                         .build(),
                //                 ),
                //             },
                //         )
                //         .await?;
                //
                //     return Ok(());
                // }
                //
                // let opponent = Users::find_by_id(invitation.invited_by)
                //     .one(self.db.connection()())
                //     .await?
                //     .ok_or_else(|| {
                //         anyhow!("could not find user information for the person that invited you")
                //     })?;
                //
                // let author_data: Member = self
                //     .utils
                //     .http_client
                //     .guild_member(
                //         guild_id,
                //         opponent
                //             .discord_user
                //             .ok_or_else(|| anyhow!("user does not have a discord id"))?
                //             .into(),
                //     )
                //     .await?
                //     .model()
                //     .await?;
                //
                // let opponent_data: Member = self
                //     .utils
                //     .http_client
                //     .guild_member(
                //         guild_id,
                //         user_model
                //             .discord_user
                //             .ok_or_else(|| anyhow!("user does not have a discord id"))?
                //             .into(),
                //     )
                //     .await?
                //     .model()
                //     .await?;
                // let message_id = invitation
                //     .message_id
                //     .ok_or_else(|| anyhow!("no invitation message id found"))?;
                //
                // let thread = self
                //     .start_matchmaking_thread(
                //         guild_id,
                //         message_id.into_id(),
                //         format!(
                //             "{} vs {}",
                //             author_data.nick.unwrap_or(author_data.user.name),
                //             opponent_data.nick.unwrap_or(opponent_data.user.name)
                //         ),
                //     )
                //     .await?;
                //
                // let users = vec![author_data.user.id, opponent_data.user.id];
                // let res = self.add_users_to_thread(thread.id, &users).await;
                //
                // if let Err(e) = res {
                //     // Close the thread and send an error.
                //
                //     self.client.delete_channel(thread.id).await?;
                //
                //     return Err(e);
                // }
                //
                // self.send_thread_opening_message(&users, thread.id).await?;
                //
                // let started_at = Utc::now();
                //
                // let owner = self.utils.find_or_create_user(author_data.user.id).await?;
                //
                // let lobby = matchmaking_lobbies::Model {
                //     id: Uuid::new_v4(),
                //     started_at,
                //     timeout_after: started_at + chrono::Duration::hours(3),
                //     channel_id: thread.id.into(),
                //     description: None,
                //     owner: owner.user_id,
                //     privacy: LobbyPrivacy::Open,
                //     game: None,
                //     game_other: None,
                //     ended_at: None,
                //     timeout_warning_message: None,
                // };
                //
                // for user in users {
                //     // Create a discord user, in case they don't exist.
                //     // TODO: Do this in bulk
                //     self.utils.find_or_create_user(user).await?;
                // }
                //
                // let _res = matchmaking_lobbies::Entity::insert(lobby.into_active_model())
                //     .exec(self.db.connection()())
                //     .await?;
                //
                // self.utils
                //     .http_client
                //     .update_message(invitation.channel_id.into(), message_id.into_id())
                //     .components(Some(&[]))?
                //     .await?;
                //
                // Ok(())
                unimplemented!()
            }
            "deny" => {
                // // validate users
                // let msg = data
                //     .interaction
                //     .message
                //     .ok_or_else(|| anyhow!("interaction not run on a message component"))?;
                //
                // let invitation = MatchmakingInvitation::find()
                //     .filter(matchmaking_invitation::Column::MessageId.eq(IdWrapper::from(msg.id)))
                //     .one(self.db.connection()())
                //     .await?
                //     .ok_or_else(|| anyhow!("could not find that match invitation"))?;
                //
                // // TODO: Cache this
                // let guild = self
                //     .utils
                //     .http_client
                //     .guild(
                //         data.interaction
                //             .guild_id
                //             .ok_or_else(|| anyhow!("you cannot use this command in a dm"))?,
                //     )
                //     .await?
                //     .model()
                //     .await?;
                //
                // let settings = self.utils.get_guild_settings(guild.id).await?;
                //
                // let is_admin = if let Some(admin_role) = settings.admin_role {
                //     member.roles.contains(&admin_role.into_id())
                // } else {
                //     false
                // };
                //
                // let user_model = self.utils.find_or_create_user(user.id).await?;
                //
                // // cancel invitation
                // if (user_model.user_id != invitation.extended_to
                //     && user_model.user_id != invitation.invited_by)
                //     || is_admin
                // {
                //     return Err(anyhow!("not authorized to deny that invitation"));
                // }
                //
                // let dm_res = self
                //     .dm_users_upon_cancellation(&invitation, &user, &guild)
                //     .await;
                //
                // if let Err(e) = dm_res {
                //     error!(err = ?e, "could not dm user");
                //
                //     // why wont this auto format?
                //     self.utils
                //         .http_client
                //         .interaction(self.utils.application_id)
                //         .create_response(data.interaction.id, data.interaction.token.as_str(),
                //         &InteractionResponse { kind: InteractionResponseType::ChannelMessageWithSource, data: Some(
                //             InteractionResponseDataBuilder::new()
                //         .flags(MessageFlags::EPHEMERAL)
                //         .embeds([
                //             EmbedBuilder::new().title("DM Error").description("Could not inform one or more of the users about the match cancellation.")
                //             .field(EmbedFieldBuilder::new("error", "The bot successfully denied the invitation, but it could not DM one or more of the users about the cancellation. Please notify them manually.").build())
                //             .build(),
                //         ])
                //         .build()
                //         ) }
                //     )
                //         .await?;
                // }
                //
                // // Remove the Accept/Deny buttons from the message
                // if let Some(msg_id) = invitation.message_id {
                //     self.utils
                //         .http_client
                //         .update_message(invitation.channel_id.into_id(), msg_id.into_id())
                //         .components(Some(&[]))?
                //         .content(None)?
                //         .await?;
                // }
                //
                // self.utils
                //     .http_client
                //     .interaction(self.utils.application_id)
                //     .create_response(
                //         data.interaction.id,
                //         data.interaction.token.as_str(),
                //         &InteractionResponse {
                //             kind: InteractionResponseType::ChannelMessageWithSource,
                //             data: Some(
                //                 InteractionResponseDataBuilder::new()
                //                     .content("Invitation cancelled.".to_string())
                //                     .flags(MessageFlags::EPHEMERAL)
                //                     .build(),
                //             ),
                //         },
                //     )
                //     .await?;
                //
                // MatchmakingInvitation::update(matchmaking_invitation::ActiveModel {
                //     id: Set(invitation.id),
                //     expires_at: Set(Utc::now()), // TODO: Set the invitation as "Denied"
                //     ..Default::default()
                // })
                // .exec(self.db.connection()())
                // .await?;
                //
                // Ok(())
                unimplemented!()
            }
            _ => return Err(anyhow!("no handler for action: {}", data.action)),
        }
    }
}

impl MatchmakingCommandHandler {
    async fn dm_users_upon_cancellation(
        &self,
        invitation: &matchmaking_invitation::Model,
        user: &User,
        guild: &Guild,
    ) -> anyhow::Result<()> {
        let user_model = self.db.find_or_create_user(user.id).await?;

        if user_model.user_id != invitation.invited_by {
            let author = Users::find_by_id(invitation.invited_by)
                .one(self.db.connection())
                .await?
                .ok_or_else(|| anyhow!("no user found with that id"))?;
            let _res = self
                .dm_invited(
                    author
                        .discord_user
                        .ok_or_else(|| anyhow!("user does not have a valid discord user"))?
                        .into_id(),
                    user,
                    guild,
                    invitation,
                )
                .await;
        } else {
            let user_model = Users::find_by_id(invitation.extended_to)
                .one(self.db.connection())
                .await?
                .ok_or_else(|| anyhow!("no user found with that id"))?;
            self.dm_invited(
                user_model
                    .discord_user
                    .ok_or_else(|| anyhow!("user does not have a valid discord user"))?
                    .into_id(),
                user,
                guild,
                invitation,
            )
            .await?;
        }
        Ok(())
    }

    async fn dm_invited(
        &self,
        user: Id<UserMarker>,
        // The person that cancelled the invitation
        canceller: &User,
        guild: &Guild,
        _invitation: &matchmaking_invitation::Model,
    ) -> anyhow::Result<Message> {
        // TODO: Cache this
        let dm = self
            .client
            .create_private_channel(user)
            .await?
            .model()
            .await?;

        debug_assert_eq!(
            dm.kind,
            ChannelType::Private,
            "DM channel created by Discord was not private"
        );

        let msg = self
            .client
            .create_message(dm.id)
            .content(
                format!(
                    "\"{}@{}\" cancelled your matchmaking request in \"{}\"",
                    canceller.name, canceller.discriminator, guild.name
                )
                .as_str(),
            )?
            .await?
            .model()
            .await?;

        Ok(msg)
    }
}

struct BackgroundLoop {
    db: RunbackDB,
    client: DiscordClient,
    standby: Arc<Standby>,
}

impl BackgroundLoop {
    fn new(client: &RunbackClient) -> Self {
        Self {
            db: client.db(),
            client: client.discord_client.clone(),
            standby: client.standby.clone(),
        }
    }

    /// Queries and updates the sessions and invitations.
    #[instrument(skip_all)]
    async fn update(&self) -> anyhow::Result<()> {
        // TODO: Aggregate errors

        // Timeout expired sessions
        let expired = self.get_expired_lobbies().await?;
        for s in &expired {
            // Send an expiration message, archive the thread, and end the session.
            if self.check_if_lobby_should_be_extended(s).await? {
                self.extend_lobby(s).await?;
                continue;
            }
            self.timeout_expired_lobby(s).await?;
        }

        // Send pre-expiration warning messages
        let almost_expired = self.get_expiring_lobbies().await?;
        for s in &almost_expired {
            if self.check_if_lobby_should_be_extended(s).await? {
                self.extend_lobby(s).await?;
                continue;
            }

            if s.timeout_warning_message.is_some() {
                continue;
            }

            self.send_expiration_warning_message(s).await?;
        }

        Ok(())
    }

    #[instrument(skip_all)]
    async fn get_expiring_lobbies(&self) -> Result<Vec<matchmaking_lobbies::Model>, anyhow::Error> {
        let lobbies = MatchmakingLobbies::find()
            .filter(
                matchmaking_lobbies::Column::TimeoutAfter
                    // Get all sessions expiring in 15 minutes
                    .lte(Utc::now().add(chrono::Duration::minutes(15))),
            )
            .filter(matchmaking_lobbies::Column::EndedAt.is_null())
            .all(self.db.connection())
            .await?;

        Ok(lobbies)
    }

    async fn extend_lobby(&self, s: &matchmaking_lobbies::Model) -> anyhow::Result<()> {
        let lobby = matchmaking_lobbies::ActiveModel {
            id: Set(s.id),
            timeout_after: Set(Utc::now() + chrono::Duration::minutes(30)),
            timeout_warning_message: Set(None),
            ..Default::default()
        };
        debug!(lobby = ?lobby.id, "extending lobby session");

        MatchmakingLobbies::update(lobby)
            .exec(self.db.connection())
            .await?;

        Ok(())
    }

    async fn check_if_lobby_should_be_extended(
        &self,
        s: &matchmaking_lobbies::Model,
    ) -> anyhow::Result<bool> {
        let chan = self
            .client
            .channel(s.channel_id.into_id())
            .await?
            .model()
            .await?;

        if let Some(msg) = chan.last_message_id {
            // If the user deleted the last message, then it's possible that the
            // last message id is going to return an invalid message.
            let msg = self
                .client
                .message(chan.id, msg.cast())
                .await?
                .model()
                .await?;

            let now = Utc::now();
            let last_message_sent_at = chrono::DateTime::parse_from_rfc3339(
                msg.timestamp.iso_8601().to_string().as_str(),
            )?;

            // Check if the last message was sent in the last 30 minutes
            // If it was, then extend the expiration time by a half hour.
            // Otherwise, send the expiration warning.

            if last_message_sent_at > (now - chrono::Duration::minutes(30))
                && msg.author.id != self.client.current_user.id
                && s.timeout_warning_message
                    .as_ref()
                    .map_or(true, |id| id.into_id() != msg.id)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[instrument(skip_all)]
    async fn send_expiration_warning_message(
        &self,
        s: &matchmaking_lobbies::Model,
    ) -> anyhow::Result<()> {
        let msg = self
        .client
            .create_message(s.channel_id.into_id())
            .content("This lobby will close in 15 minutes due to inactivity. Please click \"Extend\" or type in chat to extend the lobby.")?
            .components(&[
                Component::ActionRow(
                    ActionRow {
                        components: vec![
                            Component::Button(
                                Button {
                                    custom_id: Some("matchmaking:extend_lobby".to_string()),
                                    disabled: false,
                                    emoji: None,
                                    label: Some("Extend".to_string()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }
                            ),
                            Component::Button(
                                Button {
                                    custom_id: Some("matchmaking:close_lobby".to_string()),
                                    disabled: false,
                                    emoji: None,
                                    label: Some("Close Lobby".to_string()),
                                    style: ButtonStyle::Danger,
                                    url: None
                                }
                            )
                        ]
                    }
                )
            ])?
            .await?
            .model()
            .await?;

        MatchmakingLobbies::update(matchmaking_lobbies::ActiveModel {
            id: Set(s.id),
            timeout_warning_message: Set(Some(msg.id.into())),
            ..Default::default()
        })
        .exec(self.db.connection())
        .await?;

        Ok(())
    }

    #[instrument(skip_all)]
    async fn timeout_expired_lobby(
        &self,
        s: &entity::matchmaking_lobbies::Model,
    ) -> anyhow::Result<()> {
        let chan_id = s.channel_id.into_id();
        let chan = self.client.channel(chan_id).await?.model().await?;
        let _msg = self
            .client
            .create_message(chan.id)
            .content("This matchmaking lobby has timed out. See ya later!")?
            .await?;
        if chan.kind.is_thread() {
            let _thread = self
                .client
                .update_thread(chan.id)
                .archived(true)
                .locked(true)
                .await?;
        }

        // Close any matchmaking invitations.
        // self.close_lobby(s).await?;
        unimplemented!("Close the lobby");

        MatchmakingLobbies::update(matchmaking_lobbies::ActiveModel {
            id: Set(s.id),
            ended_at: Set(Some(Utc::now())),
            ..Default::default()
        })
        .exec(self.db.connection())
        .await?;

        Ok(())
    }

    async fn get_expired_lobbies(&self) -> Result<Vec<matchmaking_lobbies::Model>, anyhow::Error> {
        let lobbies = MatchmakingLobbies::find()
            .filter(matchmaking_lobbies::Column::TimeoutAfter.lte(Utc::now()))
            .filter(matchmaking_lobbies::Column::EndedAt.is_null())
            .all(self.db.connection())
            .await?;

        Ok(lobbies)
    }

    /// This function should only return catestrophic errors!
    #[instrument(skip_all)]
    async fn background_loop(&self) -> anyhow::Result<()> {
        let mut stream = {
            self.standby
                .wait_for_event_stream(move |e: &Event| match e {
                    Event::ChannelDelete(_) => true,
                    _ => false,
                })
        };

        let mut interval = tokio::time::interval(Duration::from_secs(60));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let start = Instant::now();
                    debug!("Filtering lobbies");

                    if let Err(e) = self.update().await {
                        error!(error = ?e, "Encountered errors while updating lobbies.");
                    }

                    let end = start.elapsed();
                    debug!(end = ?end, time_ms = ?end.as_millis(), "Finished filtering lobbies");
                }
                Some(chan_delete) = stream.next() => {
                    debug!(del = ?format!("{:?}", chan_delete), "Channel was deleted. Remove any lobbies attached to this channel.");
                    if let Event::ChannelDelete(chan) = chan_delete {
                        if let Err(e) = self.on_channel_delete(chan).await {
                            error!(error = ?e, "encountered error when dealing with deleted channel");
                        }
                    }
                }
            }
        }
    }

    #[instrument(skip_all)]
    async fn on_channel_delete(&self, chan: Box<ChannelDelete>) -> anyhow::Result<()> {
        let lobby = matchmaking_lobbies::Entity::find()
            .filter(matchmaking_lobbies::Column::ChannelId.eq(IdWrapper::from(chan.id)))
            .one(self.db.connection())
            .await?;

        if let Some(_lobby) = lobby {
            // Delete the lobby and de-activate all invitations.
            // self.close_lobby(&lobby).await?;
            unimplemented!("close the lobby")
        }

        Ok(())
    }
}
