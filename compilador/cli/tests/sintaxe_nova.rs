//! Testes diretos da sintaxe atual do Function Body.

use std::fs;
use std::process::Command;

fn caminhos(rotulo: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let base = format!("nebula-sintaxe-atual-{rotulo}-{}", std::process::id());
    (
        std::env::temp_dir().join(format!("{base}.nbx")),
        std::env::temp_dir().join(base),
    )
}

#[test]
fn rejeita_funcao_na_sintaxe_antiga_sem_function_body() {
    let (origem, _) = caminhos("legado");
    fs::write(&origem, "funcao principal()\n    passar\nfim").unwrap();

    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("verificar")
        .arg(&origem)
        .output()
        .expect("executa CLI");

    let _ = fs::remove_file(origem);
    assert!(!resultado.status.success());
    let stderr = String::from_utf8_lossy(&resultado.stderr);
    assert!(stderr.contains("N2007"), "{stderr}");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn compila_e_executa_function_body_aninhado_sem_normalizacao() {
    let (origem, executavel) = caminhos("aninhado");
    let codigo = r#"funcao principal() -> inteiro: {
    variavel total = 40
    se total == 40: {
        devolver total + 2
        fim
    }
    senao: {
        devolver 0
        fim
    }
    fim
    fim
} fim
"#;
    fs::write(&origem, codigo).unwrap();

    let compilacao = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&executavel)
        .output()
        .expect("executa CLI");

    assert!(
        compilacao.status.success(),
        "{}",
        String::from_utf8_lossy(&compilacao.stderr)
    );

    let execucao = Command::new(&executavel)
        .output()
        .expect("executa ELF gerado");

    let _ = fs::remove_file(origem);
    let _ = fs::remove_file(executavel);
    assert_eq!(execucao.status.code(), Some(42));
}

#[test]
fn distingue_function_body_de_dicionario_e_valor_estrutural() {
    let (origem, _) = caminhos("dados");
    let codigo = r#"estrutura Pessoa: {
    nome: texto
    fim
} fim

variavel pessoa = Pessoa { nome: "Nebula", fim }
variavel dados: dicionario = {
    "nome": "Nebula"
}
"#;
    fs::write(&origem, codigo).unwrap();

    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("verificar")
        .arg(&origem)
        .output()
        .expect("executa CLI");

    let _ = fs::remove_file(origem);
    assert!(
        resultado.status.success(),
        "{}",
        String::from_utf8_lossy(&resultado.stderr)
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn compila_cada_em_intervalos_crescentes_e_decrescentes() {
    let (origem, executavel) = caminhos("cada");
    let codigo = r#"funcao principal() -> inteiro: {
    variavel total = 0
    cada item em 1..3: {
        total += item
        fim
    } fim
    cada item em 3..1: {
        total += item
        fim
    } fim
    devolver total
    fim
} fim
"#;
    fs::write(&origem, codigo).unwrap();

    let compilacao = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&executavel)
        .output()
        .expect("executa CLI");

    assert!(
        compilacao.status.success(),
        "{}",
        String::from_utf8_lossy(&compilacao.stderr)
    );

    let execucao = Command::new(&executavel)
        .output()
        .expect("executa ELF gerado");

    let _ = fs::remove_file(origem);
    let _ = fs::remove_file(executavel);
    assert_eq!(execucao.status.code(), Some(12));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn compila_pulso_nativo_com_pare() {
    let (origem, executavel) = caminhos("pulso");
    let codigo = r#"funcao principal() -> inteiro: {
    variavel total = 0
    pulso(0): {
        total += 1
        se total == 3: {
            pare
            fim
        }
        fim
        fim
    } fim
    devolver total
    fim
} fim
"#;
    fs::write(&origem, codigo).unwrap();

    let compilacao = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&executavel)
        .output()
        .expect("executa CLI");

    assert!(
        compilacao.status.success(),
        "{}",
        String::from_utf8_lossy(&compilacao.stderr)
    );

    let execucao = Command::new(&executavel)
        .output()
        .expect("executa ELF gerado");

    let _ = fs::remove_file(origem);
    let _ = fs::remove_file(executavel);
    assert_eq!(execucao.status.code(), Some(3));
}
