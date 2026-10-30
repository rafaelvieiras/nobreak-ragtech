// O painel web é embutido no binário. Se o frontend ainda não foi compilado,
// cria a pasta vazia para o `cargo check` e os testes funcionarem.
fn main() {
    let dist = std::path::Path::new("../frontend/dist");
    if !dist.exists() {
        let _ = std::fs::create_dir_all(dist);
    }
    println!("cargo:rerun-if-changed=../frontend/dist");
}
