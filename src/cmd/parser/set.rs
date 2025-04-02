use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::cmd::request::ClientError; // TODO circular dependency?

#[derive(Debug, PartialEq)]
pub struct Set {
    pub key: String,
    pub value: String,
    pub expiration: Option<SystemTime>,
}

impl Set {
    pub fn parse(params: Vec<String>) -> Result<Self, ClientError> {
        if params.len() < 2 {
            return Err(ClientError::WrongNumberOfArguments("set".to_string()));
        }
        if params.len() == 3 || params.len() > 5 {
            return Err(ClientError::SyntaxError);
        }
        let key = params[0].to_owned();
        let value = params[1].to_owned();
        let expiration =  if params.len() == 4 {
            Expiration::try_from((params[2].to_owned(), params[3].to_owned())).map(|op| op.0).ok()
        } else {
            None
        };
        Ok(Self { key, value, expiration })
    }
}

#[derive(Debug, PartialEq)]
struct Expiration(SystemTime);

impl TryFrom<(String, String)> for Expiration {
    type Error = ClientError;

    fn try_from((option, value): (String, String)) -> Result<Self, Self::Error> {
        let to_add = value.parse::<u64>().map_err(|_| ClientError::IntegerError)?;

        match option {
            opt if opt == "ex" => {
                let desired = SystemTime::now().checked_add(Duration::from_secs(to_add)).ok_or(ClientError::IntegerError)?;
                Ok(Expiration(desired))
            }
            opt if opt == "px" => {
                let desired = SystemTime::now().checked_add(Duration::from_millis(to_add)).ok_or(ClientError::IntegerError)?;
                Ok(Expiration(desired))
            }
            opt if opt == "exat" => {
                let desired = UNIX_EPOCH.checked_add(Duration::from_secs(to_add)).ok_or(ClientError::IntegerError)?;
                Ok(Expiration(desired))
            }
            opt if opt == "pxat" => {
                let desired = UNIX_EPOCH.checked_add(Duration::from_millis(to_add)).ok_or(ClientError::IntegerError)?;
                Ok(Expiration(desired))
            }
            _ => Err(ClientError::SyntaxError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiration_no_number() {
        assert_eq!(ClientError::IntegerError, Expiration::try_from(("".to_string(), "hola".to_string())).unwrap_err());
    }

    #[test]
    fn expiration_out_of_range_number() {
        assert_eq!(ClientError::IntegerError, Expiration::try_from(("".to_string(), "1000000000000000000000".to_string())).unwrap_err());
    }

    #[test]
    fn expiration_wrong_command() {
        assert_eq!(ClientError::SyntaxError, Expiration::try_from(("".to_string(), "100".to_string())).unwrap_err());
    }

    #[test]
    fn expiration_exat_ok() {
        let exp = Expiration::try_from(("exat".to_string(), "1".to_string())).unwrap();
        assert_eq!(Expiration(UNIX_EPOCH.checked_add(Duration::from_secs(1)).unwrap()), exp);
    }

    #[test]
    fn expiration_pxat_ok() {
        let exp = Expiration::try_from(("pxat".to_string(), "1".to_string())).unwrap();
        assert_eq!(Expiration(UNIX_EPOCH.checked_add(Duration::from_millis(1)).unwrap()), exp);
    }
}
