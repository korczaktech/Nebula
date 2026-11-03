//! Executável independente para formatar arquivos Nebula.

#[path = "../formatador.rs"]
mod formatador;

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(input) = args.next() else {
        ajuda();
        std::process::exit(2);
    };
    if input == "--help" || input == "-h" {
        ajuda();
        return;
    }
    let check = match args.next().as_deref() {
        None => false,
        Some("--check") => true,
        Some(other) => {
            eprintln!("Opção desconhecida: {other}");
            std::process::exit(2);
        }
    };
    if args.next().is_some() {
        ajuda();
        std::process::exit(2);
    }
    let path = PathBuf::from(input);
    match formatador::format_file(&path, check) {
        Ok(changed) if check && changed => {
            eprintln!("O arquivo precisa ser formatado: {}", path.display());
            std::process::exit(1);
        }
        Ok(true) => println!("Arquivo formatado: {}", path.display()),
        Ok(false) => println!("Arquivo já está formatado: {}", path.display()),
        Err(error) => {
            eprintln!("Erro ao formatar: {error}");
            std::process::exit(1);
        }
    }
}

fn ajuda() {
    println!("Uso: nebula-formatar <arquivo.nbx> [--check]");
    println!("Normaliza espaços finais sem reescrever tokens.");
}
