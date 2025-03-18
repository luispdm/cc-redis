use std::error::Error;

enum Command {
    BulkStrings,
    Array,
}

impl TryFrom<u8> for Command {
    type Error = String;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            b'$' => Ok(Command::BulkStrings),
            b'*' => Ok(Command::Array),
            _ => Err(format!("Unknown command {byte}")),
        }
    }
}

fn main() {
    print!("$ represents a... ");
    let _ = deserialize_msg("$");
    print!("* represents a... ");
    let _ = deserialize_msg("*");
    println!("A represents a... {:?}", deserialize_msg("A"));
}

fn deserialize_msg(msg: &str) -> Result<(), Box<dyn Error>> {
    let msg_bytes = msg.as_bytes();
    let incoming_cmd = msg_bytes[0].try_into()?;
    match incoming_cmd {
        Command::BulkStrings => {
            println!("string");
        }
        Command::Array => {
            println!("array");
        }
    }
    Ok(())
}
