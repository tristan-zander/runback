use tracing::Level;

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub enum LogLevel {
    TRACE,
    DEBUG,
    INFO,
    WARN,
    ERROR,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::DEBUG
    }
}

impl Into<Level> for LogLevel {
    fn into(self) -> Level {
        match self {
            Self::TRACE => Level::TRACE,
            Self::DEBUG => Level::DEBUG,
            Self::INFO => Level::INFO,
            Self::WARN => Level::WARN,
            Self::ERROR => Level::ERROR,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum LogDriver {
    Print,
    JSON,
}

impl Default for LogDriver {
    fn default() -> Self {
        Self::Print
    }
}
