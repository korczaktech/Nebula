//! Testes de integração do backend ELF gerado integralmente em Rust.

use std::fs;
use std::process::Command;

fn caminhos(rotulo: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let base = format!("nebula-elf-{rotulo}-{}", std::process::id());
    (
        std::env::temp_dir().join(format!("{base}.nbx")),
        std::env::temp_dir().join(base),
    )
}

fn compilar_e_executar(rotulo: &str, codigo: &str) -> std::process::Output {
    let (origem, executavel) = caminhos(rotulo);
    fs::write(&origem, codigo).expect("grava fonte Nebula");
    let compilacao = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&executavel)
        .output()
        .expect("executa CLI Rust");
    assert!(
        compilacao.status.success(),
        "falha de compilação: {}",
        String::from_utf8_lossy(&compilacao.stderr)
    );
    let execucao = Command::new(&executavel)
        .output()
        .expect("executa ELF gerado");
    let _ = fs::remove_file(origem);
    let _ = fs::remove_file(executavel);
    execucao
}

#[test]
fn gera_elf_executavel_com_codigo_de_saida() {
    let resultado = compilar_e_executar("retorno", "funcao principal(): inteiro { devolver 42 }");
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_variaveis_aritmetica_e_atribuicao() {
    let resultado = compilar_e_executar(
        "aritmetica",
        "funcao principal(): inteiro {\n    variavel total: inteiro = 10\n    total = total * 4 + 2\n    devolver total\n}",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_funcoes_parametros_e_chamadas() {
    let resultado = compilar_e_executar(
        "funcao",
        "funcao dobrar(valor: inteiro): inteiro { devolver valor * 2 }\nfuncao principal(): inteiro { devolver dobrar(21) }",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_condicionais_e_comparacoes() {
    let resultado = compilar_e_executar(
        "condicional",
        "funcao principal(): inteiro {\n    quando 6 * 7 == 42 { devolver 42 } senao { devolver 1 }\n}",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_laco_enquanto() {
    let resultado = compilar_e_executar(
        "enquanto",
        "funcao principal(): inteiro {\n    variavel i: inteiro = 0\n    enquanto i < 42 { i = i + 1 }\n    devolver i\n}",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_laco_para_e_controle() {
    let resultado = compilar_e_executar(
        "para",
        "funcao principal(): inteiro {\n    variavel total: inteiro = 0\n    para (variavel i = 0; i < 10; i = i + 1) {\n        quando i == 2 { seguir }\n        quando i == 8 { parar }\n        total = total + i\n    }\n    devolver total\n}",
    );
    assert_eq!(resultado.status.code(), Some(26));
}

#[test]
fn rejeita_recurso_que_depende_do_runtime_adiado() {
    let (origem, executavel) = caminhos("runtime-adiado");
    fs::write(&origem, "funcao principal(): vazio { exibir(42) }").unwrap();
    let resultado = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&executavel)
        .output()
        .unwrap();
    assert!(!resultado.status.success());
    assert!(String::from_utf8_lossy(&resultado.stderr).contains("runtime"));
    assert!(!executavel.exists());
    let _ = fs::remove_file(origem);
}

#[test]
fn compila_instrucoes_globais_sem_funcao_principal() {
    let resultado = compilar_e_executar(
        "globais",
        "variavel total: inteiro = 42",
    );
    assert_eq!(resultado.status.code(), Some(0));
}
