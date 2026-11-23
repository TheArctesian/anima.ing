//! `anima-model-launcher` (task L1-6c): the root-owned launcher of a model server.
//!
//! `anima-model-launcher --protocol` writes the protocol line. Else the arguments are
//! `<list> <signature> <cgroup.procs> <daemon pid> <failure file> <program> <arguments…>`: the
//! signed list of runtimes and its `.sig` file, then the arguments of the launch mode of the
//! daemon. The runtime program must be on the list (R-347).

#[cfg(target_os = "linux")]
fn main() {
    use std::path::Path;

    use model_launcher::code::Step;
    use model_launcher::launch::{self, Order, Signed};

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() == 1 && arguments[0] == model_launcher::PROTOCOL_ARGUMENT {
        println!("{}", model_launcher::protocol_line());
        return;
    }
    let refuse = || std::process::exit(Step::Exec.code());
    let Some(key) = model_launcher::updater_key() else {
        refuse()
    };
    let [list, signature, rest @ ..] = arguments.as_slice() else {
        refuse()
    };
    let Some(order) = Order::parse(rest) else {
        refuse()
    };
    let signed = Signed {
        list: Path::new(list),
        signature: Path::new(signature),
        key: &key,
    };
    launch::run(&order, Some(&signed));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    std::process::exit(model_launcher::code::Step::Exec.code());
}
