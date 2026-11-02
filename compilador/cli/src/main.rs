//! Interface de linha de comando da Nebula.

#![forbid(unsafe_code)]
#![rustfmt::skip]

use nebula_fonte::ArquivoFonte;
use nebula_sintaxe::analisar;
use std::path::Path;

fn main() {
    let mut argumentos = std::env::args().skip(1);
    match argumentos.next().as_deref() {
        Some("versao" | "--version" | "-V") => println!("Nebula {} — frontend em desenvolvimento", env!("CARGO_PKG_VERSION")),
        Some("ajuda" | "--help" | "-h") | None => ajuda(),
        Some("verificar" | "check") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula verificar <arquivo.nbx>");
                std::process::exit(2);
            };
            verificar(Path::new(&caminho));
        }
        Some(comando) => {
            eprintln!("Comando desconhecido: {comando}");
            eprintln!("Execute 'nebula ajuda' para consultar os comandos disponíveis.");
            std::process::exit(2);
        }
    }
}
fn ajuda() {
    println!("Nebula — linguagem de programação");
    println!();
    println!("Uso: nebula <comando>");
    println!();
    println!("Comandos disponíveis:");
    println!("  ajuda                 Exibe esta mensagem");
    println!("  versao                Exibe a versão da ferramenta");
    println!("  verificar <arquivo>   Analisa léxica e sintaticamente um arquivo .nbx");
    println!();
    println!("A verificação não realiza análise semântica nem gera executáveis.");
}
fn verificar(caminho: &Path) {
    let conteudo = match std::fs::read_to_string(caminho) {
        Ok(c) => c,
        Err(erro) => {
            eprintln!("Erro N0001: não foi possível ler '{}': {erro}", caminho.display());
            std::process::exit(1);
        }
    };
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let resultado = analisar(&fonte);
    for diagnostico in &resultado.diagnosticos {
        let local = diagnostico.intervalo.and_then(|i| fonte.linha_coluna(i.inicio));
        if let Some((linha, coluna)) = local {
            eprintln!("{} {}:{}:{} — {}", diagnostico.codigo, caminho.display(), linha, coluna, diagnostico.mensagem);
        } else {
            eprintln!("{} — {}", diagnostico.codigo, diagnostico.mensagem);
        }
    }
    if resultado.diagnosticos.is_empty() {
        println!("Arquivo válido sintaticamente: {}", caminho.display());
    } else {
        std::process::exit(1);
    }
}
#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn verifica_arquivo_nebula_valido() {
        let caminho = std::env::temp_dir().join(format!("nebula-{}.nbx", std::process::id()));
        std::fs::write(&caminho, "variavel total = 1 + 2").expect("grava arquivo");
        let texto = std::fs::read_to_string(&caminho).expect("lê arquivo");
        assert!(analisar(&ArquivoFonte::novo(&caminho, texto)).diagnosticos.is_empty());
        let _ = std::fs::remove_file(caminho);
    }
}
