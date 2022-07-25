pub mod application_commands;
pub mod panels;

use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    sync::Arc,
};

use dashmap::DashMap;
use entity::sea_orm::{prelude::Uuid, DatabaseConnection};

use futures::future::BoxFuture;
use tracing::Level;
use twilight_cache_inmemory::InMemoryCache;
use twilight_gateway::Shard;
use twilight_model::{
    application::command::Command,
    gateway::payload::incoming::InteractionCreate,
    id::{marker::CommandMarker, Id},
};
use twilight_standby::Standby;

use crate::interactions::application_commands::{
    lfg::LfgCommandHandler, ApplicationCommandUtilities, InteractionData,
};

use self::application_commands::{
    admin::admin_handler::AdminCommandHandler, eula::EulaCommandHandler,
    matchmaking::MatchmakingCommandHandler, CommandGroupDescriptor, InteractionHandler,
    PingCommandHandler,
};

pub struct InteractionProcessor {
    utils: Arc<ApplicationCommandUtilities>,
    handlers: HashMap<Id<CommandMarker>, Arc<Box<dyn InteractionHandler + Send + Sync + 'static>>>,
    commands: Vec<Command>,
    command_groups: Vec<CommandGroupDescriptor>,
}

impl InteractionProcessor {
    pub async fn init(
        db: Arc<Box<DatabaseConnection>>,
        cache: Arc<InMemoryCache>,
        standby: Arc<Standby>,
    ) -> anyhow::Result<Self> {
        let utils = Arc::new(ApplicationCommandUtilities::new(db, cache, standby).await?);
        let lfg_sessions = Arc::new(DashMap::new());

        event!(Level::INFO, "Registering top-level command handlers");

        let top_level_handlers: Vec<Arc<Box<dyn InteractionHandler + Send + Sync + 'static>>> = vec![
            Arc::new(Box::new(PingCommandHandler {
                utils: utils.clone(),
            })),
            Arc::new(Box::new(AdminCommandHandler::new(utils.clone()))),
            Arc::new(Box::new(MatchmakingCommandHandler {})),
            Arc::new(Box::new(EulaCommandHandler::new(utils.clone()))),
            Arc::new(Box::new(LfgCommandHandler {
                utils: utils.clone(),
                lfg_sessions,
            })),
        ];

        let mut this = Self {
            commands: Vec::new(),
            command_groups: Vec::with_capacity(top_level_handlers.len()),
            utils,
            handlers: HashMap::new(),
        };

        this.register_commands(top_level_handlers).await?;

        Ok(this)
    }

    // Man this is so ugly
    async fn register_commands(
        &mut self,
        handlers: Vec<Arc<Box<dyn InteractionHandler + Send + Sync + 'static>>>,
    ) -> anyhow::Result<()> {
        let groups = handlers
            .into_iter()
            .map(|h| (h.clone(), h.describe()))
            .collect::<Vec<_>>();

        self.commands = {
            let commands = groups
                .iter()
                .flat_map(|grp| {
                    grp.1
                        .commands
                        .into_iter()
                        .map(|c| c.to_owned())
                        .collect::<Vec<Command>>()
                })
                .collect::<Vec<_>>();

            self.utils
                .http_client
                .interaction(self.utils.application_id)
                .set_guild_commands(crate::CONFIG.debug_guild_id.unwrap(), commands.as_slice())
                .exec()
                .await?
                .models()
                .await?
        };

        debug!(commands = ?serde_json::to_string(&self.commands).unwrap(), "Sent commands to discord");

        self.command_groups = groups.iter().map(|g| g.1.clone()).collect();
        for (handler, descriptor) in groups {
            for c in self.commands.iter() {
                if c.id.is_none()
                    || descriptor
                        .commands
                        .into_iter()
                        .find(|desc_comm| desc_comm.name == c.name)
                        .is_none()
                {
                    debug!(command = ?c.name, "Command does not have an id");
                    continue;
                }

                if let Some(old) = self.handlers.insert(
                    c.id.ok_or_else(|| anyhow!("Command does not have an id: {}", c.name))?,
                    handler.clone(),
                ) {
                    return Err(anyhow!(
                        "Inserted a handler over a command... {:#?}",
                        old.describe().name
                    ));
                }
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn handle_interaction<'shard>(
        &self,
        interaction: InteractionCreate,
        _shard: &'shard Shard,
    ) -> anyhow::Result<BoxFuture<'static, anyhow::Result<()>>> {
        event!(tracing::Level::DEBUG, "Received interaction");

        // TODO: Send a deferred message response, followup on it later

        match &*interaction {
            // I think this is only for webhook interaction handlers
            // twilight_model::application::interaction::Interaction::Ping(_) => ,
            twilight_model::application::interaction::Interaction::ApplicationCommand(command) => {
                // self.application_command_handlers
                //     .on_command_receive(command.as_ref())
                //     .await?;
                if let Some(handler) = self.handlers.get(&command.data.id) {
                    let data = Box::new(InteractionData {
                        command: *command.to_owned(),
                        id: Uuid::new_v4(),
                    });
                    let handler = handler.clone();
                    let fut = Box::pin(async move { handler.process_command(data).await });
                    return Ok(fut);
                } else {
                    error!(name = ?command.data.name,"No command handler found");
                    return Err(anyhow!("No such command handler found"));
                }
            }
            // twilight_model::application::interaction::Interaction::ApplicationCommandAutocomplete(
            //     _,
            // ) => todo!(),
            twilight_model::application::interaction::Interaction::MessageComponent(_message) => {
                debug!("Received message component")
            }
            twilight_model::application::interaction::Interaction::ModalSubmit(_modal) => {
                debug!("Received modal");
            }
            _ => {
                debug!(interaction = %format!("{:?}", interaction), "Unhandled interaction");
            }
        }

        trace!("Ended interaction response.");
        return Err(anyhow!("Interaction was unmatched"));
    }
}
