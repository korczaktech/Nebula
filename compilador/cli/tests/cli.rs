//! Testes de integração dos comandos informativos da CLI Nebula.

use std::process::{Command, Output};

fn executar(argumentos: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nebula"))
        .args(argumentos)
        .output()
        .expect("a CLI deve iniciar")
}

#[test]
fn ajuda_explicita_retorna_sucesso_e_instrucoes() {
    let saida = executar(&["ajuda"]);

    assert!(saida.status.success());
    let stdout = String::from_utf8_lossy(&saida.stdout);
    assert!(stdout.contains("Uso: nebula <comando>"));
    assert!(stdout.contains("ajuda"));
    assert!(stdout.contains("versao"));
    assert!(stdout.contains("ainda não estão disponíveis"));
}

#[test]
fn ausencia_de_comando_exibe_ajuda() {
    let saida = executar(&[]);

    assert!(saida.status.success());
    assert!(String::from_utf8_lossy(&saida.stdout).contains("Uso: nebula <comando>"));
}

#[test]
fn versao_exibe_versao_do_pacote() {
    let saida = executar(&["versao"]);

    assert!(saida.status.success());
    let stdout = String::from_utf8_lossy(&saida.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn comando_desconhecido_retorna_codigo_dois() {
    let saida = executar(&["comando-inexistente"]);

    assert_eq!(saida.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&saida.stderr).contains("Comando desconhecido"));
}
