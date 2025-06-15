#[cfg(test)]
use crate::config;

#[cfg(test)]
pub fn test_config(url: String) -> config::Aiseg2Config {
    config::Aiseg2Config {
        url,
        user: "test_user".to_string(),
        password: "test_password".to_string(),
    }
}
