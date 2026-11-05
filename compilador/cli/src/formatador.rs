//! Formatador conservador da linguagem Nebula.
//!
//! Preserva tokens e estrutura de linhas, normalizando apenas espaços finais
//! e a quebra de linha final. A sintaxe é validada antes da gravação.

use nebula_fonte::ArquivoFonte;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static PROXIMO_TEMPORARIO: AtomicU64 = AtomicU64::new(1);

/// Formata um arquivo. Em modo de verificação, nunca grava no disco.
pub fn format_file(path: &Path, check: bool) -> Result<bool, String> {
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
    let mut linhas = source
        .lines()
        .map(|linha| linha.trim_end().to_string())
        .collect::<Vec<_>>();

    while linhas.last().is_some_and(|linha| linha.is_empty()) {
        linhas.pop();
    }

    if linhas.is_empty() {
        String::new()
    } else {
        format!("{}\n", linhas.join("\n"))
    }
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|e| format!("não foi possível inspecionar '{}': {e}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(
            "o formatador só substitui arquivos regulares, nunca links simbólicos".to_string(),
        );
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| "caminho sem nome de arquivo".to_string())?;

    let (temporario, mut arquivo) = loop {
        let id = PROXIMO_TEMPORARIO
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |atual| {
                atual.checked_add(1)
            })
            .map_err(|_| "contador de arquivos temporários esgotado".to_string())?;
        let candidato = parent.join(format!(
            ".{}.{}.{}.nebula-format-tmp",
            name.to_string_lossy(),
            std::process::id(),
            id
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidato)
        {
            Ok(arquivo) => break (candidato, arquivo),
            Err(erro) if erro.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(erro) => {
                return Err(format!("não foi possível criar arquivo temporário: {erro}"));
            }
        }
    };

    let resultado = (|| {
        arquivo
            .set_permissions(metadata.permissions())
            .map_err(|e| format!("não foi possível preservar permissões do arquivo: {e}"))?;
        arquivo
            .write_all(bytes)
            .map_err(|e| format!("não foi possível gravar arquivo temporário: {e}"))?;
        arquivo
            .sync_all()
            .map_err(|e| format!("não foi possível sincronizar arquivo temporário: {e}"))?;
        drop(arquivo);
        let atual = std::fs::symlink_metadata(path)
            .map_err(|e| format!("arquivo de destino deixou de estar disponível: {e}"))?;
        if atual.file_type().is_symlink() || !atual.is_file() {
            return Err("o destino foi substituído por um objeto não regular".to_string());
        }
        std::fs::rename(&temporario, path)
            .map_err(|e| format!("não foi possível substituir o arquivo: {e}"))?;
        Ok(())
    })();
    if resultado.is_err() {
        let _ = std::fs::remove_file(&temporario);
    }
    resultado
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
    fn preserva_linhas_e_conteudo_das_strings() {
        let source = "funcao principal(): vazio\n    exibir(\"a  b\")  \nfim\n";
        let formatted = normalize_whitespace(source);
        assert!(formatted.contains("\"a  b\""));
        assert_eq!(formatted.matches('\\n').count(), 3);
        assert!(formatted.contains("    exibir"));
    }
    #[test]
    fn preserva_indentacao_canonica_em_blocos() {
        let source = "funcao principal(): vazio\n    exibir(1)\n    quando verdadeiro\n        exibir(2)\n    fim\nfim\n";
        let formatted = normalize_whitespace(source);
        assert_eq!(formatted, source);
        assert_eq!(normalize_whitespace(&formatted), formatted);
    }
    #[test]
    fn preserva_chaves_em_strings_e_comentarios() {
        let source = "funcao principal(): vazio\n    exibir(\"{ }\") // }\n    /* {\n     } */\n    exibir('}')\nfim\n";
        let formatted = normalize_whitespace(source);
        assert_eq!(formatted, source);
    }
    #[test]
    fn check_nao_modifica_arquivo() {
        let path =
            std::env::temp_dir().join(format!("nebula-format-check-{}.nbx", std::process::id()));
        let original = "variavel x = 1   ";
        std::fs::write(&path, original).unwrap();
        assert!(format_file(&path, true).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn grava_substituicao_atomica_e_preserva_conteudo_formatado() {
        let path =
            std::env::temp_dir().join(format!("nebula-format-write-{}.nbx", std::process::id()));
        std::fs::write(&path, "variavel x = 1   ").unwrap();
        assert!(format_file(&path, false).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "variavel x = 1\n");
        assert!(
            !std::fs::read_dir(path.parent().unwrap())
                .unwrap()
                .filter_map(Result::ok)
                .any(|entrada| entrada
                    .file_name()
                    .to_string_lossy()
                    .contains("nebula-format-tmp"))
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejeita_sintaxe_invalida_sem_escrever() {
        let path =
            std::env::temp_dir().join(format!("nebula-format-invalid-{}.nbx", std::process::id()));
        std::fs::write(&path, "variavel =").unwrap();
        assert!(format_file(&path, false).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "variavel =");
        let _ = std::fs::remove_file(path);
    }
}
