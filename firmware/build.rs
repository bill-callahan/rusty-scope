// build.rs

use std::env;

fn main() {
    // --- Add your .env logic first ---

    // Tell Cargo that if the .env file changes, it should re-run this build script.
    println!("cargo:rerun-if-changed=.env");

    // Use dotenvy to load the .env file.
    // Using `ok()` is a bit safer than `expect()` if you want to allow builds
    // without an .env file (e.g., in a CI/CD pipeline). Let's stick with `expect`
    // for local development, as it's more explicit.
    if let Err(e) = dotenv::dotenv() {
        // Print a warning but don't fail the build if .env is missing.
        // This is a good compromise for flexibility.
        println!("cargo:warning=Could not load .env file: {}. Continuing without it.", e);
    }

    // Attempt to load the variables. If they exist (from .env or the system env),
    // pass them to the compiler. If not, the build will fail at the `env!` macro later,
    // which is a clear error message.
    if let Ok(ssid) = env::var("WIFI_SSID") {
        println!("cargo:rustc-env=WIFI_SSID={}", ssid);
    }
    if let Ok(pass) = env::var("WIFI_PASS") {
        println!("cargo:rustc-env=WIFI_PASS={}", pass);
    }


    // --- Now, call the critical embuild function ---
    // This must be called to configure the ESP-IDF C build system.
    embuild::espidf::sysenv::output();
}