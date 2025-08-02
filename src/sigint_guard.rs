use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

const extern "C" fn empty_handler(_signal: std::ffi::c_int) {}

/// Disables the effects of <kbd>Ctrl</kbd>+<kbd>C</kbd> on the current process.
pub struct SigintGuard {
    old_action: SigAction,
}

impl SigintGuard {
    pub fn new() -> Self {
        // SAFETY: async-signal-safe handler and only one thread here
        unsafe {
            let old_action = sigaction(
                Signal::SIGINT,
                &SigAction::new(
                    SigHandler::Handler(empty_handler),
                    SaFlags::SA_RESTART,
                    SigSet::empty(), // TODO: pass SIGINT or no?
                ),
            )
            .unwrap();

            Self { old_action }
        }
    }
}

impl Drop for SigintGuard {
    fn drop(&mut self) {
        // SAFETY: async-signal-safe handler and only one thread here
        unsafe {
            sigaction(Signal::SIGINT, &self.old_action).unwrap();
        }
    }
}
