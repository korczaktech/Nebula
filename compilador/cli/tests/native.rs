//! Testes ponta a ponta do backend nativo pela CLI.

use std::fs;
use std::process::Command;

fn arquivo_temporario() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("nebula-e2e-{}.nbx", std::process::id()))
}

#[test]
#[rustfmt::skip]
fn compila_e_executa_programa_nativo() {
    let origem = arquivo_temporario();
    fs::write(
        &origem,
        "funcao principal(): vazio {\n    exibir(40 + 2)\n}\n",
    )
    .unwrap();
    let exe = std::env::temp_dir().join(format!(
        "nebula-e2e-{}{}",
        std::process::id(),
        if cfg!(windows) { ".exe" } else { "" }
    ));
    let compilacao = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        compilacao.status.success(),
        "{}",
        String::from_utf8_lossy(&compilacao.stderr)
    );
    let execucao = Command::new(&exe).output().unwrap();
    assert!(execucao.status.success());
    assert_eq!(String::from_utf8_lossy(&execucao.stdout).trim(), "42");
    let _ = fs::remove_file(origem);
    let _ = fs::remove_file(exe);
}

#[test]
#[rustfmt::skip]
fn rejeita_codigo_semantica_ou_sintaticamente_invalido() {
    let origem = arquivo_temporario();
    fs::write(&origem, "funcao principal(): vazio {\n    ausente()\n}\n").unwrap();
    let exe = std::env::temp_dir().join(format!(
        "nebula-e2e-invalido-{}",
        std::process::id()
    ));
    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(!resultado.status.success());
    assert!(!exe.exists());
    let _ = fs::remove_file(origem);
}
