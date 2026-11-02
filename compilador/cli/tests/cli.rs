//! Testes de integração dos comandos da CLI Nebula.

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
    assert!(stdout.contains("verificar <arquivo>"));
    assert!(stdout.contains("não realiza análise semântica"));
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
    assert!(String::from_utf8_lossy(&saida.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn comando_desconhecido_retorna_codigo_dois() {
    let saida = executar(&["comando-inexistente"]);
    assert_eq!(saida.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&saida.stderr).contains("Comando desconhecido"));
}

#[test]
fn verificar_aceita_programa_sintaticamente_valido() {
    let caminho = std::env::temp_dir().join(format!("nebula-cli-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel total = 1 + 2").expect("grava fonte");
    let saida = executar(&["verificar", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(String::from_utf8_lossy(&saida.stdout).contains("válido sintaticamente"));
}

#[test]
fn verificar_rejeita_programa_com_sintaxe_invalida() {
    let caminho = std::env::temp_dir().join(format!("nebula-erro-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel =").expect("grava fonte");
    let saida = executar(&["verificar", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    assert_eq!(saida.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&saida.stderr).contains("N2010"));
}

#[test]
fn verificar_sem_caminho_retorna_codigo_dois() {
    let saida = executar(&["verificar"]);
    assert_eq!(saida.status.code(), Some(2));
}

#[test]
fn verificar_rejeita_arquivo_sem_utf8_valido() {
    let caminho = std::env::temp_dir().join(format!("nebula-bytes-{}.nbx", std::process::id()));
    std::fs::write(&caminho, vec![255_u8, 254_u8]).expect("grava bytes de teste");
    let saida = executar(&["verificar", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    assert_eq!(saida.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&saida.stderr).contains("UTF-8 válido"));
}

#[test]
fn verificar_rejeita_argumento_extra() {
    let saida = executar(&["verificar", "programa.nbx", "argumento-extra"]);
    assert_eq!(saida.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&saida.stderr).contains("somente um arquivo"));
}

#[test]
fn verificar_mostra_localizacao_do_erro() {
    let caminho = std::env::temp_dir().join(format!("nebula-local-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel =").expect("grava fonte");
    let saida = executar(&["verificar", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    let stderr = String::from_utf8_lossy(&saida.stderr);
    assert!(stderr.contains(":1:"), "{stderr}");
    assert!(stderr.contains("N2010"), "{stderr}");
}
