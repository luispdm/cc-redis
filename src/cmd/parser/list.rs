use crate::cmd::{error::ClientError, types::LPUSH};

#[derive(Debug, PartialEq)]
pub struct List {
    pub key: String,
    pub values: Vec<String>,
}

impl List {
    pub fn parse(params: &[String]) -> Result<Self, ClientError> {
        if params.len() < 2 {
            return Err(ClientError::WrongNumberOfArguments(LPUSH.to_string()));
        }
        Ok(Self {
            key: params[0].to_owned(),
            values: params[1..].to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ok() {
        let params = &["key".to_string(), "a".to_string(), "b".to_string()];
        let l = List::parse(params).unwrap();
        assert_eq!(
            l,
            List {
                key: "key".to_string(),
                values: vec!["a".to_string(), "b".to_string()],
            }
        );
    }

    #[test]
    fn parse_too_few_args() {
        let params = &["key".to_string()];
        assert_eq!(
            List::parse(params).unwrap_err(),
            ClientError::WrongNumberOfArguments(LPUSH.to_string())
        );
    }
}
