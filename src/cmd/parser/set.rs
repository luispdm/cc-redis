use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::cmd::{commands::SET, request::ClientError}; // TODO cyclic dependency?

#[derive(Debug, PartialEq)]
pub struct Set {
    pub key: String,
    pub value: String,
    pub expiration: Option<SystemTime>,
}

impl Set {
    pub fn parse(params: Vec<String>) -> Result<Self, ClientError> {
        if params.len() < 2 {
            return Err(ClientError::WrongNumberOfArguments(SET.to_string()));
        }
        if params.len() == 3 || params.len() > 4 {
            return Err(ClientError::SyntaxError);
        }
        let key = params[0].to_owned();
        let value = params[1].to_owned();
        let expiration = if params.len() == 4 {
            match Expiration::try_from((params[2].to_owned(), params[3].to_owned())) {
                Ok(exp) => Some(exp.0),
                Err(e) => return Err(e),
            }
        } else {
            None
        };
        Ok(Self {
            key,
            value,
            expiration,
        })
    }
}

#[derive(Debug, PartialEq)]
struct Expiration(SystemTime);

impl TryFrom<(String, String)> for Expiration {
    type Error = ClientError;

    fn try_from((option, value): (String, String)) -> Result<Self, Self::Error> {
        let to_add = value
            .parse::<u64>()
            .map_err(|_| ClientError::IntegerError)?;

        match option.to_lowercase().as_str() {
            "ex" => {
                let desired = SystemTime::now()
                    .checked_add(Duration::from_secs(to_add))
                    .ok_or(ClientError::IntegerError)?;
                Ok(Expiration(desired))
            }
            "px" => {
                let desired = SystemTime::now()
                    .checked_add(Duration::from_millis(to_add))
                    .ok_or(ClientError::IntegerError)?;
                Ok(Expiration(desired))
            }
            "exat" => {
                let desired = UNIX_EPOCH
                    .checked_add(Duration::from_secs(to_add))
                    .ok_or(ClientError::IntegerError)?;
                Ok(Expiration(desired))
            }
            "pxat" => {
                let desired = UNIX_EPOCH
                    .checked_add(Duration::from_millis(to_add))
                    .ok_or(ClientError::IntegerError)?;
                Ok(Expiration(desired))
            }
            _ => Err(ClientError::SyntaxError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_one_arg() {
        assert_eq!(
            ClientError::WrongNumberOfArguments(SET.to_string()),
            Set::parse(vec!["".to_string()]).unwrap_err()
        );
    }

    #[test]
    fn parse_three_args() {
        let params = vec!["".to_string(), "".to_string(), "".to_string()];
        assert_eq!(ClientError::SyntaxError, Set::parse(params).unwrap_err());
    }

    #[test]
    fn parse_five_args() {
        let params = vec![
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
        ];
        assert_eq!(ClientError::SyntaxError, Set::parse(params).unwrap_err());
    }

    #[test]
    fn parse_two_args() {
        let params = vec!["key".to_string(), "value".to_string()];
        assert_eq!(
            Set {
                key: "key".to_string(),
                value: "value".to_string(),
                expiration: None
            },
            Set::parse(params).unwrap()
        );
    }

    #[test]
    fn parse_four_args() {
        let params = vec![
            "key".to_string(),
            "value".to_string(),
            "exat".to_string(),
            "10".to_string(),
        ];
        assert_eq!(
            Set {
                key: "key".to_string(),
                value: "value".to_string(),
                expiration: Some(UNIX_EPOCH.checked_add(Duration::from_secs(10)).unwrap())
            },
            Set::parse(params).unwrap()
        );
    }

    #[test]
    fn parse_four_args_err() {
        let params = vec![
            "key".to_string(),
            "value".to_string(),
            "NOTVALID".to_string(),
            "10".to_string(),
        ];
        assert_eq!(ClientError::SyntaxError, Set::parse(params).unwrap_err());
    }

    #[test]
    fn expiration_no_number() {
        assert_eq!(
            ClientError::IntegerError,
            Expiration::try_from(("".to_string(), "hola".to_string())).unwrap_err()
        );
    }

    #[test]
    fn expiration_out_of_range_number() {
        assert_eq!(
            ClientError::IntegerError,
            Expiration::try_from(("".to_string(), "1000000000000000000000".to_string()))
                .unwrap_err()
        );
    }

    #[test]
    fn expiration_wrong_command() {
        assert_eq!(
            ClientError::SyntaxError,
            Expiration::try_from(("".to_string(), "100".to_string())).unwrap_err()
        );
    }

    #[test]
    fn expiration_ex_ok() {
        let before = SystemTime::now();
        let expiration = Expiration::try_from(("ex".to_string(), "1".to_string())).unwrap();
        let after = SystemTime::now();

        let min_expected = before.checked_add(Duration::from_secs(1)).unwrap();
        let max_expected = after.checked_add(Duration::from_secs(1)).unwrap();

        assert!(expiration.0 >= min_expected);
        assert!(expiration.0 <= max_expected);
    }

    // to test the error case of px, `System::now()` has to be mocked
    #[test]
    fn expiration_px_ok() {
        let before = SystemTime::now();
        let expiration = Expiration::try_from(("px".to_string(), "100".to_string())).unwrap();
        let after = SystemTime::now();

        let min_expected = before.checked_add(Duration::from_millis(100)).unwrap();
        let max_expected = after.checked_add(Duration::from_millis(100)).unwrap();

        assert!(expiration.0 >= min_expected);
        assert!(expiration.0 <= max_expected);
    }

    #[test]
    fn expiration_ex_out_of_range() {
        let err = Expiration::try_from(("ex".to_string(), "18446744073709551615".to_string()))
            .unwrap_err();
        assert_eq!(ClientError::IntegerError, err);
    }

    #[test]
    fn expiration_exat_ok() {
        let exp = Expiration::try_from(("exat".to_string(), "1".to_string())).unwrap();
        assert_eq!(
            Expiration(UNIX_EPOCH.checked_add(Duration::from_secs(1)).unwrap()),
            exp
        );
    }

    #[test]
    fn expiration_exat_out_of_range() {
        let err = Expiration::try_from(("exat".to_string(), "18446744073709551615".to_string()))
            .unwrap_err();
        assert_eq!(ClientError::IntegerError, err);
    }

    // to test the error case of pxat, `System::now()` has to be mocked
    #[test]
    fn expiration_pxat_ok() {
        let exp = Expiration::try_from(("pxat".to_string(), "1".to_string())).unwrap();
        assert_eq!(
            Expiration(UNIX_EPOCH.checked_add(Duration::from_millis(1)).unwrap()),
            exp
        );
    }
}
