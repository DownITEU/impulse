fn main() {
    embed_resource::compile("assets/impulse.rc", embed_resource::NONE).manifest_required().unwrap();
}
