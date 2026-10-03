fn command_spec() {
    #{ id: "rustcraft:where", name: "where", usage: "", help: "Inspect player position", capability: "player.read" }
}
fn command(args) { player_position() }
