use bot::entity::{self, prelude::*, IdWrapper};
use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::{prelude::*, IntoActiveModel, Set};
use tokio::task::JoinHandle;
use twilight_gateway::Event;
use twilight_model::{
    application::{
        command::{BaseCommandOptionData, CommandOption, CommandType},
        component::{button::ButtonStyle, ActionRow, Button, Component},
    },
    channel::{
        message::{allowed_mentions::AllowedMentionsBuilder, MessageFlags},
        thread::AutoArchiveDuration,
        Channel, ChannelType, Message,
    },
    gateway::payload::incoming::ChannelDelete,
    guild::{Guild, Member},
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{
        marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker},
        Id,
    },
    user::User,
};
use twilight_util::builder::{
    command::{CommandBuilder, SubCommandBuilder},
    embed::{EmbedBuilder, EmbedFieldBuilder},
    InteractionResponseDataBuilder,
};

use super::{
    ApplicationCommandData, CommandGroupDescriptor, CommonUtilities, InteractionHandler,
    MessageComponentData,
};

use futures::StreamExt;

use std::{
    ops::Add,
    sync::Arc,
    time::{Duration, Instant},
};

pub struct MatchmakingCommandHandler {
    utils: Arc<CommonUtilities>,
    _background_task: JoinHandle<()>,
}

