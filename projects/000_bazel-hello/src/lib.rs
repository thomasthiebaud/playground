use thiserror::Error;
use rand::prelude::IndexedRandom;

#[derive(Error, Debug)]
pub enum GreetError {
    #[error("name cannot be empty")]
    EmptyName,
}

pub fn greet(name: &str) -> Result<String, GreetError> {
    if name.is_empty() {
        return Err(GreetError::EmptyName);
    }

    let greetings = ["Hello!", "Hi!", "Hey!", "Howdy!"];
    let greeting = greetings.choose(&mut rand::rng()).unwrap();

    Ok(format!("{}, {}!", greeting, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greet_returns_greeting() {
        let result = greet("Bazel").unwrap();
        assert!(result.contains("Bazel"));
    }

    #[test]
    fn greet_empty_name_errors() {
        assert!(matches!(greet(""), Err(GreetError::EmptyName)));
    }
}
