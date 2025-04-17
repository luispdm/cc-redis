use crate::cmd::{error::ClientError, types::INCRBY};

#[derive(Debug, PartialEq)]
pub struct Integer {
    pub key: String,
    pub value: i64,
}

impl Integer {
    pub fn parse(params: &[String]) -> Result<Self, ClientError> {
        if params.len() != 2 {
            return Err(ClientError::WrongNumberOfArguments(INCRBY.to_string()));
        }

        let value = params[1]
            .parse::<i64>()
            .map_err(|_| ClientError::IntegerError)?;

        Ok(Self {
            key: params[0].to_owned(),
            value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ok() {
        let expected = Integer{
            key: "key".to_string(),
            value: 100,
        };
        let params = &["key".to_string(), "100".to_string()];
        let i = Integer::parse(params).unwrap();
        assert_eq!(i, expected);
    }

    #[test]
    fn parse_err() {
        let params = &["key".to_string(), "not_an_i64".to_string()];
        assert!(Integer::parse(params).is_err());
    }
}
