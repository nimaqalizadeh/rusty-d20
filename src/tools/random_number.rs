use serde_json::{Value, json};
use crate::message::Message;
use serde::{Serialize, Deserialize};

pub const NAME: &str = "random_number";

pub fn create_tool() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Generate a random integer between 'min' and 'max'.",
            "parameters": {
                "type": "object",
                "properties": {
                    "min": {
                        "type": "number", 
                        "description": "The lowest possible number that can be generated. For
                        example 0.",

                    },
                    "max": {
                        "type": "number", 
                        "description": "The largest possible number that can be generated. For
                        example 10.",
                    } 
                },
                "required": [
                    "min",
                    "max"
                ]
            }
        }
    })
}

pub fn run(arguments: String) -> Message {
    println!("Running random number tool");
    let args = match serde_json::from_str::<RandomNumberArgs>(&arguments) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("AI didn't passed in the arguments correctly: {arguments}: {error:?}");

            return Message::new_tool(format!("Error parsing the arguments: {error}"));
        }
    };
    let random_number = rand::random_range(args.min..=args.max);
    dbg!(random_number);
    Message::new_tool( random_number)

}

#[derive(Debug, Serialize, Deserialize)]
struct RandomNumberArgs {
    pub min: i32,
    pub max: i32
}
