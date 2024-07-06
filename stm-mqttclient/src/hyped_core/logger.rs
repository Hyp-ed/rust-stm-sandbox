#![allow(dead_code)]

#[derive(PartialEq, PartialOrd)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

pub enum LogTarget {
    Console,
    Mqtt,
}

pub struct Logger {
    level: LogLevel,
    target: LogTarget,
}

impl Logger {
    pub fn new(level: LogLevel, target: LogTarget) -> Self {
        Self { level, target }
    }

    pub fn log(&self, level: LogLevel, message: &str) {
        if level >= self.level {
            match self.target {
                LogTarget::Console => match level {
                    LogLevel::Debug => defmt::debug!("{}", message),
                    LogLevel::Info => defmt::info!("{}", message),
                    LogLevel::Warn => defmt::warn!("{}", message),
                    LogLevel::Error => defmt::error!("{}", message),
                },
                LogTarget::Mqtt => {
                    // Send message to MQTT
                }
            }
        }
    }
}
