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
    assert_eq!(String::from_utf8_lossy(&resultado.stdout).trim(), "1249999975000000");
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

#[test]
#[rustfmt::skip]
fn compila_lista_homogenea_e_indexacao() {
    let origem = arquivo_temporario("lista");
    fs::write(&origem, "funcao principal(): vazio {\n    variavel valores = [10, 20, 30]\n    exibir(valores[1])\n}\n").unwrap();
    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("executar").arg(&origem).output().unwrap();
    assert!(resultado.status.success(), "{}", String::from_utf8_lossy(&resultado.stderr));
    assert_eq!(String::from_utf8_lossy(&resultado.stdout).trim(), "20");
    let _ = fs::remove_file(origem);
}

#[test]
#[rustfmt::skip]
fn concatena_texto_no_backend_nativo() {
    let origem = arquivo_temporario("concat");
    fs::write(&origem, "funcao principal(): vazio {\n    variavel nome: texto = \"Nebula\"\n    exibir(\"Olá, \" + nome)\n}\n").unwrap();
    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("executar").arg(&origem).output().unwrap();
    assert!(resultado.status.success(), "{}", String::from_utf8_lossy(&resultado.stderr));
    assert_eq!(String::from_utf8_lossy(&resultado.stdout).trim(), "Olá, Nebula");
    let _ = fs::remove_file(origem);
}

#[test]
#[rustfmt::skip]
fn compila_laco_para_e_controle_de_fluxo() {
    let origem = arquivo_temporario("para");
    fs::write(&origem, "funcao principal(): vazio {\n    variavel total: inteiro = 0\n    para (variavel i = 0; i < 5; i = i + 1) {\n        quando i == 2 { seguir }\n        quando i == 4 { parar }\n        total = total + i\n    }\n    exibir(total)\n}\n").unwrap();
    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("executar").arg(&origem).output().unwrap();
    assert!(resultado.status.success(), "{}", String::from_utf8_lossy(&resultado.stderr));
    assert_eq!(String::from_utf8_lossy(&resultado.stdout).trim(), "4");
    let _ = fs::remove_file(origem);
}

#[test]
#[rustfmt::skip]
fn imprime_tipos_escalares_nativos() {
    let origem = arquivo_temporario("escalares");
    fs::write(&origem, "funcao principal(): vazio {\n    exibir(3.5)\n    exibir(verdadeiro)\n    exibir('A')\n}\n").unwrap();
    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("executar").arg(&origem).output().unwrap();
    assert!(resultado.status.success(), "{}", String::from_utf8_lossy(&resultado.stderr));
    assert_eq!(String::from_utf8_lossy(&resultado.stdout).trim(), "3.5\ntrue\nA");
    let _ = fs::remove_file(origem);
}

#[test]
#[rustfmt::skip]
fn compila_chamadas_recursivas() {
    let origem = arquivo_temporario("recursao");
    fs::write(&origem, include_str!("../../../benchmarks/fibonacci.nbx")).unwrap();
    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("executar").arg(&origem).output().unwrap();
    assert!(resultado.status.success(), "{}", String::from_utf8_lossy(&resultado.stderr));
    assert_eq!(String::from_utf8_lossy(&resultado.stdout).trim(), "24157817");
    let _ = fs::remove_file(origem);
}

#[test]
#[rustfmt::skip]
fn passa_e_retorna_listas_em_funcoes() {
    let origem = arquivo_temporario("lista-funcao");
    fs::write(&origem, "funcao primeiro(itens: [inteiro]): inteiro {\n    devolver itens[1]\n}\nfuncao principal(): vazio {\n    variavel itens = [7, 8]\n    exibir(primeiro(itens))\n}\n").unwrap();
    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("executar").arg(&origem).output().unwrap();
    assert!(resultado.status.success(), "{}", String::from_utf8_lossy(&resultado.stderr));
    assert_eq!(String::from_utf8_lossy(&resultado.stdout).trim(), "8");
    let _ = fs::remove_file(origem);
}
