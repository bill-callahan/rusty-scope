# Implementing the Alpaca Standard

ASCOM Alpaca is a modern, open standard for communication between astronomical devices and software. It is based on standard web technologies (HTTP, JSON) and provides a significant improvement over older, serial-port-based protocols. RustyScope implements the Alpaca API to ensure out-of-the-box compatibility with a wide range of popular astrophotography software, such as N.I.N.A., PHD2, and Stellarium.

## The Alpaca API

The Alpaca API is a RESTful API that uses standard HTTP methods (GET and PUT) to interact with astronomical devices. Each device (e.g., a telescope, a focuser, a camera) exposes a set of endpoints that correspond to the properties and methods defined in the ASCOM standard.

For example, to get the current right ascension of a telescope, a client would send an HTTP GET request to a URL like:

```
http://<device-ip>:<port>/api/v1/telescope/0/rightascension
```

The device would then respond with a JSON object containing the value and any associated error information.

## RustyScope's Implementation

RustyScope implements the Alpaca API for two types of devices:

*   **Telescope:** This is the primary device, representing the telescope mount itself. It exposes endpoints for getting and setting the mount's coordinates, slewing, tracking, parking, and other functions.
*   **Focuser:** This device represents the electronic focuser, allowing clients to get and set the focuser's position.

### The Alpaca Server Task

The Alpaca API is implemented in a dedicated FreeRTOS task that runs an instance of the `esp-idf-svc` web server. This task is responsible for:

*   **Handling HTTP Requests:** It listens for incoming HTTP requests on the standard Alpaca port (11111).
*   **Routing:** It parses the URL of each request to determine which device and which property or method is being requested.
*   **Dispatching:** It calls the appropriate handler function to process the request.
*   **Sending Responses:** It serializes the response data into a JSON object and sends it back to the client.

### Alpaca Discovery

In addition to the main API, RustyScope also implements the Alpaca discovery protocol. This allows clients to automatically discover Alpaca-enabled devices on the local network without requiring the user to manually enter IP addresses.

The discovery mechanism works as follows:

1.  A client sends a UDP broadcast message to port 32227 on the local network.
2.  The `alpaca_discovery` task in RustyScope listens for these broadcast messages.
3.  When it receives a discovery request, it responds with a JSON object containing the name and port number of the Alpaca API server.

This makes the initial setup process much simpler and more user-friendly.

By implementing both the core Alpaca API and the discovery protocol, RustyScope provides a seamless, plug-and-play experience for users of modern astrophotography software.
