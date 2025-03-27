use crate::resp::types::{BULK_STRING, CR, ERROR, INTEGER, LF, NULL, SIMPLE_STRING};

#[derive(Debug, PartialEq)]
pub enum Response {
    Null,
    SimpleString(String),
    BulkString(String),
    Integer(String),
    SimpleError(String),
}

impl Response {
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = vec![];
        match self {
            Response::Null => {
                bytes.push(NULL);
                bytes.push(CR);
                bytes.push(LF);
            }
            Response::SimpleString(s) => {
                bytes.push(SIMPLE_STRING);
                bytes.extend_from_slice(s.as_bytes());
                bytes.push(CR);
                bytes.push(LF);
            }
            Response::Integer(s) => {
                bytes.push(INTEGER);
                bytes.extend_from_slice(s.as_bytes());
                bytes.push(CR);
                bytes.push(LF);
            }
            Response::SimpleError(s) => {
                bytes.push(ERROR);
                bytes.extend_from_slice(s.as_bytes());
                bytes.push(CR);
                bytes.push(LF);
            }
            Response::BulkString(s) => {
                bytes.push(BULK_STRING);
                bytes.extend_from_slice(s.len().to_string().as_bytes());
                bytes.push(CR);
                bytes.push(LF);
                bytes.extend_from_slice(s.as_bytes());
                bytes.push(CR);
                bytes.push(LF);
            }
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::Response;

    #[test]
    fn serialize_null() {
        let reply = Response::Null;
        assert_eq!(reply.serialize(), b"_\r\n");
    }

    #[test]
    fn serialize_simple_string() {
        let reply = Response::SimpleString("".to_string());
        assert_eq!(reply.serialize(), b"+\r\n");

        let reply = Response::SimpleString("OK".to_string());
        assert_eq!(reply.serialize(), b"+OK\r\n");

        let reply = Response::SimpleString("Hello World".to_string());
        assert_eq!(reply.serialize(), b"+Hello World\r\n");

        let reply = Response::SimpleString("こんにちは".to_string());
        assert_eq!(reply.serialize(), "+こんにちは\r\n".as_bytes());
    }

    #[test]
    fn serialize_integer() {
        let reply = Response::Integer("0".to_string());
        assert_eq!(reply.serialize(), b":0\r\n");

        let reply = Response::Integer("42".to_string());
        assert_eq!(reply.serialize(), b":42\r\n");

        let reply = Response::Integer("-1".to_string());
        assert_eq!(reply.serialize(), b":-1\r\n");
    }

    #[test]
    fn serialize_simple_error() {
        let reply = Response::SimpleError("Error".to_string());
        assert_eq!(reply.serialize(), b"-Error\r\n");

        let reply = Response::SimpleError("ERR unknown command".to_string());
        assert_eq!(reply.serialize(), b"-ERR unknown command\r\n");
    }

    #[test]
    fn serialize_bulk_string() {
        let reply = Response::BulkString("".to_string());
        assert_eq!(reply.serialize(), b"$0\r\n\r\n");

        let reply = Response::BulkString("hello world".to_string());
        assert_eq!(reply.serialize(), b"$11\r\nhello world\r\n");

        let reply = Response::BulkString("💸".to_string());
        assert_eq!(reply.serialize(), b"$4\r\n\xF0\x9F\x92\xB8\r\n");
    }
}