#[async_trait]
impl InteractionHandler for MatchmakingCommandHandler {
    fn describe(&self) -> CommandGroupDescriptor {
        let builder = CommandBuilder::new(
            "matchmaking".to_string(),
            "Matchmaking commands".to_string(),
            CommandType::ChatInput,
        )
        .option(
            SubCommandBuilder::new(
                "play-against".to_string(),
                "Start a match with an opponent".to_string(),
            )
            .option(CommandOption::User(BaseCommandOptionData {
                name: "opponent".to_string(),
                description: "The user that you wish to play against".to_string(),
                description_localizations: None,
                name_localizations: None,
                required: true,
            }))
            // TODO: Add this when it's ready
            // .option(CommandOption::String(ChoiceCommandOptionData {
            //     autocomplete: false,
            //     choices: vec![],
            //     description: "An invite message to your opponent".to_string(),
            //     description_localizations: None,
            //     name: "invitation".to_string(),
            //     name_localizations: None,
            //     required: false,
            // }))
            .build(),
        )
        // .option(
        //     SubCommandBuilder::new("show-matches".into(), "Show the matchmaking menu".into())
        //         .build(),
        // )
        //
        // .option(
        //     SubCommandBuilder::new(
        //         "settings", // Deprecating this in favor of `/admin` commands
        //         "View and update settings such as default character",
        //     )
        //     .build(),
        // )
        .option(SubCommandBuilder::new("done", "Finish your matchmaking lobby").build())
        .option(SubCommandBuilder::new("report-score", "Report the score of a match").build());

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

        let member = data
            .interaction
            .member
            .ok_or_else(|| anyhow!("command cannot be run in a DM"))?;

        let user = member
            .user
            .ok_or_else(|| anyhow!("could not get user data for caller"))?;

        let action = data
            .command
            .options
            .get(0)
            .ok_or_else(|| anyhow!("could not get subcommand option"))?
            .name
            .clone();

        match action.as_str() {
            "play-against" => {
                let resolved = data
                    .command
                    .resolved
                    .ok_or_else(|| anyhow!("cannot get the resolved command user data"))?;

                let invited = resolved.users.values().next().ok_or_else(|| {
                    anyhow!("cannot get the user specified in \"play-against\" command")
                })?;

                if invited.id == user.id {
                    return Err(anyhow!("you cannot invite yourself"));
                }

                let guild_settings = self
                    .utils
                    .get_guild_settings(
                        data.command
                            .guild_id
                            .ok_or_else(|| anyhow!("command cannot be used in a DM"))?,
                    )
                    .await?;

                let channel;
                if let Some(cid) = guild_settings.channel_id {
                    // TODO: make sure that the channel actually exists.
                    channel = cid.into_id();
                } else {
                    channel = data
                        .interaction
                        .channel_id
                        .ok_or_else(|| anyhow!("command was not run in a channel"))?;
                }

                let msg = self
                    .utils
                    .http_client
                    .create_message(channel)
                    .content(format!("<@{}>", invited.id).as_str())?
                    .embeds(&[EmbedBuilder::new()
                        .title("New matchmaking request")
                        .description(format!(
                            "<@{}> has invited you to a match, <@{}>",
                            user.id, invited.id
                        ))
                        .validate()?
                        .build()])?
                    .components(&[Component::ActionRow(ActionRow {
                        components: vec![
                            Component::Button(Button {
                                custom_id: Some("matchmaking:accept".to_string()),
                                disabled: false,
                                emoji: None,
                                label: Some("Accept".to_string()),
                                style: ButtonStyle::Primary,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("matchmaking:deny".to_string()),
                                disabled: false,
                                emoji: None,
                                label: Some("Deny".to_string()),
                                style: ButtonStyle::Danger,
                                url: None,
                            }),
                        ],
                    })])?
                    .allowed_mentions(Some(
                        &AllowedMentionsBuilder::new()
                            .user_ids([user.id, invited.id])
                            .build(),
                    ))
                    .exec()
                    .await?
                    .model()
                    .await?;

                let _followup = self
                    .utils
                    .http_client
                    .interaction(self.utils.application_id)
                    .create_followup(data.interaction.token.as_str())
                    .content(format!("Sent a request in <#{}>", channel).as_str())?
                    .flags(MessageFlags::EPHEMERAL)
                    .exec()
                    .await?
                    .model()
                    .await?;

                let author = self.utils.find_or_create_user(user.id).await?;
                let invited = self.utils.find_or_create_user(invited.id).await?;

                let invitation = matchmaking_invitation::Model {
                    id: Uuid::new_v4(),
                    lobby: None,
                    extended_to: invited.user_id,
                    invited_by: author.user_id,
                    game: None,
                    description: None,
                    message_id: Some(msg.id.into()),
                    expires_at: Utc::now() + chrono::Duration::minutes(30),
                    channel_id: channel.into(),
                };

                debug!(invitation = ?format!("{:?}", invitation));

                MatchmakingInvitation::insert(invitation.into_active_model())
                    .exec(self.utils.db_ref())
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
                    .one(self.utils.db_ref())
                    .await?;

                if let Some(lobby) = lobby {
                    // TODO: Validate that the user is a part of the lobby.

                    match self
                        .utils
                        .http_client
                        .interaction(self.utils.application_id)
                        .create_followup(data.interaction.token.as_str())
                        .content("Closing the lobby soon. Thanks for using runback!")?
                        .exec()
                        .await
                    {
                        Ok(_) => tokio::time::sleep(Duration::from_secs(3)).await,
                        Err(e) => {
                            error!(error = ?e, "closing matchmaking channel success message failed to send")
                        }
                    }

                    // self.utils.http_client.delete_channel(id).exec().await?;
                    self.utils
                        .http_client
                        .update_thread(chan_id)
                        .archived(true)
                        .locked(true)
                        .exec()
                        .await?;

                    MatchmakingLobbies::update(matchmaking_lobbies::ActiveModel {
                        id: Set(lobby.id),
                        ended_at: Set(Some(Utc::now())),
                        ..Default::default()
                    })
                    .filter(matchmaking_lobbies::Column::TimeoutAfter.gte(Utc::now()))
                    .exec(self.utils.db_ref())
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

        let user = member
            .user
            .ok_or_else(|| anyhow!("could not get user data for caller"))?;

        let guild_id = data
            .interaction
            .guild_id
            .ok_or_else(|| anyhow!("Command cannot be run in a DM"))?;

        match data.action.as_str() {
            "accept" => {
                let chan_id = data
                    .interaction
                    .channel_id
                    .ok_or_else(|| anyhow!("could not get channel of message component"))?;
                let msg_id = data
                    .interaction
                    .message
                    .ok_or_else(|| anyhow!("interaction not run on a message component"))?
                    .id;

                let invitation = MatchmakingInvitation::find()
                    .filter(matchmaking_invitation::Column::MessageId.eq(IdWrapper::from(msg_id)))
                    .filter(matchmaking_invitation::Column::ChannelId.eq(IdWrapper::from(chan_id)))
                    .one(self.utils.db_ref())
                    .await?
                    .ok_or_else(|| anyhow!("could not find a valid invitation."))?;

                let user_model = self.utils.find_or_create_user(user.id).await?;

                if invitation.extended_to != user_model.user_id {
                    self.utils
                        .http_client
                        .interaction(self.utils.application_id)
                        .create_response(
                            data.interaction.id,
                            data.interaction.token.as_str(),
                            &InteractionResponse {
                                kind: InteractionResponseType::ChannelMessageWithSource,
                                data: Some(
                                    InteractionResponseDataBuilder::new()
                                        .content("You were not invited to this match.".to_string())
                                        .flags(MessageFlags::EPHEMERAL)
                                        .build(),
                                ),
                            },
                        )
                        .exec()
                        .await?;

                    return Ok(());
                }

                let opponent = Users::find_by_id(invitation.invited_by)
                    .one(self.utils.db_ref())
                    .await?
                    .ok_or_else(|| {
                        anyhow!("could not find user information for the person that invited you")
                    })?;

                let author_data: Member = self
                    .utils
                    .http_client
                    .guild_member(
                        guild_id,
                        opponent
                            .discord_user
                            .ok_or_else(|| anyhow!("user does not have a discord id"))?
                            .into(),
                    )
                    .exec()
                    .await?
                    .model()
                    .await?;

                let opponent_data: Member = self
                    .utils
                    .http_client
                    .guild_member(
                        guild_id,
                        user_model
                            .discord_user
                            .ok_or_else(|| anyhow!("user does not have a discord id"))?
                            .into(),
                    )
                    .exec()
                    .await?
                    .model()
                    .await?;
                let message_id = invitation
                    .message_id
                    .ok_or_else(|| anyhow!("no invitation message id found"))?;

                let thread = self
                    .start_matchmaking_thread(
                        guild_id,
                        message_id.into_id(),
                        format!(
                            "{} vs {}",
                            author_data.nick.unwrap_or(author_data.user.name),
                            opponent_data.nick.unwrap_or(opponent_data.user.name)
                        ),
                    )
                    .await?;

                let users = vec![author_data.user.id, opponent_data.user.id];
                let res = self.add_users_to_thread(thread.id, &users).await;

                if let Err(e) = res {
                    // Close the thread and send an error.

                    self.utils
                        .http_client
                        .delete_channel(thread.id)
                        .exec()
                        .await?;

                    return Err(e);
                }

                self.send_thread_opening_message(&users, thread.id).await?;

                let started_at = Utc::now();

                let owner = self.utils.find_or_create_user(author_data.user.id).await?;

                let lobby = matchmaking_lobbies::Model {
                    id: Uuid::new_v4(),
                    started_at,
                    timeout_after: started_at + chrono::Duration::hours(3),
                    channel_id: thread.id.into(),
                    description: None,
                    owner: owner.user_id,
                    privacy: LobbyPrivacy::Open,
                    game: None,
                    game_other: None,
                    ended_at: None,
                };

                for user in users {
                    // Create a discord user, in case they don't exist.
                    // TODO: Do this in bulk
                    self.utils.find_or_create_user(user).await?;
                }

                let _res = matchmaking_lobbies::Entity::insert(lobby.into_active_model())
                    .exec(self.utils.db_ref())
                    .await?;

                self.utils
                    .http_client
                    .update_message(invitation.channel_id.into(), message_id.into_id())
                    .components(Some(&[]))?
                    .exec()
                    .await?;

                Ok(())
            }
            "deny" => {
                // validate users
                let msg = data
                    .interaction
                    .message
                    .ok_or_else(|| anyhow!("interaction not run on a message component"))?;

                let invitation = MatchmakingInvitation::find()
                    .filter(matchmaking_invitation::Column::MessageId.eq(IdWrapper::from(msg.id)))
                    .one(self.utils.db_ref())
                    .await?
                    .ok_or_else(|| anyhow!("could not find that match invitation"))?;

                // TODO: Cache this
                let guild = self
                    .utils
                    .http_client
                    .guild(
                        data.interaction
                            .guild_id
                            .ok_or_else(|| anyhow!("you cannot use this command in a dm"))?,
                    )
                    .exec()
                    .await?
                    .model()
                    .await?;

                let settings = self.utils.get_guild_settings(guild.id).await?;

                let is_admin = if let Some(admin_role) = settings.admin_role {
                    member.roles.contains(&admin_role.into_id())
                } else {
                    false
                };

                let user_model = self.utils.find_or_create_user(user.id).await?;

                // cancel invitation
                if (user_model.user_id != invitation.extended_to
                    && user_model.user_id != invitation.invited_by)
                    || is_admin
                {
                    return Err(anyhow!("not authorized to deny that invitation"));
                }

                let dm_res = self
                    .dm_users_upon_cancellation(&invitation, &user, &guild)
                    .await;

                if let Err(e) = dm_res {
                    error!(err = ?e, "could not dm user");

                    // why wont this auto format?
                    self.utils
                        .http_client
                        .interaction(self.utils.application_id)
                        .create_response(data.interaction.id, data.interaction.token.as_str(),
                        &InteractionResponse { kind: InteractionResponseType::ChannelMessageWithSource, data: Some(
                            InteractionResponseDataBuilder::new()
                        .flags(MessageFlags::EPHEMERAL)
                        .embeds([
                            EmbedBuilder::new().title("DM Error").description("Could not inform one or more of the users about the match cancellation.")
                            .field(EmbedFieldBuilder::new("error", "The bot successfully denied the invitation, but it could not DM one or more of the users about the cancellation. Please notify them manually.").build())
                            .build(),
                        ])
                        .build()
                        ) }
                    )
                        .exec()
                        .await?;
                }

                // Remove the Accept/Deny buttons from the message
                if let Some(msg_id) = invitation.message_id {
                    self.utils
                        .http_client
                        .update_message(invitation.channel_id.into_id(), msg_id.into_id())
                        .components(Some(&[]))?
                        .content(None)?
                        .exec()
                        .await?;
                }

                self.utils
                    .http_client
                    .interaction(self.utils.application_id)
                    .create_response(
                        data.interaction.id,
                        data.interaction.token.as_str(),
                        &InteractionResponse {
                            kind: InteractionResponseType::ChannelMessageWithSource,
                            data: Some(
                                InteractionResponseDataBuilder::new()
                                    .content("Invitation cancelled.".to_string())
                                    .flags(MessageFlags::EPHEMERAL)
                                    .build(),
                            ),
                        },
                    )
                    .exec()
                    .await?;

                MatchmakingInvitation::update(matchmaking_invitation::ActiveModel {
                    id: Set(invitation.id),
                    expires_at: Set(Utc::now()), // TODO: Set the invitation as "Denied"
                    ..Default::default()
                })
                .exec(self.utils.db_ref())
                .await?;

                Ok(())
            }
            _ => return Err(anyhow!("no handler for action: {}", data.action)),
        }
    }
}

impl MatchmakingCommandHandler {
    pub fn new(utils: Arc<CommonUtilities>) -> Self {
        // TODO: Start a thread to keep track of the matchmaking instances.
        let utils_bg = utils.clone();
        let background_task = tokio::task::spawn(async move {
            let bg = BackgroundLoop::new(utils_bg);
            loop {
                if let Err(e) = bg.background_loop().await {
                    error!(error = ?e, "background loop update failed");
                }
            }
        });

        Self {
            utils,
            _background_task: background_task,
        }
    }

    async fn send_thread_opening_message(
        &self,
        users: impl IntoIterator<Item = &Id<UserMarker>>,
        channel: Id<ChannelMarker>,
    ) -> anyhow::Result<()> {
        let _msg = self
            .utils
            .http_client
            .create_message(channel)
            .allowed_mentions(Some(
                &AllowedMentionsBuilder::new()
                    .user_ids(users.into_iter().copied())
                    .build(),
            ))
            .embeds(&[EmbedBuilder::new()
                .description(
                    "**Thank you for using Runback. \
                        Below are a list of commands to assist you during your matches.**",
                )
                .field(
                    EmbedFieldBuilder::new(
                        "/matchmaking report",
                        "Report the score for your match",
                    )
                    .build(),
                )
                .field(
                    EmbedFieldBuilder::new(
                        "/matchmaking done",
                        "Finish matchmaking and finalize results",
                    )
                    .build(),
                )
                .field(
                    EmbedFieldBuilder::new(
                        "/matchmaking settings",
                        "Set the settings of the lobby.",
                    )
                    .build(),
                )
                .validate()?
                .build()])?
            .exec()
            .await?;

        Ok(())
    }

    async fn start_matchmaking_thread(
        &self,
        guild: Id<GuildMarker>,
        message: Id<MessageMarker>,
        name: String,
    ) -> anyhow::Result<Channel> {
        let settings = self.utils.get_guild_settings(guild).await?;

        if let Some(channel) = settings.channel_id {
            let channel = channel.into_id();

            let thread = self
                .utils
                .http_client
                .create_thread_from_message(channel, message, name.as_str())?
                // .invitable(true)
                // archive in 3 hours
                .auto_archive_duration(AutoArchiveDuration::Day)
                .exec()
                .await?
                .model()
                .await?;

            return Ok(thread);
        }

        Err(anyhow!(
            "The server has not enabled a default matchmaking channel"
        ))
    }

    async fn add_users_to_thread(
        &self,
        thread_id: Id<ChannelMarker>,
        users: impl IntoIterator<Item = &Id<UserMarker>>,
    ) -> anyhow::Result<()> {
        self.utils.http_client.join_thread(thread_id).exec().await?;

        for user in users {
            self.utils
                .http_client
                .add_thread_member(thread_id, *user)
                .exec()
                .await?;
        }

        Ok(())
    }

    async fn dm_users_upon_cancellation(
        &self,
        invitation: &matchmaking_invitation::Model,
        user: &User,
        guild: &Guild,
    ) -> anyhow::Result<()> {
        let user_model = self.utils.find_or_create_user(user.id).await?;

        if user_model.user_id != invitation.invited_by {
            let author = Users::find_by_id(invitation.invited_by)
                .one(self.utils.db_ref())
                .await?
                .ok_or_else(|| anyhow!("no user found with that id"))?;
            let res = self
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
                .one(self.utils.db_ref())
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
            .utils
            .http_client
            .create_private_channel(user)
            .exec()
            .await?
            .model()
            .await?;

        debug_assert_eq!(
            dm.kind,
            ChannelType::Private,
            "DM channel created by Discord was not private"
        );

        let msg = self
            .utils
            .http_client
            .create_message(dm.id)
            .content(
                format!(
                    "\"{}@{}\" cancelled your matchmaking request in \"{}\"",
                    canceller.name, canceller.discriminator, guild.name
                )
                .as_str(),
            )?
            .exec()
            .await?
            .model()
            .await?;

        Ok(msg)
    }
}

struct BackgroundLoop {
    utils: Arc<CommonUtilities>,
}

impl BackgroundLoop {
    fn new(utils: Arc<CommonUtilities>) -> Self {
        Self {
            utils: utils.clone(),
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
            .all(self.utils.db_ref())
            .await?;

        Ok(lobbies)
    }

    async fn extend_lobby(&self, s: &matchmaking_lobbies::Model) -> anyhow::Result<()> {
        let lobby = matchmaking_lobbies::ActiveModel {
            id: Set(s.id),
            timeout_after: Set(Utc::now() + chrono::Duration::minutes(30)),
            ..Default::default()
        };
        debug!(lobby = ?lobby.id, "extending lobby session");

        MatchmakingLobbies::update(lobby)
            .exec(self.utils.db_ref())
            .await?;

        Ok(())
    }

    async fn check_if_lobby_should_be_extended(
        &self,
        s: &matchmaking_lobbies::Model,
    ) -> anyhow::Result<bool> {
        let chan = self
            .utils
            .http_client
            .channel(s.channel_id.into_id())
            .exec()
            .await?
            .model()
            .await?;

        if let Some(msg) = chan.last_message_id {
            let msg = self
                .utils
                .http_client
                .message(chan.id, msg.cast())
                .exec()
                .await?
                .model()
                .await?;

            let now =
                DateTime::<FixedOffset>::from_utc(Utc::now().naive_utc(), FixedOffset::east(0));
            let last_message_sent_at = chrono::DateTime::parse_from_rfc3339(
                msg.timestamp.iso_8601().to_string().as_str(),
            )?;

            // Check if the last message was sent in the last 30 minutes
            // If it was, then extend the expiration time by a half hour.
            // Otherwise, send the expiration warning.

            if last_message_sent_at > (now - chrono::Duration::minutes(30)) {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    #[instrument(skip_all)]
    async fn send_expiration_warning_message(
        &self,
        s: &matchmaking_lobbies::Model,
    ) -> anyhow::Result<()> {
        let _msg = self
            .utils
            .http_client
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
            .exec()
            .await?
            .model()
            .await?;

        Ok(())
    }

    #[instrument(skip_all)]
    async fn timeout_expired_lobby(
        &self,
        s: &entity::matchmaking_lobbies::Model,
    ) -> anyhow::Result<()> {
        let chan_id = s.channel_id.into_id();
        let chan = self
            .utils
            .http_client
            .channel(chan_id)
            .exec()
            .await?
            .model()
            .await?;
        let _msg = self
            .utils
            .http_client
            .create_message(chan.id)
            .content("This matchmaking lobby has timed out. See ya later!")?
            .exec()
            .await?;
        if chan.kind.is_thread() {
            let _thread = self
                .utils
                .http_client
                .update_thread(chan_id)
                .archived(true)
                .exec()
                .await?;
        }

        // Close any matchmaking invitations.
        self.deactivate_invitations_upon_closing_lobby(s).await?;

        Ok(())
    }

    async fn deactivate_invitations_upon_closing_lobby(
        &self,
        lobby: &matchmaking_lobbies::Model,
    ) -> anyhow::Result<()> {
        let _update_res = entity::matchmaking_invitation::Entity::update_many()
            .filter(matchmaking_invitation::Column::Lobby.eq(lobby.id))
            .filter(matchmaking_invitation::Column::ExpiresAt.gt(Utc::now()))
            .set(matchmaking_invitation::ActiveModel {
                expires_at: Set(Utc::now()),
                ..Default::default()
            })
            .exec(self.utils.db_ref())
            .await?;
        Ok(())
    }

    async fn get_expired_lobbies(&self) -> Result<Vec<matchmaking_lobbies::Model>, anyhow::Error> {
        let lobbies = MatchmakingLobbies::find()
            .filter(matchmaking_lobbies::Column::TimeoutAfter.lte(Utc::now()))
            .filter(matchmaking_lobbies::Column::EndedAt.is_null())
            .all(self.utils.db_ref())
            .await?;

        Ok(lobbies)
    }

    /// This function should only return catestrophic errors!
    #[instrument(skip_all)]
    async fn background_loop(&self) -> anyhow::Result<()> {
        let mut stream = {
            self.utils
                .standby
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
            .one(self.utils.db_ref())
            .await?;

        if let Some(lobby) = lobby {
            // Delete the lobby and de-activate all invitations.
            self.deactivate_invitations_upon_closing_lobby(&lobby)
                .await?;
        }

        Ok(())
    }
}
