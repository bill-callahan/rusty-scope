# The Commander: An Actor-Based Approach

The `RustyScope` struct, located in `src/mount/rustyscope.rs`, is the central hub for the telescope's high-level logic. However, it doesn't execute commands directly. Instead, it acts as a state and configuration manager, delegating the actual execution to a dedicated "Commander" task. This design pattern is a form of the Actor Model, which is excellent for building robust, concurrent systems.

## The `RustyScope` Struct

The `RustyScope` struct holds:

*   A reference to the `SystemServices` for accessing hardware and system resources.
*   A shared `Arc<Mutex<>>` to the global `RustyScopeState`.
*   The hardware configuration for the mount's drivetrains (steps per revolution, gear ratios, etc.).
*   A handle to a FreeRTOS queue, which serves as the mailbox for the Commander task.

Its primary role is to provide a clean API for other parts of the system (like the Alpaca server) to interact with the mount. When a command like `park()` or `slew_to_target()` is called on a `RustyScope` instance, it doesn't block. It simply packages the command into a message and sends it to the Commander task's queue.

```rust
// In RustyScope
pub fn command_park(&self) -> Result<()> {
    info!("Park command issued.");
    self.send_command(Command::Park);
    Ok(())
}

fn send_command(&self, cmd: Command) {
   let result = unsafe {
        xQueueGenericSend(self.command_queue.0, &cmd as *const _ as *mut _, 0, 0)
    };
    // ... error handling ...
}
```

## The Commander Task

The `commander_task_entry` function implements the actor itself. It runs in an infinite loop, which is the core of its state machine. This loop does three main things:

1.  **Receive Messages:** It performs a non-blocking check on its command queue for any new incoming messages (commands).
2.  **Execute State Logic:** It executes the logic for its *current* state. For example, if its state is `WaitingForSlew`, it will check if the motion planner is still busy. If not, it will transition to a new state, like `Idle` or `Tracking`.
3.  **Update Telemetry:** It periodically updates the public `RustyScopeState` with the latest telemetry from the hardware, such as the current motor positions and GPS status.

### The State Machine

The Commander's logic is implemented as a state machine, defined by the `CommanderState` enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum CommanderState {
    Idle,
    WaitingForCoarseSlew,
    WaitingForFineSlew,
    WaitingForPark,
    WaitingForHome,
    // ... and others
}
```

This is crucial for managing long-running, multi-step operations. For example, a GoTo slew is not a single action. It involves:

1.  Stopping any existing tracking.
2.  Commanding the motion planner to move to the target coordinates (the "coarse" slew).
3.  Waiting in the `WaitingForCoarseSlew` state until the planner reports it is finished.
4.  Performing a second, smaller "fine-tuning" slew to correct for any initial error.
5.  Waiting in the `WaitingForFineSlew` state.
6.  Once complete, automatically starting tracking at the new position.
7.  Finally, transitioning back to the `Idle` state.

By using a state machine, the Commander task can manage this entire sequence without ever blocking. It can still receive and handle other commands (like a `StopMotion` command) while it is waiting for a slew to complete.

This actor-based, state-machine-driven design makes the system highly responsive and robust. It cleanly separates the command-issuing logic from the command-execution logic, which is essential for a real-time embedded system.