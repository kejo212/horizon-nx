//! The game as the platform runs it, for an executable to start (docs/layers.md, "How a Horizon
//! crate plugs in"). The official executable (`horizon-official`, Horizon layer) registers its
//! hooks (`horizon_platform::hooks`) and then calls [`main`]; it never names an engine crate.
//!
//! An executable built on it:
//!
//! ```ignore
//! static GAME_IDENTITY: [u8; horizon_game::GAME_IDENTITY.len()] = horizon_game::GAME_IDENTITY;
//!
//! fn main() {
//!     horizon_game::prepare_main_stack();
//!     std::hint::black_box(&GAME_IDENTITY);
//!     my_crate::register();
//!     horizon_game::main();
//! }
//! ```
//!
//! The platform's own parts register here, in [`main`], as they leave the engine (layer plan M6).
#![forbid(unsafe_code)]

/// Identifies an executable as the game for launcher imports, without an API version or build id.
pub const GAME_IDENTITY: [u8; horizon_mod_package::game_identity::MARK.len()] =
    horizon_mod_package::game_identity::MARK;

/// Runs the game with the hooks registered so far, and returns when it ends. Register hooks
/// (`horizon_platform::hooks::register`) before calling it.
pub fn main() {
    // The Switch: the homebrew loader's main thread has a small stack, which creating the render
    // pipelines (naga's shader passes) overflowed. The game runs on a thread with room instead;
    // nothing on the Switch needs the first thread (winit's Horizon loop runs on any).
    #[cfg(target_os = "horizon")]
    {
        const STACK: usize = 64 << 20;
        let game = std::thread::Builder::new()
            .name("main".into())
            .stack_size(STACK)
            .spawn(run)
            .expect("start the game's thread");
        if game.join().is_err() {
            std::process::exit(101);
        }
        return;
    }
    #[cfg(not(target_os = "horizon"))]
    run();
}

fn run() {
    horizon_guest_stack::prepare_main();
    horizon_mod_host::exit_if_compile_child();
    let startup = horizon_client::entry::prepare();
    // The Switch has no holder process (horizon-client builds without runtime-process there).
    #[cfg(not(target_os = "horizon"))]
    std::hint::black_box(&horizon_mod_package::holder::CAPABILITY);
    // The fork's Switch build links Create into this binary; desktop builds and vanilla
    // Switch builds keep the empty list.
    #[cfg(feature = "create")]
    horizon_mod_host::main_with(
        vec![horizon_create::entry()],
        &[],
        horizon_create::BUILT,
        startup,
    );
    #[cfg(not(feature = "create"))]
    horizon_mod_host::main_with(Vec::new(), &[], &[], startup);
}

/// Establish the executable main thread stack before initialization.
pub use horizon_guest_stack::prepare_main as prepare_main_stack;
