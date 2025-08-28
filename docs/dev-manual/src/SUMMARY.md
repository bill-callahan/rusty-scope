# Table of Contents

- **Introduction**
    - [Goals of the Project](./introduction/goals.md)
    - [Why Rust on ESP32?](./introduction/why-rust.md)
    - [Core Design Principles](./introduction/principles.md)

- **Chapter 1: High-Level Architecture**
    - [System Architecture Overview](./architecture/system-architecture.md)
    - [The `main` Function and System Bring-up](./architecture/systemservices.md)
    - [The Commander: An Actor-Based Approach](./architecture/rustyscope.md)

- **Chapter 2: The Motion Control System**
    - [Motion Control Stack Overview](./motion-control/stepper.md)
        - *This is your `stepper.md` file.*
    - [Deep Dive: I2S for Jitter-Free Stepping](./motion-control/i2s-deep-dive.md)
    - [Deep Dive: The Motion Planner's Algorithms](./motion-control/planner-algorithms.md)

- **Chapter 3: Pointing Model & Coordinate Systems**
    - [Modeling the Sky and the Mount](./coordinates/coordinates.md)
        - *This is your `coordinates.md` file.*
    - [The Timekeeping System](./coordinates/time.md)
        - *This is your `time.md` file.*

- **Chapter 4: I/O and System Services**
    - [GPS: The Location and Time Source](./io-services/gpscontroller.md)
        - *This is your `gpscontroller.md` file.*
    - [Persistence: The Storage Layer](./io-services/storage.md)
        - *This is your `storage.md` file.*
    - [The Display Controller and UI](./io-services/display.md)
        - *This is your `display.md` file.*

- **Chapter 5: The Alpaca API Layer**
    - [Implementing the Alpaca Standard](./api/AlpacaAPI.md)
        - *This is your `AlpacaAPI.md` file.*
    - [The Dispatcher Pattern](./api/dispatcher-pattern.md)
    - [Type-Safe JSON with Serde](./api/serde-json.md)

- **Chapter 6: Development and Contribution**
    - [Setting up the Development Environment](./development/setup.md)
    - [Build, Flash, and Debug Workflow](./development/build-and-debug.md)
    - [Contribution Guide](./development/contributing.md)

- **Conclusion**
    - [Project Retrospective & Future Work](./conclusion/retrospective.md)