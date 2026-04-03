use serde_json::{Value, json};

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
