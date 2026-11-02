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
fn overflow_de_divisao_signed_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-divisao",
        "funcao principal(): inteiro { variavel minimo: inteiro = -9223372036854775807 - 1; devolver minimo / -1 }",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn expoente_acima_de_u32_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "expoente-acima-u32",
        "funcao principal(): inteiro { devolver 1 ** 4294967296 }",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn aceita_expoente_maximo_da_ir() {
    let resultado = compilar_e_executar(
        "expoente-maximo-u32",
        "funcao principal(): inteiro { devolver 1 ** 4294967295 }",
    );
    assert_eq!(resultado.status.code(), Some(1));
}

#[test]
fn overflow_de_resto_signed_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-resto",
        "funcao principal(): inteiro { variavel minimo: inteiro = -9223372036854775807 - 1; devolver minimo % -1 }",
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
fn overflow_de_soma_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-soma",
        "funcao principal(): inteiro { devolver 9223372036854775807 + 1 }",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_multiplicacao_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-multiplicacao",
        "funcao principal(): inteiro { devolver 3037000500 * 3037000500 }",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_potencia_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-potencia",
        "funcao principal(): inteiro { devolver 2 ** 63 }",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_subtracao_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-subtracao",
        "funcao principal(): inteiro { devolver -9223372036854775807 - 2 }",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_negacao_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-negacao",
        "funcao principal(): inteiro { variavel minimo: inteiro = -9223372036854775807 - 1; devolver -minimo }",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn aceita_literal_minimo_de_inteiro_assinado() {
    let resultado = compilar_e_executar(
        "inteiro-minimo-literal",
        "funcao principal(): inteiro { devolver -9223372036854775808 }",
    );
    assert_eq!(resultado.status.code(), Some(0));
}

#[test]
fn divisao_e_resto_negativos_seguem_semantica_signed() {
    let divisao = compilar_e_executar(
        "divisao-negativa",
        "funcao principal(): inteiro { devolver -7 / 2 }",
    );
    assert_eq!(divisao.status.code(), Some(253));
    let resto = compilar_e_executar(
        "resto-negativo",
        "funcao principal(): inteiro { devolver -5 % 2 }",
    );
    assert_eq!(resto.status.code(), Some(255));
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
fn variavel_local_sombreia_global_sem_altera_la() {
    let resultado = compilar_e_executar(
        "global-sombreada",
        "variavel valor: inteiro = 10\nfuncao principal(): inteiro { variavel valor: inteiro = 42; devolver valor }",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn inicializadores_globais_respeitam_ordem_de_declaracao() {
    let resultado = compilar_e_executar(
        "globais-ordem",
        "variavel base: inteiro = 10\nvariavel total: inteiro = base + 5\nfuncao principal(): inteiro { devolver total }",
    );
    assert_eq!(resultado.status.code(), Some(15));
}

#[test]
fn variaveis_globais_sao_compartilhadas_entre_funcoes() {
    let resultado = compilar_e_executar(
        "globais-compartilhadas",
        "variavel total: inteiro = 40\nfuncao incrementar(): inteiro { devolver total + 2 }\nfuncao principal(): inteiro { total = incrementar(); devolver total }",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_seis_parametros_pelo_abi_system_v() {
    let resultado = compilar_e_executar(
        "seis-parametros",
        "funcao somar(primeiro: inteiro, segundo: inteiro, terceiro: inteiro, quarto: inteiro, quinto: inteiro, sexto: inteiro): inteiro { devolver primeiro + segundo + terceiro + quarto + quinto + sexto }\nfuncao principal(): inteiro { devolver somar(1, 2, 3, 4, 5, 6) }",
    );
    assert_eq!(resultado.status.code(), Some(21));
}

#[test]
fn compila_chamadas_aninhadas_sem_perder_alinhamento() {
    let resultado = compilar_e_executar(
        "chamadas-aninhadas",
        "funcao dobrar(valor: inteiro): inteiro { devolver valor * 2 }\nfuncao somar(a: inteiro, b: inteiro): inteiro { devolver a + b }\nfuncao principal(): inteiro { devolver somar(dobrar(2), dobrar(3)) }",
    );
    assert_eq!(resultado.status.code(), Some(10));
}

#[test]
fn preserva_sombreamento_de_variaveis_em_blocos() {
    let resultado = compilar_e_executar(
        "sombreamento",
        "funcao principal(): inteiro { variavel valor: inteiro = 1; quando verdadeiro { variavel valor: inteiro = 41; devolver valor }; devolver valor }",
    );
    assert_eq!(resultado.status.code(), Some(41));
}

#[test]
fn compila_negacao_logica_unaria() {
    let resultado = compilar_e_executar(
        "negacao-logica",
        "funcao principal(): inteiro { quando nao falso { devolver 42 }; devolver 1 }",
    );
    assert_eq!(resultado.status.code(), Some(42));
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

#[test]
fn chamada_no_operando_direito_preserva_resultado_e_alinhamento() {
    let resultado = compilar_e_executar(
        "chamada-operando-direito",
        "funcao identidade(valor: inteiro): inteiro { devolver valor }\nfuncao principal(): inteiro { devolver 40 + identidade(2) }",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_retorno_logico_e_constantes_booleanas() {
    let resultado = compilar_e_executar(
        "retorno-logico",
        "funcao habilitado(): logico { devolver verdadeiro }\nfuncao principal(): inteiro { quando habilitado() e nao falso { devolver 42 }; devolver 1 }",
    );
    assert_eq!(resultado.status.code(), Some(42));
}
