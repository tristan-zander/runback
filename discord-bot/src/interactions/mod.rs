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
    application::command::Command, gateway::payload::incoming::InteractionCreate,
};
use twilight_standby::Standby;

use crate::interactions::application_commands::{
    lfg::LfgCommandHandler, ApplicationCommandUtilities, InteractionData,
};

use self::application_commands::{
    admin::admin_handler::AdminCommandHandler, eula::EulaCommandHandler,
    matchmaking::MatchmakingCommandHandler, ApplicationCommandHandler, CommandGroupDescriptor,
    HandlerType, PingCommandHandler,
};

pub struct InteractionHandler {
    utils: Arc<ApplicationCommandUtilities>,
    command_map: HashMap<u64, HandlerType>,
    #[allow(unused)]
    commands: Vec<Command>,
    #[allow(unused)]
    command_groups: Vec<CommandGroupDescriptor>,
}

impl InteractionHandler {
    pub async fn init(
        db: Arc<Box<DatabaseConnection>>,
        cache: Arc<InMemoryCache>,
        standby: Arc<Standby>,
    ) -> anyhow::Result<Self> {
        let utils = Arc::new(ApplicationCommandUtilities::new(db, cache, standby).await?);
        let lfg_sessions = Arc::new(DashMap::new());

        event!(Level::INFO, "Registering top-level command handlers");

        let top_level_handlers: Vec<Box<dyn ApplicationCommandHandler + Send + Sync>> = vec![
            Box::new(PingCommandHandler {
                utils: utils.clone(),
            }),
            Box::new(AdminCommandHandler::new(utils.clone())),
            Box::new(MatchmakingCommandHandler {}),
            Box::new(EulaCommandHandler::new(utils.clone())),
            Box::new(LfgCommandHandler {
                utils: utils.clone(),
                lfg_sessions,
            }),
        ];

        let mut command_map = HashMap::new();

        let mut command_groups = Vec::with_capacity(top_level_handlers.len());

        let commands = top_level_handlers
            .iter()
            .flat_map(|handler| {
                let command_group = handler.register();
                command_groups.push(command_group.to_owned());

                command_group
                    .commands
                    .into_iter()
                    .map(|grp| {
                        let comm = grp.command.to_owned();

                        if let Some(h) = grp.handler {
                            command_map.insert(Self::hash_command_name(comm.name.as_str()), h);
                        }

                        comm
                    })
                    .collect::<Vec<Command>>()
            })
            .collect::<Vec<_>>();

        let _res = utils
            .http_client
            .interaction(utils.application_id)
            .set_guild_commands(crate::CONFIG.debug_guild_id.unwrap(), commands.as_slice())
            .exec()
            .await?
            .models()
            .await?;

        Ok(Self {
            command_map,
            commands,
            command_groups,
            utils,
        })
    }

    fn hash_command_name(name: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        let res = hasher.finish();
        trace!(name = ?name, hashed = ?res, "Hashed command name");
        res
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
                if let Some(handler) = self
                    .command_map
                    .get(&Self::hash_command_name(command.data.name.as_str()))
                {
                    let data = Box::new(InteractionData {
                        command: *command.to_owned(),
                        id: Uuid::new_v4(),
                    });
                    let fut = (handler)(self.utils.clone(), data);
                    // self.executing_futures.push(fut);
                    return Ok(fut);
                    // handler
                    //     .execute(InteractionData {
                    //         command: *command.to_owned(),
                    //         id: Uuid::new_v4(),
                    //     })
                    //     .await
                    //     .map_err(|e| RunbackError {
                    //         message: "Application Command error".to_string(),
                    //         inner: Some(e.into()),
                    //     })?;
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
                return Err(anyhow!("Interaction was unmatched"));
            }
        }

        // TODO: Do some error handling, send a message back to the user
        // if let Err(e) = res {
        //     error!(error = %e, "Unhandled interaction error");
        // }

        trace!("Ended interaction response.");
        return Err(anyhow!("Interaction was unmatched"));
    }
}
