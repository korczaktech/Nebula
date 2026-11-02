//! CLI provisória da Nebula. O compilador funcional será desenvolvido nas próximas fases.

#![forbid(unsafe_code)]

fn main() {
    let mut argumentos = std::env::args().skip(1);
    match argumentos.next().as_deref() {
        Some("versao" | "--version" | "-V") => {
            println!(
                "Nebula {} — fundação em desenvolvimento",
                env!("CARGO_PKG_VERSION")
            );
        }
        Some("ajuda" | "--help" | "-h") | None => {
            println!("Nebula — linguagem de programação");
            println!();
            println!("Uso: nebula <comando>");
            println!();
            println!("Comandos disponíveis nesta etapa:");
            println!("  ajuda       Exibe esta mensagem");
            println!("  versao      Exibe a versão da ferramenta");
            println!();
            println!("A compilação e a execução de arquivos .nbx ainda não estão disponíveis.");
        }
        Some(comando) => {
            eprintln!("Comando desconhecido: {comando}");
            eprintln!("Execute 'nebula ajuda' para consultar os comandos disponíveis.");
            std::process::exit(2);
        }
    }
}
