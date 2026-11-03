//! Formatador conservador da linguagem Nebula.
//!
//! Preserva tokens e estrutura de linhas, normalizando apenas espaços finais
//! e a quebra de linha final. A sintaxe é validada antes da gravação.

use nebula_fonte::ArquivoFonte;
use std::path::Path;

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
    let mut depth = 0_usize;
    let mut dentro_comentario = false;
    let mut linhas = Vec::new();

    for original in source.lines() {
        let conteudo = original.trim();
        if conteudo.is_empty() {
            linhas.push(String::new());
            continue;
        }

        let (aberturas, fechamentos, fechamentos_iniciais) =
            contar_chaves(conteudo, &mut dentro_comentario);
        let nivel = depth.saturating_sub(fechamentos_iniciais);
        let recuo = " ".repeat(nivel.saturating_mul(4));
        linhas.push(format!("{recuo}{conteudo}"));
        depth = depth
            .saturating_sub(fechamentos)
            .saturating_add(aberturas);
    }

    while linhas.last().is_some_and(|linha| linha.is_empty()) {
        linhas.pop();
    }
    if linhas.is_empty() {
        String::new()
    } else {
        format!("{}\n", linhas.join("\n"))
    }
}

/// Conta delimitadores de bloco ignorando strings e comentários.
fn contar_chaves(linha: &str, dentro_comentario: &mut bool) -> (usize, usize, usize) {
    let caracteres = linha.chars().collect::<Vec<_>>();
    let mut indice = 0;
    let mut aberturas = 0_usize;
    let mut fechamentos = 0_usize;
    let mut fechamentos_iniciais = 0_usize;
    let mut inicio_logico = true;

    while indice < caracteres.len() {
        if *dentro_comentario {
            if caracteres[indice] == '*' && caracteres.get(indice + 1) == Some(&'/') {
                *dentro_comentario = false;
                indice += 2;
            } else {
                indice += 1;
            }
            continue;
        }

        let atual = caracteres[indice];
        let proximo = caracteres.get(indice + 1).copied();
        if atual.is_whitespace() {
            indice += 1;
            continue;
        }
        if atual == '/' && proximo == Some('*') {
            *dentro_comentario = true;
            indice += 2;
            continue;
        }
        if atual == '/' && proximo == Some('/') {
            break;
        }
        if atual == '}' {
            fechamentos += 1;
            if inicio_logico {
                fechamentos_iniciais += 1;
            }
            indice += 1;
            continue;
        }

        inicio_logico = false;
        if atual == '{' {
            aberturas += 1;
            indice += 1;
            continue;
        }

        if atual == '"' || atual == '\'' {
            let delimitador = atual;
            indice += 1;
            while indice < caracteres.len() {
                if caracteres[indice] == '\\' {
                    indice = (indice + 2).min(caracteres.len());
                    continue;
                }
                if caracteres[indice] == delimitador {
                    indice += 1;
                    break;
                }
                indice += 1;
            }
            continue;
        }
        indice += 1;
    }

    (aberturas, fechamentos, fechamentos_iniciais)
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
    fn preserva_linhas_e_conteudo_das_strings() {
        let source = "funcao principal(): vazio {\n  exibir(\"a  b\")  \n}\n";
        let formatted = normalize_whitespace(source);
        assert!(formatted.contains("\"a  b\""));
        assert_eq!(formatted.matches('\n').count(), 3);
    }

    #[test]
    fn aplica_indentacao_canonica_em_blocos() {
        let source = "funcao principal(): vazio {\nexibir(1)\nquando verdadeiro {\nexibir(2)\n}\n}\n";
        let formatted = normalize_whitespace(source);
        assert_eq!(
            formatted,
            "funcao principal(): vazio {\n    exibir(1)\n    quando verdadeiro {\n        exibir(2)\n    }\n}\n"
        );
        assert_eq!(normalize_whitespace(&formatted), formatted);
    }

    #[test]
    fn ignora_chaves_em_strings_e_comentarios() {
        let source = "funcao principal(): vazio {\nexibir(\"{ }\") // }\n/* {\n } */\nexibir('}')\n}\n";
        let formatted = normalize_whitespace(source);
        assert_eq!(
            formatted,
            "funcao principal(): vazio {\n    exibir(\"{ }\") // }\n    /* {\n     } */\n    exibir('}')\n}\n"
        );
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
    fn rejeita_sintaxe_invalida_sem_escrever() {
        let path =
            std::env::temp_dir().join(format!("nebula-format-invalid-{}.nbx", std::process::id()));
        std::fs::write(&path, "variavel =").unwrap();
        assert!(format_file(&path, false).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "variavel =");
        let _ = std::fs::remove_file(path);
    }
}
