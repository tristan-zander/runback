#[macro_use]
extern crate rocket;
#[macro_use]
extern crate tracing;

mod config;
mod entities;

use std::error::Error;

use common::OpenIDUtil;
use config::Config;
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    reqwest::{async_http_client, http_client},
    url::ParseError,
    ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    RedirectUrl, Scope,
};
use rocket::State;

#[get("/")]
#[tracing::instrument]
async fn index(state: &State<OpenIDUtil>) -> std::string::String {
    let creds = state
        .client
        .exchange_client_credentials()
        .request_async(async_http_client)
        .await;

    match creds {
        Ok(x) => {
            let client_secret = x.access_token().secret();
            trace!(client_secret = ?client_secret, "New Client Secret");
            return "Success!".into();
        }
        Err(e) => {
            error!(error = ?e, "Could not get client secret");
            return format!("{:?}", e);
        }
    }
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_target(true)
        .with_max_level(tracing::Level::TRACE)
        // .json()
        .init();

    let config = Config::new()?;

    // Setup some test data.
    let openid_util = match common::OpenIDUtil::new(
        config.auth.client_id.clone(),
        config.auth.client_secret.clone(),
        config.auth.keycloak_realm.to_string(),
        None,
    )
    .await
    {
        Ok(o) => o,
        Err(e) => {
            error!(error = ?e, "Could not create OpenID toolkit");
            return Err(e);
        }
    };

    info!("Attempting to start webserver");

    rocket::build()
        .mount("/", routes![index])
        .manage(openid_util)
        .manage(config)
        .launch()
        .await?;

    Ok(())
}
