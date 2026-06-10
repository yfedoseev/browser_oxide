use crate::js_runtime::state::{ConsoleLevel, ConsoleMessage, DomState};
use deno_core::op2;
use deno_core::OpState;

#[op2(fast)]
pub fn op_console_log(state: &mut OpState, #[string] msg: String) {
    let state = state.borrow_mut::<DomState>();
    state.console_output.push(ConsoleMessage {
        level: ConsoleLevel::Log,
        args: vec![msg],
    });
}

#[op2(fast)]
pub fn op_console_warn(state: &mut OpState, #[string] msg: String) {
    let state = state.borrow_mut::<DomState>();
    state.console_output.push(ConsoleMessage {
        level: ConsoleLevel::Warn,
        args: vec![msg],
    });
}

#[op2(fast)]
pub fn op_console_error(state: &mut OpState, #[string] msg: String) {
    let state = state.borrow_mut::<DomState>();
    state.console_output.push(ConsoleMessage {
        level: ConsoleLevel::Error,
        args: vec![msg],
    });
}

deno_core::extension!(
    console_extension,
    ops = [op_console_log, op_console_warn, op_console_error],
);
