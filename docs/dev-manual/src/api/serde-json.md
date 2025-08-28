# Type-Safe JSON with Serde

JSON (JavaScript Object Notation) is the data format used for all communication in the Alpaca standard. While JSON is flexible and human-readable, working with it in a statically-typed language like Rust can be cumbersome and error-prone if not handled correctly. This is where the `serde` library comes in.

`serde` is a powerful and widely-used framework for **ser**ializing and **de**serializing Rust data structures efficiently and generically. RustyScope leverages `serde` and its companion library, `serde_json`, to handle all JSON operations in a way that is both type-safe and boilerplate-free.

## The `AlpacaResponse` Structs

The Alpaca standard defines a specific set of JSON response formats for different types of requests. For example, a request for a numeric value should be returned in a JSON object like this:

```json
{
  "Value": 12.345,
  "ClientTransactionID": 1,
  "ServerTransactionID": 123,
  "ErrorNumber": 0,
  "ErrorMessage": ""
}
```

Instead of manually constructing this JSON string, RustyScope defines a set of Rust structs that mirror these formats.

```rust
// In src/mount/alpacaserver.rs

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AlpacaResponse<T> {
    pub value: T,
    #[serde(rename = "ClientTransactionID")]
    pub client_transaction_id: u32,
    #[serde(rename = "ServerTransactionID")]
    pub server_transaction_id: u32,
    pub error_number: i32,
    pub error_message: String,
}
```

This `AlpacaResponse<T>` struct is a generic representation of the standard Alpaca response. The `T` can be any type that `serde` knows how to serialize, such as `f64`, `i32`, `bool`, or `String`.

## The Power of `#[derive(Serialize)]`

The key to `serde`'s power is the `#[derive(Serialize)]` macro. By adding this attribute to a Rust struct, we are telling `serde` to automatically generate the code required to convert an instance of that struct into a JSON string.

The `#[serde(...)]` attributes provide fine-grained control over the serialization process. For example:

*   `#[serde(rename_all = "PascalCase")]` tells `serde` to convert the Rust-standard `snake_case` field names (like `error_message`) into the JSON-standard `PascalCase` field names (like `ErrorMessage`).
*   `#[serde(rename = "...")]` allows for custom renaming of specific fields, which is necessary for `ClientTransactionID` and `ServerTransactionID`.

## From Struct to JSON

With these structs in place, handling an Alpaca request becomes remarkably simple. The handler logic creates an instance of the appropriate response struct, and then `serde_json::to_string` is used to perform the serialization.

```rust
// Simplified handler logic
let response = AlpacaResponse {
    value: 12.345, // The value we want to return
    client_transaction_id: 1,
    server_transaction_id: server_tid,
    error_number: 0,
    error_message: "".to_string(),
};

// Serialize the struct to a JSON string
let json_body = serde_json::to_string(&response)?;

// Send the JSON string in the HTTP response
http_response.write_all(json_body.as_bytes())?;
```

This approach provides the best of both worlds. We get the strong type safety of Rust structs at compile time, ensuring that we can't accidentally create a malformed response. At the same time, `serde` handles all the tedious and error-prone work of generating the correct JSON format at runtime. This makes the Alpaca API implementation in RustyScope clean, robust, and easy to maintain.
