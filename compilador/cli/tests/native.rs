//! Testes de integração do backend ELF gerado integralmente em Rust.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

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
fn compila_sem_compilador_externo_no_path() {
    let (origem, executavel) = caminhos("sem-toolchain-c");
    fs::write(&origem, "funcao principal(): inteiro { devolver 42 }").unwrap();
    let compilacao = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .env("PATH", "")
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&executavel)
        .output()
        .unwrap();
    assert!(
        compilacao.status.success(),
        "{}",
        String::from_utf8_lossy(&compilacao.stderr)
    );
    let execucao = Command::new(&executavel).output().unwrap();
    assert_eq!(execucao.status.code(), Some(42));
    let _ = fs::remove_file(origem);
    let _ = fs::remove_file(executavel);
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
fn compila_divisao_resto_e_operacoes_com_sinal() {
    let resultado = compilar_e_executar(
        "divisao-resto",
        "funcao principal(): inteiro {\n    quando -2 < -1 { devolver 84 / 2 + 43 % 5 }\n    devolver 1\n}",
    );
    assert_eq!(resultado.status.code(), Some(45));
}

#[test]
fn compila_potencia_inteira_por_quadratura() {
    let resultado = compilar_e_executar(
        "potencia",
        "funcao principal(): inteiro { devolver 3 ** 3 }",
    );
    assert_eq!(resultado.status.code(), Some(27));
}

#[test]
fn potencia_usa_expoente_calculado_em_tempo_de_execucao() {
    let resultado = compilar_e_executar(
        "potencia-dinamica",
        "funcao principal(): inteiro { variavel expoente: inteiro = 3; devolver 2 ** expoente }",
    );
    assert_eq!(resultado.status.code(), Some(8));
}

#[test]
fn expoente_zero_produz_unidade() {
    let resultado = compilar_e_executar(
        "potencia-zero",
        "funcao principal(): inteiro { devolver 7 ** 0 }",
    );
    assert_eq!(resultado.status.code(), Some(1));
}

#[test]
fn divisao_por_zero_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "divisao-zero",
        "funcao principal(): inteiro { devolver 42 / 0 }",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn expoente_negativo_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "expoente-negativo",
        "funcao principal(): inteiro { devolver 2 ** -1 }",
    );
    assert_eq!(resultado.status.code(), Some(2));
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
fn operadores_logicos_respeitam_curto_circuito() {
    let resultado = compilar_e_executar(
        "curto-circuito",
        "funcao principal(): inteiro {\n    quando falso e (1 / 0 == 0) { devolver 1 }\n    quando verdadeiro ou (1 / 0 == 0) { devolver 42 }\n    devolver 2\n}",
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
fn compila_chamadas_recursivas_com_alinhamento_de_pilha() {
    let resultado = compilar_e_executar(
        "recursao",
        "funcao fibonacci(n: inteiro): inteiro {\n    quando n <= 1 { devolver n }\n    devolver fibonacci(n - 1) + fibonacci(n - 2)\n}\nfuncao principal(): inteiro { devolver fibonacci(10) }",
    );
    assert_eq!(resultado.status.code(), Some(55));
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
    let resultado = compilar_e_executar("globais", "variavel total: inteiro = 42");
    assert_eq!(resultado.status.code(), Some(0));
}
