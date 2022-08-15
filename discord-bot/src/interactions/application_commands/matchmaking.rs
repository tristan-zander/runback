use chrono::Utc;
use dashmap::DashMap;
use entity::sea_orm::prelude::{DateTimeUtc, Uuid};
use tokio::task::JoinHandle;
use twilight_gateway::Event;
use twilight_model::{
    application::{
        command::{BaseCommandOptionData, ChoiceCommandOptionData, CommandOption, CommandType},
        component::{button::ButtonStyle, ActionRow, Button, Component},
    },
    channel::{
        message::{allowed_mentions::AllowedMentionsBuilder, MessageFlags},
        thread::AutoArchiveDuration,
        Channel,
    },
    guild::Member,
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{
        marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker},
        Id,
    },
};
use twilight_util::builder::{
    command::{CommandBuilder, SubCommandBuilder},
    embed::{EmbedBuilder, EmbedFieldBuilder},
    InteractionResponseDataBuilder,
};

use super::{
    ApplicationCommandData, ApplicationCommandUtilities, CommandGroupDescriptor,
    InteractionHandler, MessageComponentData,
};

use futures::StreamExt;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

// TODO: Make this 3 hours after testing
// const TIMEOUT_AFTER: Duration = chrono::Duration::hours(3);
static TIMEOUT_AFTER: Duration = Duration::from_secs(60);

// TODO: Don't use a model like this. Use the sea_orm model that's stored in the database
#[derive(Debug, Clone)]
struct Session {
    pub id: Uuid,
    pub users: Vec<Id<UserMarker>>,
    pub thread: Id<ChannelMarker>,
    pub started_at: DateTimeUtc,
    pub timeout_after: DateTimeUtc,
}

#[derive(Debug, Clone)]
struct MatchInvitation {
    pub id: Uuid,
    /// The user that created the invitation
    pub author: Id<UserMarker>,
    pub invited: Option<Id<UserMarker>>,
    pub message_id: Id<MessageMarker>,
    pub channel_id: Id<ChannelMarker>,
    pub timeout_after: DateTimeUtc,
}

pub struct MatchmakingCommandHandler {
    utils: Arc<ApplicationCommandUtilities>,
    sessions: Arc<DashMap<Id<ChannelMarker>, Session>>,
    invitations: Arc<DashMap<Id<MessageMarker>, MatchInvitation>>,
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
            .option(CommandOption::String(ChoiceCommandOptionData {
                autocomplete: false,
                choices: vec![],
                description: "An invite message to your opponent".to_string(),
                description_localizations: None,
                name: "invitation".to_string(),
                name_localizations: None,
                required: false,
            }))
            .build(),
        )
        // .option(
        //     SubCommandBuilder::new("show-matches".into(), "Show the matchmaking menu".into())
        //         .build(),
        // )
        .option(
            SubCommandBuilder::new(
                "settings".into(),
                "View and update settings such as default character".into(),
            )
            .build(),
        )
        .option(
            SubCommandBuilder::new("done".into(), "Finish your matchmaking session".into()).build(),
        )
        .option(
            SubCommandBuilder::new("report-score".into(), "Report the score of a match".into())
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
        if let Some(_target) = data.command.data.target_id {
            // Then start a mm session with that user. It's not chat message command,
            // but a click interaction.
        }

        let member = data
            .command
            .member
            .ok_or_else(|| anyhow!("command cannot be run in a DM"))?;

        let user = member
            .user
            .ok_or_else(|| anyhow!("could not get user data for caller"))?;

        let resolved = data
            .command
            .data
            .resolved
            .ok_or_else(|| anyhow!("cannot get the resolved command user data"))?;

        match data
            .command
            .data
            .options
            .get(0)
            .ok_or_else(|| anyhow!("could not get subcommand option"))?
            .name
            .as_str()
        {
            "play-against" => {
                let invited = resolved.users.values().nth(0).ok_or_else(|| {
                    anyhow!("cannot get the user specified in \"play-against\" command")
                })?;

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
                    channel = data.command.channel_id;
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
                        components: vec![Component::Button(Button {
                            custom_id: Some("matchmaking:accept".to_string()),
                            disabled: false,
                            emoji: None,
                            label: Some("Accept".to_string()),
                            style: ButtonStyle::Primary,
                            url: None,
                        })],
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
                    .create_followup(data.command.token.as_str())
                    .content(format!("Sent a request in <#{}>", channel).as_str())?
                    .flags(MessageFlags::EPHEMERAL)
                    .exec()
                    .await?
                    .model()
                    .await?;

                self.invitations.insert(
                    msg.id,
                    MatchInvitation {
                        id: Uuid::new_v4(),
                        author: user.id,
                        invited: Some(invited.id),
                        message_id: msg.id,
                        timeout_after: Utc::now() + chrono::Duration::minutes(15),
                        channel_id: msg.channel_id,
                    },
                );

                return Ok(());
            }
            _ => {
                return Err(anyhow!(
                    "command handler for {} not found.",
                    data.command.data.name.as_str()
                ))
            }
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
            .message
            .member
            .ok_or_else(|| anyhow!("command cannot be run in a DM"))?;

        let user = member
            .user
            .ok_or_else(|| anyhow!("could not get user data for caller"))?;

        match data.action.as_str() {
            "accept" => {
                let invitation = self
                    .invitations
                    .get(&data.message.message.id)
                    .ok_or_else(|| anyhow!("no invitation found"))?;

                if let Some(invited) = invitation.invited {
                    if invited != user.id {
                        self.utils
                            .http_client
                            .interaction(self.utils.application_id)
                            .create_response(
                                data.message.id,
                                data.message.token.as_str(),
                                &InteractionResponse {
                                    kind: InteractionResponseType::ChannelMessageWithSource,
                                    data: Some(
                                        InteractionResponseDataBuilder::new()
                                            .content(
                                                "You were not invited to this match.".to_string(),
                                            )
                                            .flags(MessageFlags::EPHEMERAL)
                                            .build(),
                                    ),
                                },
                            )
                            .exec()
                            .await?;

                        return Ok(());
                    }
                }

                let opponent = if let Some(invited) = invitation.invited {
                    invited
                } else {
                    user.id
                };

                let guild_id = data
                    .message
                    .guild_id
                    .ok_or_else(|| anyhow!("Command cannot be run in a DM"))?;

                let author_data: Member = self
                    .utils
                    .http_client
                    .guild_member(guild_id, invitation.author)
                    .exec()
                    .await?
                    .model()
                    .await?;
                let opponent_data: Member = self
                    .utils
                    .http_client
                    .guild_member(guild_id, opponent)
                    .exec()
                    .await?
                    .model()
                    .await?;

                let thread = self
                    .start_matchmaking_thread(
                        guild_id,
                        invitation.message_id,
                        format!(
                            "{} vs {}",
                            author_data.nick.unwrap_or(author_data.user.name),
                            opponent_data.nick.unwrap_or(opponent_data.user.name)
                        ),
                    )
                    .await?;

                let users = vec![opponent, invitation.author];
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
                let session = Session {
                    id: Uuid::new_v4(),
                    users,
                    thread: thread.id,
                    started_at,
                    timeout_after: started_at + chrono::Duration::from_std(TIMEOUT_AFTER).unwrap(),
                };

                self.sessions.insert(thread.id, session);

                self.utils
                    .http_client
                    .update_message(invitation.channel_id, invitation.message_id)
                    .components(Some(&[]))?
                    .exec()
                    .await?;

                Ok(())
            }
            _ => return Err(anyhow!("no handler for action: {}", data.action)),
        }
    }
}

