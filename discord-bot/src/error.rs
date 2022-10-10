use std::error::Error;

use sea_orm::DbErr;
use tracing::instrument::Instrumented;
use twilight_gateway::cluster::ClusterStartError;
use twilight_http::response::DeserializeBodyError;
use twilight_validate::message::MessageValidationError;

#[derive(Debug)]
pub struct RunbackError {
    pub message: String,
    pub inner: Option<Box<dyn Error + 'static>>,
}

impl Error for RunbackError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.inner {
            Some(e) => Some(e.as_ref()),
            None => None,
        }
    }
}

impl std::fmt::Display for RunbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<DbErr> for RunbackError {
    fn from(e: DbErr) -> Self {
        RunbackError {
            message: "Database Error".to_owned(),
            inner: Some(e.into()),
        }
    }
}

impl From<ClusterStartError> for RunbackError {
    fn from(e: ClusterStartError) -> Self {
        RunbackError {
            message: "Cluster Start Error".to_owned(),
            inner: Some(e.into()),
        }
    }
}

impl From<twilight_http::Error> for RunbackError {
    fn from(e: twilight_http::Error) -> Self {
        RunbackError {
            message: format!("Twilight HTTP Error: {}", e),
            inner: Some(e.into()),
        }
    }
}

impl From<Box<dyn Error>> for RunbackError {
    fn from(e: Box<dyn Error>) -> Self {
        RunbackError {
            message: format!("Unknown error: {}", e),
            inner: Some(e),
        }
    }
}

impl From<DeserializeBodyError> for RunbackError {
    fn from(e: DeserializeBodyError) -> Self {
        RunbackError {
            message: "Error deserializing message body".to_owned(),
            inner: Some(e.into()),
        }
    }
}

impl From<Instrumented<RunbackError>> for RunbackError {
    fn from(e: Instrumented<RunbackError>) -> Self {
        e.into_inner()
    }
}

impl From<String> for RunbackError {
    fn from(e: String) -> Self {
        RunbackError {
            message: e,
            inner: None,
        }
    }
}

impl From<&str> for RunbackError {
    fn from(message: &str) -> Self {
        RunbackError {
            message: message.to_owned(),
            inner: None,
        }
    }
}

impl From<MessageValidationError> for RunbackError {
    fn from(e: MessageValidationError) -> Self {
        RunbackError {
            message: "Unable to validate message".to_owned(),
            inner: Some(e.into()),
        }
    }
}
