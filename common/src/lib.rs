extern crate openidconnect;
extern crate reqwest;

use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    reqwest::{async_http_client, http_client},
    url::ParseError,
    ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    RedirectUrl, Scope,
};
use std::{boxed::Box, error::Error, fmt::Display, string::String};

#[derive(Debug, Clone)]
pub struct OpenIDUtil {
    pub client: CoreClient,
    pub client_id: ClientId,
    pub client_secret: Option<ClientSecret>,
}

#[derive(Debug)]
struct OpenIDError {
    message: String,
}

impl Display for OpenIDError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenID Error: {}", self.message)
    }
}

impl Error for OpenIDError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

/// This utility is *only* for clients that are going to use the client_credentials grant type.
impl OpenIDUtil {
    pub async fn new(
        client_id: String,
        client_secret: Option<String>,
        keycloak_url: String,
        redirect_url: Option<String>,
    ) -> Result<OpenIDUtil, Box<dyn Error>> {
        if client_id.len() <= 0 {
            return Err(Box::new(OpenIDError {
                message: "Client ID cannot be empty.".into(),
            }));
        }
        let client_id = ClientId::new(client_id);
        let client_secret = match client_secret {
            Some(c) => {
                if c.len() <= 0 {
                    return Err(Box::new(OpenIDError {
                        message: "Client secret cannot be empty.".into(),
                    }));
                }
                Some(ClientSecret::new(c))
            }
            None => None,
        };
        let url = IssuerUrl::new(keycloak_url)?;

        let metadata = CoreProviderMetadata::discover_async(url, async_http_client).await?;

        // There's probably a way to change the redirect_url without moving from self
        let client = match redirect_url {
            Some(url) => CoreClient::from_provider_metadata(
                metadata,
                client_id.clone(),
                client_secret.clone(),
            )
            .set_redirect_uri(RedirectUrl::new(url)?),
            None => CoreClient::from_provider_metadata(
                metadata,
                client_id.clone(),
                client_secret.clone(),
            ),
        };

        Ok(OpenIDUtil {
            client,
            client_id,
            client_secret,
        })
    }
}