impl MatchmakingCommandHandler {
    pub fn new(utils: Arc<ApplicationCommandUtilities>) -> Self {
        // TODO: Start a thread to keep track of the matchmaking instances.
        let sessions = Arc::new(DashMap::with_shard_amount(4));
        let invitations = Arc::new(DashMap::with_shard_amount(4));
        let background_task = tokio::task::spawn(Self::background_loop(
            sessions.clone(),
            utils.clone(),
            invitations.clone(),
        ));

        Self {
            utils,
            sessions,
            _background_task: background_task,
            invitations,
        }
    }

    #[instrument(skip_all)]
    async fn background_loop(
        sessions: Arc<DashMap<Id<ChannelMarker>, Session>>,
        utils: Arc<ApplicationCommandUtilities>,
        interactions: Arc<DashMap<Id<MessageMarker>, MatchInvitation>>,
    ) {
        let mut stream = {
            let s = sessions.clone();
            utils
                .standby
                .wait_for_event_stream(move |e: &Event| match e {
                    Event::ChannelDelete(chan) => {
                        return s.contains_key(&chan.id);
                    }
                    _ => return false,
                })
        };

        let mut interval = tokio::time::interval(Duration::from_secs(30));

        let mut thread_ids_to_remove = Vec::new();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let start = Instant::now();
                    let s_count = sessions.len();
                    debug!(num_sessions = ?s_count, "Filtering sessions");

                    let now = Utc::now();

                    sessions.retain(|_key, val: &mut Session| {
                        let res = val.timeout_after > now;
                        if res == false {
                            thread_ids_to_remove.push(val.thread);
                        }
                        res
                    });

                    debug!(time_ms = ?start.elapsed(), "Found all bad sessions");

                    for thread in &thread_ids_to_remove {
                        let fut = Self::timeout_matchmaking_session(*thread, utils.as_ref());
                        // TODO: Store this in a FuturesUnordered and send any errors back to the parent struct (Prob best to do through a channel)
                        if let Err(e) = fut.await {
                            error!(error = ?e, "Failure to delete thread");
                        }
                    }

                    thread_ids_to_remove.clear();

                    let end = start.elapsed();
                    debug!(end = ?end, time_ms = ?end.as_millis(), sessions_removed = ?s_count - sessions.len(), "Finished filtering sessions");
                }
                chan_delete = stream.next() => {
                    debug!(del = ?format!("{:?}", chan_delete), "Channel was deleted");
                }
            }
        }
    }

    async fn timeout_matchmaking_session(
        thread: Id<ChannelMarker>,
        utils: &ApplicationCommandUtilities,
    ) -> anyhow::Result<()> {
        // TODO: Send a message, declaring the timeout of the session.

        utils.http_client.delete_channel(thread).exec().await?;

        Ok(())
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
                    .user_ids(users.into_iter().map(|id| id.to_owned()))
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
}
