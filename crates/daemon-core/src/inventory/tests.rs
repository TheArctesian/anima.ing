use super::{PROBES, Probe, ToolName};

mod folders;
mod probes;
mod system;

fn probe(tool: ToolName) -> &'static Probe {
    PROBES
        .iter()
        .find(|probe| probe.tool == tool)
        .expect("every tool has a probe")
}
