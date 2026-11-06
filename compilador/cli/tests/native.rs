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

fn normalizar_blocos_legados(codigo: &str) -> String {
    let codigo = codigo.replace("): ", ") -> ");
    if codigo.contains(": {") {
        return codigo;
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TipoBloco {
        Completo,
        Condicional,
        Escolha,
        Caso,
    }

    struct Bloco {
        indentacao: usize,
        tipo: TipoBloco,
    }

    fn indentacao(texto: &str, nivel: usize) -> String {
        format!("{}{}", " ".repeat(nivel), texto)
    }

    fn fechar(saida: &mut Vec<String>, bloco: Bloco) {
        match bloco.tipo {
            TipoBloco::Caso => {
                saida.push(indentacao("fim", bloco.indentacao));
                saida.push(indentacao("}", bloco.indentacao));
            }
            TipoBloco::Completo | TipoBloco::Condicional | TipoBloco::Escolha => {
                saida.push(indentacao("fim", bloco.indentacao));
                saida.push(indentacao("}", bloco.indentacao));
                saida.push(indentacao("fim", bloco.indentacao));
            }
        }
    }

    let mut saida = Vec::new();
    let mut blocos: Vec<Bloco> = Vec::new();

    for linha in codigo.lines() {
        let conteudo = linha.trim();
        let nivel = linha.len() - linha.trim_start().len();

        if conteudo.is_empty() {
            saida.push(linha.to_owned());
            continue;
        }

        if conteudo.starts_with("senao") {
            if let Some(bloco) = blocos.last() {
                if bloco.tipo == TipoBloco::Condicional && bloco.indentacao == nivel {
                    saida.push(indentacao("fim", nivel));
                    saida.push(indentacao("}", nivel));
                    saida.push(indentacao("senao: {", nivel));
                    continue;
                }
            }
        }

        if conteudo.starts_with("caso ") || conteudo == "padrao" {
            if blocos
                .last()
                .is_some_and(|bloco| bloco.tipo == TipoBloco::Caso && bloco.indentacao == nivel)
            {
                if let Some(bloco) = blocos.pop() {
                    fechar(&mut saida, bloco);
                }
            }
            saida.push(indentacao(&format!("{conteudo}: {{"), nivel));
            blocos.push(Bloco {
                indentacao: nivel,
                tipo: TipoBloco::Caso,
            });
            continue;
        }

        if conteudo == "fim" {
            while blocos
                .last()
                .is_some_and(|bloco| bloco.tipo == TipoBloco::Caso && bloco.indentacao > nivel)
            {
                if let Some(bloco) = blocos.pop() {
                    fechar(&mut saida, bloco);
                }
            }

            if blocos.last().is_some_and(|bloco| bloco.indentacao == nivel) {
                if let Some(bloco) = blocos.pop() {
                    fechar(&mut saida, bloco);
                }
            } else {
                saida.push(linha.to_owned());
            }
            continue;
        }

        let (tipo, cabecalho) =
            if conteudo.starts_with("publico funcao ") || conteudo.starts_with("funcao ") {
                let assinatura = conteudo.replace("): ", ") -> ");
                let assinatura = if assinatura.ends_with(':') {
                    format!("{assinatura} {{")
                } else {
                    format!("{assinatura}: {{")
                };
                (Some(TipoBloco::Completo), Some(assinatura))
            } else if conteudo.starts_with("estrutura ") || conteudo.starts_with("enumeracao ") {
                let assinatura = if conteudo.ends_with(':') {
                    format!("{conteudo} {{")
                } else {
                    format!("{conteudo}: {{")
                };
                (Some(TipoBloco::Completo), Some(assinatura))
            } else if conteudo.starts_with("escolha ") || conteudo.starts_with("avaliar ") {
                (Some(TipoBloco::Escolha), Some(format!("{conteudo}: {{")))
            } else if conteudo.starts_with("quando ") || conteudo.starts_with("se ") {
                (
                    Some(TipoBloco::Condicional),
                    Some(format!("{conteudo}: {{")),
                )
            } else if conteudo.starts_with("cada ")
                || conteudo.starts_with("enquanto ")
                || conteudo.starts_with("para ")
                || conteudo.starts_with("pulso(")
                || conteudo.starts_with("durante ")
                || conteudo.starts_with("repetir ")
            {
                (Some(TipoBloco::Completo), Some(format!("{conteudo}: {{")))
            } else {
                (None, None)
            };
        if let (Some(tipo), Some(cabecalho)) = (tipo, cabecalho) {
            saida.push(indentacao(&cabecalho, nivel));
            blocos.push(Bloco {
                indentacao: nivel,
                tipo,
            });
        } else {
            saida.push(linha.to_owned());
        }
    }

    saida.join("\n")
}

fn compilar_e_executar(rotulo: &str, codigo: &str) -> std::process::Output {
    let (origem, executavel) = caminhos(rotulo);
    let codigo_normalizado = normalizar_blocos_legados(codigo);
    fs::write(&origem, codigo_normalizado).expect("grava fonte Nebula");
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
    let resultado = compilar_e_executar(
        "retorno",
        "funcao principal(): inteiro\n    devolver 42\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_sem_compilador_externo_no_path() {
    let (origem, executavel) = caminhos("sem-toolchain-c");
    let codigo = normalizar_blocos_legados("funcao principal(): inteiro\n    devolver 42\nfim");
    fs::write(&origem, codigo).unwrap();
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
        "funcao principal(): inteiro\n    variavel total: inteiro = 10\n    total = total * 4 + 2\n    devolver total\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_valor_de_funcao_e_chamada_indireta() {
    let resultado = compilar_e_executar(
        "funcao-indireta",
        "funcao somar(valor: inteiro): inteiro\n    devolver valor + 1\nfim\nfuncao principal(): inteiro\n    variavel callback: funcao(inteiro): inteiro = somar\n    devolver callback(41)\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_parametros_e_retornos_decimais_no_abi_proprio() {
    let resultado = compilar_e_executar(
        "abi-decimal",
        "funcao ajustar(valor: decimal64): decimal64\n    devolver valor + 1.0\nfim\nfuncao principal(): inteiro\n    variavel total = ajustar(41.0)\n    quando total == 42.0\n        devolver 42\n    fim\n    \n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_campos_decimais_em_estruturas() {
    let resultado = compilar_e_executar(
        "estrutura-decimal",
        "estrutura Medida:\n    valor: decimal64\n    codigo: inteiro\nfim\nfuncao principal(): inteiro\n    variavel medida = Medida { valor: 42.0, codigo: 7, fim }\n    quando medida.valor == 42.0\n        devolver medida.codigo + 35\n    fim\n    \n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_elementos_decimais_em_tuplas() {
    let resultado = compilar_e_executar(
        "tupla-decimal",
        "funcao principal(): inteiro\n    variavel medida = (42.0, 7)\n    quando medida[0] == 42.0\n        devolver medida[1] + 35\n    fim\n    \n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_payload_decimal_em_enumeracoes() {
    let resultado = compilar_e_executar(
        "enum-decimal",
        "enumeracao Leitura:\n    Numero(decimal64)\n    Codigo(inteiro)\nfim\nfuncao principal(): inteiro\n    variavel leitura = Leitura::Numero(42.0)\n    escolha leitura\n        caso Leitura::Numero(valor)\n            quando valor == 42.0\n                devolver 42\n            fim\n        caso Leitura::Codigo(valor)\n            passar\n    fim\n    \n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_funcoes_parametros_e_chamadas() {
    let resultado = compilar_e_executar(
        "funcao",
        "funcao dobrar(valor: inteiro): inteiro\n    devolver valor * 2\nfim\nfuncao principal(): inteiro\n    devolver dobrar(21)\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn copia_explicitamente_estrutura_com_campos_copiaveis_no_backend() {
    let resultado = compilar_e_executar(
        "copia-explicita",
        "estrutura Ponto:\n    x: inteiro\n    y: inteiro\nfim\nfuncao principal(): inteiro\n    variavel original = Ponto { x: 40, y: 2, fim }\n    variavel copia = copiar(original)\n    quando original.x + original.y == copia.x + copia.y\n        devolver 42\n    fim\n    \n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn executa_copia_explicita_de_estrutura_no_backend_nativo() {
    let resultado = compilar_e_executar(
        "copia-estrutura",
        "estrutura Ponto:\n    x: inteiro8\nfim\nfuncao principal(): inteiro\n    variavel origem = Ponto { x: 7, fim }\n    variavel destino = copiar(origem)\n    quando origem.x == destino.x\n        devolver 42\n    fim\n    \n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn abi_transporta_inteiro128_com_parte_alta_em_parametro_e_retorno() {
    let resultado = compilar_e_executar(
        "abi-inteiro128",
        "funcao transportar(valor: inteiro128): inteiro128\n    devolver valor\nfim\nfuncao principal(): inteiro\n    variavel original: inteiro128 = 1267650600228229401496703205376\n    variavel recebido = transportar(original)\n    quando recebido == original\n        devolver 42\n    fim\n    \n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn abi_transporta_inteiro_u128_com_parte_alta_em_parametro_e_retorno() {
    let resultado = compilar_e_executar(
        "abi-inteiro-u128",
        "funcao transportar(valor: inteiro_u128): inteiro_u128\n    devolver valor\nfim\nfuncao principal(): inteiro\n    variavel original: inteiro_u128 = 340282366920938463463374607431768211455\n    variavel recebido = transportar(original)\n    quando recebido == original\n        devolver 42\n    fim\n    \n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn executa_aritmetica_signed128_acima_de_64_bits() {
    let resultado = compilar_e_executar(
        "aritmetica-signed128",
        "funcao principal(): inteiro\n    variavel base: inteiro128 = 1267650600228229401496703205376\n    variavel dobro = base * 2\n    quando dobro / 2 == base\n        devolver 42\n    fim\n    \n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn executa_aritmetica_unsigned128_acima_de_64_bits() {
    let resultado = compilar_e_executar(
        "aritmetica-unsigned128",
        "funcao principal(): inteiro\n    variavel base: inteiro_u128 = 18446744073709551619\n    variavel fator: inteiro_u128 = 3\n    variavel produto = base * fator\n    quando produto / fator == base\n        devolver 42\n    fim\n    \n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn monomorfiza_funcao_generica_no_backend_nativo() {
    let resultado = compilar_e_executar(
        "generica",
        "funcao identidade<T>(valor: T): T\n    devolver valor\nfim\nfuncao principal(): inteiro\n    devolver identidade(42)\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_divisao_resto_e_operacoes_com_sinal() {
    let resultado = compilar_e_executar(
        "divisao-resto",
        r#"funcao principal(): inteiro
    quando -2 < -1
        devolver 84 / 2 + 43 % 5
    fim
    devolver 1
fim"#,
    );
    assert_eq!(resultado.status.code(), Some(45));
}

#[test]
fn compila_potencia_inteira_por_quadratura() {
    let resultado = compilar_e_executar(
        "potencia",
        "funcao principal(): inteiro\n    devolver 3 ** 3\nfim",
    );
    assert_eq!(resultado.status.code(), Some(27));
}

#[test]
fn potencia_usa_expoente_calculado_em_tempo_de_execucao() {
    let resultado = compilar_e_executar(
        "potencia-dinamica",
        "funcao principal(): inteiro\n    variavel expoente: inteiro = 3\n    devolver 2 ** expoente\nfim",
    );
    assert_eq!(resultado.status.code(), Some(8));
}

#[test]
fn expoente_zero_produz_unidade() {
    let resultado = compilar_e_executar(
        "potencia-zero",
        "funcao principal(): inteiro\n    devolver 7 ** 0\nfim",
    );
    assert_eq!(resultado.status.code(), Some(1));
}

#[test]
fn divisao_por_zero_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "divisao-zero",
        "funcao principal(): inteiro\n    devolver 42 / 0\nfim",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_divisao_signed_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-divisao",
        "funcao principal(): inteiro\n    variavel minimo: inteiro = -9223372036854775807 - 1\n    devolver minimo / -1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn expoente_acima_de_u32_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "expoente-acima-u32",
        "funcao principal(): inteiro\n    devolver 1 ** 4294967296\nfim",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn aceita_expoente_maximo_da_ir() {
    let resultado = compilar_e_executar(
        "expoente-maximo-u32",
        "funcao principal(): inteiro\n    devolver 1 ** 4294967295\nfim",
    );
    assert_eq!(resultado.status.code(), Some(1));
}

#[test]
fn overflow_de_resto_signed_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-resto",
        "funcao principal(): inteiro\n    variavel minimo: inteiro = -9223372036854775807 - 1\n    devolver minimo % -1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn expoente_negativo_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "expoente-negativo",
        "funcao principal(): inteiro\n    devolver 2 ** -1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_soma_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-soma",
        "funcao principal(): inteiro\n    devolver 9223372036854775807 + 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_multiplicacao_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-multiplicacao",
        "funcao principal(): inteiro\n    devolver 3037000500 * 3037000500\nfim",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_potencia_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-potencia",
        "funcao principal(): inteiro\n    devolver 2 ** 63\nfim",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_subtracao_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-subtracao",
        "funcao principal(): inteiro\n    devolver -9223372036854775807 - 2\nfim",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_negacao_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-negacao",
        "funcao principal(): inteiro\n    variavel minimo: inteiro = -9223372036854775807 - 1\n    devolver -minimo\nfim",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn aceita_literal_minimo_de_inteiro_assinado() {
    let resultado = compilar_e_executar(
        "inteiro-minimo-literal",
        "funcao principal(): inteiro\n    devolver -9223372036854775808\nfim",
    );
    assert_eq!(resultado.status.code(), Some(0));
}

#[test]
fn divisao_e_resto_negativos_seguem_semantica_signed() {
    let divisao = compilar_e_executar(
        "divisao-negativa",
        "funcao principal(): inteiro\n    devolver -7 / 2\nfim",
    );
    assert_eq!(divisao.status.code(), Some(253));
    let resto = compilar_e_executar(
        "resto-negativo",
        "funcao principal(): inteiro\n    devolver -5 % 2\nfim",
    );
    assert_eq!(resto.status.code(), Some(255));
}

#[test]
fn compila_condicionais_e_comparacoes() {
    let resultado = compilar_e_executar(
        "condicional",
        r#"funcao principal(): inteiro
    quando 6 * 7 == 42
        devolver 42
    senao
        devolver 1
    fim
fim"#,
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn operadores_logicos_respeitam_curto_circuito() {
    let resultado = compilar_e_executar(
        "curto-circuito",
        r#"funcao principal(): inteiro
    quando falso e (1 / 0 == 0)
        devolver 1
    fim
    quando verdadeiro ou (1 / 0 == 0)
        devolver 42
    fim
    devolver 2
fim"#,
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_laco_enquanto() {
    let resultado = compilar_e_executar(
        "enquanto",
        r#"funcao principal(): inteiro
    variavel i: inteiro = 0
    enquanto i < 42
        i = i + 1
    fim
    devolver i
fim"#,
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_laco_para_e_controle() {
    let resultado = compilar_e_executar(
        "para",
        r#"funcao principal(): inteiro
    variavel total: inteiro = 0
    para (variavel i = 0; i < 10; i = i + 1)
        quando i == 2
            seguir
        fim
        quando i == 8
            parar
        fim
        total = total + i
    fim
    devolver total
fim"#,
    );
    assert_eq!(resultado.status.code(), Some(26));
}

#[test]
fn variavel_local_sombreia_global_sem_altera_la() {
    let resultado = compilar_e_executar(
        "global-sombreada",
        "variavel valor: inteiro = 10\nfuncao principal(): inteiro\n    variavel valor: inteiro = 42\n    devolver valor\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn inicializadores_globais_respeitam_ordem_de_declaracao() {
    let resultado = compilar_e_executar(
        "globais-ordem",
        "variavel base: inteiro = 10\nvariavel total: inteiro = base + 5\nfuncao principal(): inteiro\n    devolver total\nfim",
    );
    assert_eq!(resultado.status.code(), Some(15));
}

#[test]
fn variaveis_globais_sao_compartilhadas_entre_funcoes() {
    let resultado = compilar_e_executar(
        "globais-compartilhadas",
        "variavel total: inteiro = 40\nfuncao incrementar(): inteiro\n    devolver total + 2\nfim\nfuncao principal(): inteiro\n    total = incrementar()\n    devolver total\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_seis_parametros_pelo_abi_system_v() {
    let resultado = compilar_e_executar(
        "seis-parametros",
        "funcao somar(primeiro: inteiro, segundo: inteiro, terceiro: inteiro, quarto: inteiro, quinto: inteiro, sexto: inteiro): inteiro\n    devolver primeiro + segundo + terceiro + quarto + quinto + sexto\nfim\nfuncao principal(): inteiro\n    devolver somar(1, 2, 3, 4, 5, 6)\nfim",
    );
    assert_eq!(resultado.status.code(), Some(21));
}

#[test]
fn compila_chamadas_aninhadas_sem_perder_alinhamento() {
    let resultado = compilar_e_executar(
        "chamadas-aninhadas",
        "funcao dobrar(valor: inteiro): inteiro\n    devolver valor * 2\nfim\nfuncao somar(a: inteiro, b: inteiro): inteiro\n    devolver a + b\nfim\nfuncao principal(): inteiro\n    devolver somar(dobrar(2), dobrar(3))\nfim",
    );
    assert_eq!(resultado.status.code(), Some(10));
}

#[test]
fn preserva_sombreamento_de_variaveis_em_blocos() {
    let resultado = compilar_e_executar(
        "sombreamento",
        "funcao principal(): inteiro\n    variavel valor: inteiro = 1\n    \n    quando verdadeiro\n        variavel valor: inteiro = 41\n        devolver valor\n    fim\n    devolver valor\nfim",
    );
    assert_eq!(resultado.status.code(), Some(41));
}

#[test]
fn compila_negacao_logica_unaria() {
    let resultado = compilar_e_executar(
        "negacao-logica",
        "funcao principal(): inteiro\n    quando nao falso\n        devolver 42\n    fim\n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_chamadas_recursivas_com_alinhamento_de_pilha() {
    let resultado = compilar_e_executar(
        "recursao",
        r#"funcao fibonacci(n: inteiro): inteiro
    quando n <= 1
        devolver n
    fim
    devolver fibonacci(n - 1) + fibonacci(n - 2)
fim
funcao principal(): inteiro
    devolver fibonacci(10)
fim"#,
    );
    assert_eq!(resultado.status.code(), Some(55));
}

#[test]
fn rejeita_recurso_que_depende_do_runtime_adiado() {
    let (origem, executavel) = caminhos("runtime-adiado");
    let codigo = normalizar_blocos_legados("funcao principal(): vazio\n    exibir(42)\nfim");
    fs::write(&origem, codigo).unwrap();
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
        "funcao identidade(valor: inteiro): inteiro\n    devolver valor\nfim\nfuncao principal(): inteiro\n    devolver 40 + identidade(2)\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_retorno_logico_e_constantes_booleanas() {
    let resultado = compilar_e_executar(
        "retorno-logico",
        "funcao habilitado(): logico\n    devolver verdadeiro\nfim\nfuncao principal(): inteiro\n    quando habilitado() e nao falso\n        devolver 42\n    fim\n    devolver 1\nfim",
    );
    assert_eq!(resultado.status.code(), Some(42));
}
