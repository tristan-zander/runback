#[macro_use]
extern crate rocket;
#[macro_use]
extern crate tracing;

mod config;
mod entities;
mod events;

use std::{
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use common::auth::OpenIDUtil;
use config::Config;
use openidconnect::{
    reqwest::{async_http_client}, OAuth2TokenResponse,
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

    let should_run = Arc::new(AtomicBool::new(true));

    // Setup some test data.
    let openid_util = OpenIDUtil::new(
        config.auth.client_id.clone(),
        config.auth.client_secret.clone(),
        config.auth.keycloak_realm.to_string(),
        None,
    )
    .await?;

    info!("Attempting to start webserver");

    let rocket = rocket::build()
        .mount("/", routes![index])
        .manage(openid_util)
        .manage(config.clone())
        .ignite()
        .await?;

    let rocket_ev_loop = rocket::tokio::task::spawn(async move {
        let err = match rocket.launch().await {
            Err(e) => {
                error!(error = %e, message = "Rocket shut down with errors");
                Some(e)
            }
            Ok(_) => {
                info!("Rocket received shutdown event.");
                None
            }
        };
        should_run.store(false, Ordering::Release);
        err
    });

    let mut kafka = events::EventLoop::new(config.events.clone())?;
    kafka.fake_event_loop().await;

    rocket_ev_loop.await?;
    Ok(())
}
