# Rust REST template Documentation

This set of documents aims to give you an overview of how this REST microservice template is structured and how to customize
and use it for your own needs.

This microservice template is built using [Hexagonal Architecture](https://medium.com/ssense-tech/hexagonal-architecture-there-are-always-two-sides-to-every-story-bc0780ed7d9c) principles. **Please read that document before reading
anything else**!!

This template has a MSRV of Rust 1.75, as it requires official async-function-in-trait support.

## Areas of interest

1. [Architecture layers](./architecture_layers.md)
2. [Logging](./logging.md)
3. [Testing](./testing.md)
4. [Database connectivity](./database.md)
5. [Documenting the API](./api_documentation.md)
6. [Configuration](./configuration.md)