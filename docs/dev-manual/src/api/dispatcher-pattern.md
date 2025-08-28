# The Dispatcher Pattern

The Alpaca API has a large number of endpoints, each corresponding to a specific property or method of a device. A naive implementation would involve writing a separate handler function for every single endpoint, leading to a lot of boilerplate and duplicated code.

To avoid this, RustyScope uses a **dispatcher pattern**. This pattern centralizes the handling of all Alpaca requests into a single, generic dispatcher function. This function is responsible for parsing the request, identifying the target action, and then dispatching it to the appropriate handler logic.

## The `handle_alpaca_request` Function

The core of this pattern is the `handle_alpaca_request` function in `src/mount/alpacaserver.rs`. This function is registered as the handler for all Alpaca-related URLs with the web server.

When a request comes in, this function performs the following steps:

1.  **Parse the URL:** It parses the URL to extract the key components of the request: the device type (e.g., `telescope`), the device number, and the action being requested (e.g., `rightascension`).

2.  **Determine the HTTP Method:** It checks if the request is an HTTP GET (to read a value) or an HTTP PUT (to set a value or execute an action).

3.  **Match the Action:** It uses a large `match` statement to map the requested action string to a specific variant of an internal `AlpacaCommand` enum. This provides a type-safe way to represent all possible Alpaca actions.

    ```rust
    // Simplified match statement
    let command = match action {
        "rightascension" => AlpacaCommand::RightAscension,
        "declination" => AlpacaCommand::Declination,
        "slewtoasync" => AlpacaCommand::SlewToAsync,
        // ... and so on for all other actions
    };
    ```

4.  **Dispatch to a Handler:** Based on the `AlpacaCommand` variant and the HTTP method, it then calls a more specific handler function. For example, all GET requests for simple properties might be handled by a `handle_get_property` function, while all PUT requests that execute a command might be handled by a `handle_put_command` function.

## Benefits of the Dispatcher Pattern

This pattern has several key advantages:

*   **Reduced Boilerplate:** It eliminates the need to write a separate top-level handler function for every single Alpaca endpoint. The `match` statement provides a concise way to map URL actions to internal commands.
*   **Centralized Logic:** All the common logic for parsing requests, handling errors, and serializing responses is centralized in one place. This makes the code easier to maintain and debug.
*   **Type Safety:** By using the `AlpacaCommand` enum, the dispatcher ensures that only valid Alpaca actions can be processed. This prevents errors that could arise from typos or incorrect action strings.
*   **Flexibility:** It is easy to add new Alpaca endpoints by simply adding a new variant to the `AlpacaCommand` enum and a new arm to the `match` statement.

By using this dispatcher pattern, the Alpaca API implementation in RustyScope is both robust and maintainable, even with the large number of endpoints required by the Alpaca standard.
