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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err("O backend inicial gera executáveis somente para Linux x86-64.".into());
    }
    validar_caminhos_compilacao(caminho, saida)?;
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
    gravar_executavel_atomico(saida, &binario)
}

fn validar_caminhos_compilacao(entrada: &Path, saida: &Path) -> Result<(), String> {
    let entrada_real = std::fs::canonicalize(entrada).map_err(|erro| {
        format!(
            "não foi possível resolver o arquivo de entrada '{}': {erro}",
            entrada.display()
        )
    })?;
    let nome_saida = saida.file_name().ok_or_else(|| {
        format!("caminho de saída inválido: '{}'", saida.display())
    })?;
    let diretorio_saida = saida
        .parent()
        .filter(|diretorio| !diretorio.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let diretorio_real = std::fs::canonicalize(diretorio_saida).map_err(|erro| {
        format!(
            "não foi possível resolver o diretório de saída '{}': {erro}",
            diretorio_saida.display()
        )
    })?;
    let saida_real = if saida.exists() {
        std::fs::canonicalize(saida).map_err(|erro| {
            format!("não foi possível resolver a saída '{}': {erro}", saida.display())
        })?
    } else {
        diretorio_real.join(nome_saida)
    };
    if entrada_real == saida_real {
        return Err("O executável de saída não pode sobrescrever o arquivo-fonte.".into());
    }
    Ok(())
}

#[rustfmt::skip]
fn gravar_executavel_atomico(saida: &Path, binario: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let nome = saida.file_name().ok_or_else(|| {
        format!("caminho de saída inválido: '{}'", saida.display())
    })?;
    let temporario = saida.with_file_name(format!(
        ".{}.{}.nebula-tmp",
        nome.to_string_lossy(),
        std::process::id()
    ));

    let resultado = (|| {
        // Um processo anterior pode ter sido encerrado antes da publicação.
        let _ = std::fs::remove_file(&temporario);
        let mut arquivo = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporario)
            .map_err(|e| format!("não foi possível criar arquivo temporário '{}': {e}", temporario.display()))?;
        arquivo.write_all(binario)
            .map_err(|e| format!("não foi possível gravar executável temporário: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            arquivo.set_permissions(std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("não foi possível marcar o executável: {e}"))?;
        }
        arquivo.sync_all()
            .map_err(|e| format!("não foi possível sincronizar o executável: {e}"))?;
        drop(arquivo);
        std::fs::rename(&temporario, saida)
            .map_err(|e| format!("não foi possível publicar o executável '{}': {e}", saida.display()))?;
        Ok(())
    })();

    if resultado.is_err() {
        let _ = std::fs::remove_file(&temporario);
    }
    resultado
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
    println!("O backend gera diretamente executáveis ELF x86-64 em Rust, sem compilador externo.");
    println!("O alvo inicial ainda não inclui runtime, entrada/saída, strings ou coleções.");
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
    fn publica_executavel_atomico_e_substitui_saida_anterior() {
        let caminho = std::env::temp_dir().join(format!("nebula-atomic-{}", std::process::id()));
        let _ = std::fs::remove_file(&caminho);
        gravar_executavel_atomico(&caminho, b"primeira versao").unwrap();
        gravar_executavel_atomico(&caminho, b"segunda versao").unwrap();
        assert_eq!(std::fs::read(&caminho).unwrap(), b"segunda versao");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&caminho).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
        let _ = std::fs::remove_file(caminho);
    }

    #[test]
    fn falha_de_compilacao_preserva_executavel_anterior() {
        let origem = std::env::temp_dir().join(format!(
            "nebula-invalido-{}.nbx",
            std::process::id()
        ));
        let saida = std::env::temp_dir().join(format!(
            "nebula-preservado-{}",
            std::process::id()
        ));
        std::fs::write(&origem, "funcao principal(): vazio { exibir(42) }").unwrap();
        std::fs::write(&saida, b"executavel anterior").unwrap();
        assert!(compilar_arquivo(&origem, &saida).is_err());
        assert_eq!(std::fs::read(&saida).unwrap(), b"executavel anterior");
        let _ = std::fs::remove_file(origem);
        let _ = std::fs::remove_file(saida);
    }

    #[test]
    fn falha_de_publicacao_remove_temporario_e_preserva_destino() {
        let destino = std::env::temp_dir().join(format!(
            "nebula-destino-diretorio-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&destino);
        std::fs::create_dir(&destino).unwrap();
        let erro = gravar_executavel_atomico(&destino, b"binario").unwrap_err();
        assert!(erro.contains("não foi possível publicar"));
        assert!(destino.is_dir());
        let nome = destino.file_name().unwrap().to_string_lossy();
        let temporario = destino.with_file_name(format!(
            ".{nome}.{}.nebula-tmp",
            std::process::id()
        ));
        assert!(!temporario.exists());
        std::fs::remove_dir(destino).unwrap();
    }

    #[test]
    fn compilacao_nao_sobrescreve_o_arquivo_fonte() {
        let origem = std::env::temp_dir().join(format!(
            "nebula-fonte-protegido-{}.nbx",
            std::process::id()
        ));
        let conteudo = "funcao principal(): inteiro { devolver 42 }";
        std::fs::write(&origem, conteudo).unwrap();
        let erro = validar_caminhos_compilacao(&origem, &origem).unwrap_err();
        assert!(erro.contains("não pode sobrescrever"));
        assert_eq!(std::fs::read_to_string(&origem).unwrap(), conteudo);
        let _ = std::fs::remove_file(origem);
    }

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
