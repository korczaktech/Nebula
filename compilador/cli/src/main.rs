//! Interface de linha de comando da Nebula.

#![forbid(unsafe_code)]

use nebula_fonte::ArquivoFonte;
use nebula_ir::baixar;
use nebula_semantica::{ModulosDisponiveis, analisar_semantica};
use nebula_sintaxe::analisar;
use std::path::{Path, PathBuf};
use std::process::Command;

#[rustfmt::skip]
fn main() {
    let mut argumentos = std::env::args().skip(1);
    match argumentos.next().as_deref() {
        Some("versao" | "--version" | "-V") => println!(
            "Nebula {} — compilador nativo inicial",
            env!("CARGO_PKG_VERSION")
        ),
        Some("ajuda" | "--help" | "-h") | None => ajuda(),
        Some("compilar") => {
            let Some(caminho) = argumentos.next() else { eprintln!("Uso: nebula compilar <arquivo.nbx> [-o <saida>]"); std::process::exit(2); };
            let mut saida = PathBuf::from(&caminho).with_extension("out");
            while let Some(arg) = argumentos.next() {
                if arg == "-o" {
                    let Some(valor) = argumentos.next() else { eprintln!("Erro: -o exige um caminho."); std::process::exit(2); };
                    saida = PathBuf::from(valor);
                } else {
                    eprintln!("Argumento desconhecido: {arg}");
                    std::process::exit(2);
                }
            }
            if let Err(erro) = compilar_arquivo(Path::new(&caminho), &saida) {
                eprintln!("Erro de compilação: {erro}");
                std::process::exit(1);
            }
            println!("Executável gerado: {}", saida.display());
        }
        Some("executar" | "testar") => {
            let Some(caminho) = argumentos.next() else { eprintln!("Uso: nebula executar|testar <arquivo.nbx>"); std::process::exit(2); };
            if argumentos.next().is_some() { eprintln!("Erro: executar aceita somente um arquivo."); std::process::exit(2); }
            let temporario = std::env::temp_dir().join(format!("nebula-{}{}", std::process::id(), if cfg!(windows) { ".exe" } else { "" }));
            if let Err(erro) = compilar_arquivo(Path::new(&caminho), &temporario) { eprintln!("Erro de compilação: {erro}"); std::process::exit(1); }
            let status = Command::new(&temporario).status();
            let _ = std::fs::remove_file(&temporario);
            match status { Ok(s) if s.success() => {}, Ok(s) => std::process::exit(s.code().unwrap_or(1)), Err(e) => { eprintln!("Não foi possível executar o programa: {e}"); std::process::exit(1); } }
        }
        Some("ir") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula ir <arquivo.nbx>");
                std::process::exit(2);
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: o comando 'ir' aceita somente um arquivo.");
                std::process::exit(2);
            }
            mostrar_ir(Path::new(&caminho));
        }
        Some("analisar") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula analisar <arquivo.nbx>");
                std::process::exit(2);
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: o comando 'analisar' aceita somente um arquivo.");
                std::process::exit(2);
            }
            analisar_arquivo(Path::new(&caminho));
        }
        Some("verificar" | "check") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula verificar <arquivo.nbx>");
                std::process::exit(2);
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: o comando 'verificar' aceita somente um arquivo.");
                eprintln!("Uso: nebula verificar <arquivo.nbx>");
                std::process::exit(2);
            }
            verificar(Path::new(&caminho));
        }
        Some(comando) => {
            eprintln!("Comando desconhecido: {comando}");
            eprintln!("Execute 'nebula ajuda' para consultar os comandos disponíveis.");
            std::process::exit(2);
        }
    }
}

