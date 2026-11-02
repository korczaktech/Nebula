//! Testes ponta a ponta do backend nativo pela CLI.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn arquivo_temporario(rotulo: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("nebula-e2e-{rotulo}-{}.nbx", std::process::id()))
}

#[test]
#[rustfmt::skip]
fn compila_e_executa_programa_nativo() {
    let origem = arquivo_temporario("valido");
    fs::write(
        &origem,
        "funcao dobro(valor: inteiro): inteiro {\n    devolver valor * 2\n}\nfuncao principal(): vazio {\n    variavel resposta: inteiro = dobro(21)\n    quando resposta == 42 {\n        exibir(resposta)\n    } senao {\n        exibir(0)\n    }\n}\n",
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
        .arg("-O2")
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
    let execucao_cli = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("executar")
        .arg(&origem)
        .output()
        .unwrap();
    assert!(execucao_cli.status.success());
    assert_eq!(String::from_utf8_lossy(&execucao_cli.stdout).trim(), "42");
    let _ = fs::remove_file(origem);
    let _ = fs::remove_file(exe);
}

#[test]
#[rustfmt::skip]
fn rejeita_codigo_semantica_ou_sintaticamente_invalido() {
    let origem = arquivo_temporario("invalido");
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

#[test]
#[rustfmt::skip]
fn compila_texto_utf8_do_exemplo_oficial() {
    let origem = arquivo_temporario("texto");
    fs::write(&origem, include_str!("../../../exemplos/ola_mundo.nbx")).unwrap();
    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("executar")
        .arg(&origem)
        .output()
        .unwrap();
    assert!(resultado.status.success(), "{}", String::from_utf8_lossy(&resultado.stderr));
    assert_eq!(String::from_utf8_lossy(&resultado.stdout).trim(), "Olá, Nebula!");
    let _ = fs::remove_file(origem);
}

#[test]
#[rustfmt::skip]
fn compila_e_executa_laco_do_benchmark() {
    let origem = arquivo_temporario("benchmark");
    fs::write(&origem, include_str!("../../../benchmarks/loop.nbx")).unwrap();
    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("executar")
        .arg(&origem)
        .output()
        .unwrap();
    assert!(resultado.status.success(), "{}", String::from_utf8_lossy(&resultado.stderr));
    assert_eq!(String::from_utf8_lossy(&resultado.stdout).trim(), "499999500000");
    let _ = fs::remove_file(origem);
}

#[test]
#[rustfmt::skip]
fn aceita_todos_os_niveis_de_otimizacao() {
    let origem = arquivo_temporario("otimizacao");
    fs::write(&origem, "funcao principal(): vazio {\n    exibir(6 * 7)\n}\n").unwrap();
    for nivel in ["-O0", "-O1", "-O2", "-O3", "-Os"] {
        let nome = nivel.trim_start_matches('-');
        let exe = std::env::temp_dir().join(format!("nebula-e2e-{nome}-{}.out", std::process::id()));
        let compilacao = Command::new(env!("CARGO_BIN_EXE_nebula"))
            .arg("compilar").arg(&origem).arg("-o").arg(&exe).arg(nivel).output().unwrap();
        assert!(compilacao.status.success(), "{nivel}: {}", String::from_utf8_lossy(&compilacao.stderr));
        let execucao = Command::new(&exe).output().unwrap();
        assert!(execucao.status.success());
        assert_eq!(String::from_utf8_lossy(&execucao.stdout).trim(), "42");
        let _ = fs::remove_file(exe);
    }
    let _ = fs::remove_file(origem);
}

#[test]
#[rustfmt::skip]
fn biblioteca_padrao_le_texto_da_entrada() {
    let origem = arquivo_temporario("entrada");
    fs::write(
        &origem,
        "funcao principal(): vazio {\n    variavel entrada: texto = ler_texto()\n    exibir(entrada)\n}\n",
    ).unwrap();
    let mut processo = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("executar")
        .arg(&origem)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    processo.stdin.as_mut().unwrap().write_all("Nebula UTF-8\n".as_bytes()).unwrap();
    let resultado = processo.wait_with_output().unwrap();
    assert!(resultado.status.success(), "{}", String::from_utf8_lossy(&resultado.stderr));
    assert_eq!(String::from_utf8_lossy(&resultado.stdout).trim(), "Nebula UTF-8");
    let _ = fs::remove_file(origem);
}
