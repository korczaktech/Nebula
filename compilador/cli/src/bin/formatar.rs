//! Formatador conservador da linguagem Nebula.
//!
//! Normaliza somente espaços em branco no fim das linhas e a quebra final.
//! O conteúdo léxico é preservado; o parser valida antes da gravação.

use nebula_fonte::ArquivoFonte;
use std::path::{Path, PathBuf};

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
    match format_file(&path, check) {
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

fn format_file(path: &Path, check: bool) -> Result<bool, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("não foi possível ler '{}': {e}", path.display()))?;
    let source = String::from_utf8(bytes).map_err(|e| {
        format!(
            "arquivo não contém UTF-8 válido no byte {}",
            e.utf8_error().valid_up_to()
        )
    })?;
    let parsed = nebula_sintaxe::analisar(&ArquivoFonte::novo(path, source.clone()));
    if !parsed.diagnosticos.is_empty() {
        return Err(parsed
            .diagnosticos
            .iter()
            .map(|d| format!("{} — {}", d.codigo, d.mensagem))
            .collect::<Vec<_>>()
            .join("\n"));
    }

    let formatted = normalize_whitespace(&source);
    if formatted == source {
        return Ok(false);
    }
    if check {
        return Ok(true);
    }
    atomic_write(path, formatted.as_bytes())?;
    Ok(true)
}

fn normalize_whitespace(source: &str) -> String {
    let mut lines = source.lines().map(str::trim_end).collect::<Vec<_>>();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|e| format!("não foi possível inspecionar '{}': {e}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err("o formatador não substitui links simbólicos".to_string());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| "caminho sem nome de arquivo".to_string())?;
    let temp = parent.join(format!(
        ".{}.{}.nebula-format-tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        std::fs::write(&temp, bytes)
            .map_err(|e| format!("não foi possível gravar temporário: {e}"))?;
        std::fs::rename(&temp, path)
            .map_err(|e| format!("não foi possível substituir o arquivo: {e}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizacao_e_deterministica_e_idempotente() {
        let first = normalize_whitespace("variavel x = 1   \n\n");
        assert_eq!(first, "variavel x = 1\n");
        assert_eq!(normalize_whitespace(&first), first);
    }

    #[test]
    fn preserva_bytes_lexicos_e_linhas_internas() {
        let source = "funcao principal(): vazio {\n  exibir(\"a  b\")  \n}\n";
        let formatted = normalize_whitespace(source);
        assert!(formatted.contains("\"a  b\""));
        assert!(formatted.contains("\n  exibir"));
        assert_eq!(formatted.matches('\n').count(), 3);
    }

    #[test]
    fn arquivo_vazio_permanece_vazio() {
        assert_eq!(normalize_whitespace(""), "");
        assert_eq!(normalize_whitespace("\n\n"), "");
    }

    #[test]
    fn modo_check_detecta_sem_modificar_arquivo() {
        let path =
            std::env::temp_dir().join(format!("nebula-format-check-{}.nbx", std::process::id()));
        let original = "variavel x = 1   ";
        std::fs::write(&path, original).unwrap();
        assert!(format_file(&path, true).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn formatacao_grava_arquivo_validado() {
        let path =
            std::env::temp_dir().join(format!("nebula-format-write-{}.nbx", std::process::id()));
        std::fs::write(&path, "variavel x = 1   ").unwrap();
        assert!(format_file(&path, false).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "variavel x = 1\n");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn nao_formata_arquivo_sintaticamente_invalido() {
        let path =
            std::env::temp_dir().join(format!("nebula-format-invalid-{}.nbx", std::process::id()));
        std::fs::write(&path, "variavel =").unwrap();
        assert!(format_file(&path, false).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "variavel =");
        let _ = std::fs::remove_file(path);
    }
}
