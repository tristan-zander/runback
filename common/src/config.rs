use std::path::Path;

use reqwest::Url;

use crate::logging::{LogDriver, LogLevel};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Logging {
    pub log_level: LogLevel,
    pub log_driver: LogDriver,
    pub log_to_file: Option<Box<Path>>,
    /// Specifically, add extra information about stats like thread ID, file name, etc. Only useful for debugging
    pub too_much_information: bool,
}
