pub mod application_commands;

use twilight_model::gateway::payload::incoming::InteractionCreate;

#[tracing::instrument]
pub async fn handle_interaction(interaction: Box<InteractionCreate>) {
    let i = &**interaction;

    match i {
        twilight_model::application::interaction::Interaction::Ping(_) => unreachable!(),
        twilight_model::application::interaction::Interaction::ApplicationCommand(_) => todo!(),
        twilight_model::application::interaction::Interaction::ApplicationCommandAutocomplete(
            _,
        ) => todo!(),
        twilight_model::application::interaction::Interaction::MessageComponent(_) => todo!(),
        _ => debug!("Unhandled interaction")
    }

    debug!(interaction = %format!("{:?}", interaction));
}
