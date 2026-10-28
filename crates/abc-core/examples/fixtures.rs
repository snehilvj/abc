//! Makes the fixture repositories in the folder given and prints each as
//! `<name> <path>`, for `just fixtures` to add.

fn main() -> std::io::Result<()> {
    let root = std::env::args_os()
        .nth(1)
        .expect("usage: fixtures <folder>");
    for fixture in abc_core::fixtures::generate(std::path::Path::new(&root))? {
        println!("{} {}", fixture.name, fixture.path.display());
    }
    Ok(())
}
