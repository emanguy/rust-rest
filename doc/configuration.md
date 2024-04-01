# Configuration

This template is configured via environment variables, as that is typically how microservices are configured when deployed
with Docker. For both the ability to rename environment variables across the microservice and to track environment variable
usage within the application, environment variable names are defined as constants in the `app_env` module. Environment
variables used specifically in tests are defined in the `app_env::test` module. This can also be used as a way to document
environment variables.

Here's an example of how these environment variables might be defined:

```rust
// in app_env.rs

/// Defines whether the big red button should be pushed during app startup
pub const PRESS_BUTTON: &str = "PRESS_BUTTON";

// ...other environment variables

#[cfg(test)]
pub mod test {
    /// The state of the big red button in tests
    pub const BUTTON_STATE: &str = "BUTTON_STATE";
}
```

Once those constants are defined, you can use them with `env::var()` to read the configuration value:

```rust
// Somewhere else in the application

use std::env;

fn maybe_press_button() {
    let press_button = match env::var(app_env::PRESS_BUTTON) {
        Ok(state) => state,
        Err(_) => "false".to_owned(),
    };
    
    // ...do something with that config option
}
```