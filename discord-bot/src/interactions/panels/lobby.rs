use entity::matchmaking::lobby::LobbyPrivacy;
use twilight_model::channel::embed::EmbedField;
use twilight_util::builder::{embed::EmbedBuilder, InteractionResponseDataBuilder};

pub struct LobbyPanel<'a> {
    pub lobbies: &'a [entity::matchmaking::lobby::Model],
}

impl<'a> LobbyPanel<'a> {
    pub fn create(&self) -> InteractionResponseDataBuilder {
        let mut embed = EmbedBuilder::new()
            .title("Lobbies")
            .description("Join a lobby and play some games")
            .build();

        for lobby in self
            .lobbies
            .iter()
            .filter(|l| l.privacy == LobbyPrivacy::Open)
            .take(50)
        {
            embed.fields.push(EmbedField {
                inline: false,
                name: lobby.description.clone(),
                value: lobby.id.to_string(),
            })
        }

        InteractionResponseDataBuilder::new().embeds(vec![embed])
    }
}
