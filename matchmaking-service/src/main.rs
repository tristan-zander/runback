#[macro_use]
extern crate rocket;
extern crate openidconnect;
extern crate reqwest;

use common::OpenIDUtil;
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    reqwest::{async_http_client, http_client},
    url::ParseError,
    ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    RedirectUrl, Scope,
};
use rocket::State;

#[get("/")]
async fn index(state: &State<OpenIDUtil>) -> std::string::String {
    let creds = state
        .client
        .exchange_client_credentials()
        .request_async(async_http_client)
        .await;

    match creds {
        Ok(x) => {
            println!("{:?}", x.access_token().secret().clone());
            return "Success!".into();
        }
        Err(e) => return format!("{:?}", e),
    }
}

#[launch]
async fn rocket() -> _ {
    // Setup some test data.
    let openid_util = common::OpenIDUtil::new(
        "matchmaking".into(),
        Some("get-from-env".into()),
        "http://localhost:8080/auth/realms/demo".into(),
        None,
    )
    .await
    .expect("Couldn't create openid tools");

    rocket::build()
        .mount("/", routes![index])
        .manage(openid_util)
}