#[rustfmt::skip]
fn compilar_arquivo(caminho: &Path, saida: &Path) -> Result<(), String> {
    let bytes = std::fs::read(caminho)
        .map_err(|e| format!("não foi possível ler '{}': {e}", caminho.display()))?;
    let conteudo = String::from_utf8(bytes)
        .map_err(|e| format!("arquivo não contém UTF-8 válido no byte {}", e.utf8_error().valid_up_to()))?;
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let sintaxe = analisar(&fonte);
    if !sintaxe.diagnosticos.is_empty() {
        return Err(sintaxe.diagnosticos.iter()
            .map(|d| format!("{} — {}", d.codigo, d.mensagem))
            .collect::<Vec<_>>().join("\n"));
    }
    let ir = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
    if !ir.diagnosticos.is_empty() {
        return Err(ir.diagnosticos.join("\n"));
    }
    let binario = nebula_backend::gerar_executavel(&ir.programa.expect("IR validada"))?;
    std::fs::write(saida, binario)
        .map_err(|e| format!("não foi possível gravar executável '{}': {e}", saida.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissoes = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(saida, permissoes)
            .map_err(|e| format!("não foi possível marcar o executável: {e}"))?;
    }
    Ok(())
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
    println!("  analisar <arquivo>    Verifica nomes, escopos e tipos estáticos");
    println!("  ir <arquivo>          Gera e exibe a representação intermediária");
    println!("  compilar <arquivo>    Gera executável ELF nativo Linux x86-64");
    println!("  executar <arquivo>    Compila e executa um programa nativo");
    println!("  testar <arquivo>      Compila e executa como teste de fumaça");
    println!();
    println!("A verificação sintática não realiza análise semântica nem gera executáveis.");
    println!("O backend gera diretamente executáveis ELF x86-64 em Rust, sem compilador externo.");\n    println!("O alvo inicial não inclui ainda a biblioteca padrão/runtime, strings ou coleções.");
}

fn mostrar_ir(caminho: &Path) {
    let bytes = std::fs::read(caminho).unwrap_or_else(|erro| {
        eprintln!(
            "Erro N0001: não foi possível ler '{}': {erro}",
            caminho.display()
        );
        std::process::exit(1);
    });
    let conteudo = String::from_utf8(bytes).unwrap_or_else(|erro| {
        eprintln!(
            "Erro N1000: arquivo '{}' não contém UTF-8 válido; sequência inválida a partir do byte {}.",
            caminho.display(),
            erro.utf8_error().valid_up_to()
        );
        std::process::exit(1);
    });
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let sintaxe = analisar(&fonte);
    if !sintaxe.diagnosticos.is_empty() {
        for diagnostico in &sintaxe.diagnosticos {
            eprintln!("{} — {}", diagnostico.codigo, diagnostico.mensagem);
        }
        std::process::exit(1);
    }
    let resultado = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
    if !resultado.diagnosticos.is_empty() {
        for diagnostico in &resultado.diagnosticos {
            eprintln!("{diagnostico}");
        }
        std::process::exit(1);
    }
    println!("{:#?}", resultado.programa.expect("baixamento validado"));
}

fn analisar_arquivo(caminho: &Path) {
    let bytes = std::fs::read(caminho).unwrap_or_else(|erro| {
        eprintln!(
            "Erro N0001: não foi possível ler '{}': {erro}",
            caminho.display()
        );
        std::process::exit(1);
    });
    let conteudo = String::from_utf8(bytes).unwrap_or_else(|erro| {
        eprintln!("Erro N1000: arquivo '{}' não contém UTF-8 válido; sequência inválida a partir do byte {}.", caminho.display(), erro.utf8_error().valid_up_to());
        std::process::exit(1);
    });
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let sintaxe = analisar(&fonte);
    if !sintaxe.diagnosticos.is_empty() {
        for diagnostico in &sintaxe.diagnosticos {
            let local = diagnostico
                .intervalo
                .and_then(|i| fonte.linha_coluna(i.inicio));
            if let Some((linha, coluna)) = local {
                eprintln!(
                    "{} {}:{}:{} — {}",
                    diagnostico.codigo,
                    caminho.display(),
                    linha,
                    coluna,
                    diagnostico.mensagem
                );
            } else {
                eprintln!("{} — {}", diagnostico.codigo, diagnostico.mensagem);
            }
        }
        std::process::exit(1);
    }
    let resultado = analisar_semantica(&sintaxe.programa);
    for diagnostico in &resultado.diagnosticos {
        eprintln!("{} — {}", diagnostico.codigo, diagnostico.mensagem);
    }
    if resultado.diagnosticos.is_empty() {
        println!("Arquivo válido semanticamente: {}", caminho.display());
    } else {
        std::process::exit(1);
    }
}

fn verificar(caminho: &Path) {
    let bytes = match std::fs::read(caminho) {
        Ok(bytes) => bytes,
        Err(erro) => {
            eprintln!(
                "Erro N0001: não foi possível ler '{}': {erro}",
                caminho.display()
            );
            std::process::exit(1);
        }
    };
    let conteudo = match String::from_utf8(bytes) {
        Ok(conteudo) => conteudo,
        Err(erro) => {
            eprintln!(
                "Erro N1000: arquivo '{}' não contém UTF-8 válido; sequência inválida a partir do byte {}.",
                caminho.display(),
                erro.utf8_error().valid_up_to()
            );
            std::process::exit(1);
        }
    };
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let resultado = analisar(&fonte);
    for diagnostico in &resultado.diagnosticos {
        let local = diagnostico
            .intervalo
            .and_then(|i| fonte.linha_coluna(i.inicio));
        if let Some((linha, coluna)) = local {
            eprintln!(
                "{} {}:{}:{} — {}",
                diagnostico.codigo,
                caminho.display(),
                linha,
                coluna,
                diagnostico.mensagem
            );
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
        assert!(
            analisar(&ArquivoFonte::novo(&caminho, texto))
                .diagnosticos
                .is_empty()
        );
        let _ = std::fs::remove_file(caminho);
    }
}
