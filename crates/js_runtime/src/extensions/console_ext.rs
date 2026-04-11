use crate::state::{ConsoleLevel, ConsoleMessage, DomState};
use deno_core::op2;

#[op2(fast)]
pub fn op_console_log(#[state] state: &mut DomState, #[string] msg: String) {
    state.console_output.push(ConsoleMessage {
        level: ConsoleLevel::Log,
        args: vec![msg],
    });
}

#[op2(fast)]
pub fn op_console_warn(#[state] state: &mut DomState, #[string] msg: String) {
    state.console_output.push(ConsoleMessage {
        level: ConsoleLevel::Warn,
        args: vec![msg],
    });
}

#[op2(fast)]
pub fn op_console_error(#[state] state: &mut DomState, #[string] msg: String) {
    state.console_output.push(ConsoleMessage {
        level: ConsoleLevel::Error,
        args: vec![msg],
    });
}

deno_core::extension!(
    console_extension,
    ops = [op_console_log, op_console_warn, op_console_error],
);
