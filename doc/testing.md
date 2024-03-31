# Testing

Since the template uses Hexagonal Architecture, it is relatively easy to test both the business logic and routing
logic in isolation. This is achieved by providing fakes for the driven ports in the case of business logic, or mocking
the driving port in the case of HTTP routing. This template also provides support for writing integration tests against
a real database.

The examples in this document use hexagonal architecture terms and provide tests for the "player API" for a
hypothetical video game backend described in [the Architecture documentation](./architecture_layers.md). It is recommended
that you familiarize yourself with the examples provided there before reading through the testing examples here.

## Unit Testing Business Logic

Business logic is most easily tested by defining fakes for the driven ports the logic communicates with. We'll start with
a sample implementation of just the player detection fake, then use both that and a hypothetical fake of the player writer fake
to write a whole test for the `new_player()` function defined on the player driving port.



## Unit Testing API Routes

## Writing Integration Tests