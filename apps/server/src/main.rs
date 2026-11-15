use meldframe_server::Server;

fn main() {
    let server = Server::assemble();
    for module in server.registry().module_names() {
        println!("module {module}");
    }
    for tool in server.registry().agent_tools() {
        println!("tool {} ({})", tool.name, tool.tier.as_str());
    }
}
