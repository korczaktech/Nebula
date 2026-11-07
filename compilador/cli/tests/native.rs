//! Testes de integração do backend ELF gerado integralmente em Rust.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn caminhos(rotulo: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let base = format!("nebula-elf-{rotulo}-{}", std::process::id());
    (
        std::env::temp_dir().join(format!("{base}.nbx")),
        std::env::temp_dir().join(base),
    )
}

fn normalizar_blocos_indentados(codigo: &str) -> String {
    let possui_abertura_explicita = codigo.lines().any(|linha| linha.trim().ends_with(": {"));
    let possui_cabecalho_legado = codigo.lines().any(|linha| {
        let conteudo = linha.trim();
        let prefixos = [
            "funcao ",
            "publico funcao ",
            "privado funcao ",
            "protegido estatico funcao ",
            "async funcao ",
            "exportar funcao ",
            "estrutura ",
            "enumeracao ",
            "classe ",
            "quando ",
            "se ",
            "sendo ",
            "senao",
            "escolha ",
            "avaliar ",
            "caso ",
            "cada ",
            "enquanto ",
            "para ",
            "pulso(",
            "durante ",
            "repetir ",
            "critico",
            "privilegio ",
            "montagem ",
            "tente",
            "erro ",
            "sempre",
            "com ",
        ];
        prefixos.iter().any(|prefixo| conteudo.starts_with(prefixo)) && !conteudo.ends_with(": {")
    });
    if possui_abertura_explicita && !possui_cabecalho_legado {
        return codigo.to_owned();
    }
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TipoBloco {
        Completo,
        Condicional,
        Escolha,
        Caso,
    }

    #[derive(Clone, Copy)]
    struct Bloco {
        indentacao: usize,
        tipo: TipoBloco,
        explicito: bool,
    }

    fn indentacao(texto: &str, nivel: usize) -> String {
        format!("{}{}", " ".repeat(nivel), texto)
    }

    fn fechar(saida: &mut Vec<String>, bloco: Bloco) {
        match bloco.tipo {
            TipoBloco::Caso | TipoBloco::Completo | TipoBloco::Condicional | TipoBloco::Escolha => {
                saida.push(indentacao("}", bloco.indentacao));
            }
        }
    }

    let mut saida = Vec::new();
    let mut blocos: Vec<Bloco> = Vec::new();

    for linha_original in codigo.lines() {
        let nivel = linha_original.len() - linha_original.trim_start().len();
        let linha_normalizada = if linha_original.trim().ends_with("(): {") {
            linha_original.to_owned()
        } else {
            linha_original.replace("): ", ") -> ")
        };
        let linha = linha_normalizada.as_str();
        let conteudo = linha.trim();

        if conteudo.is_empty() {
            saida.push(linha.to_owned());
            continue;
        }

        let continuacao = conteudo.starts_with("senao")
            || conteudo.starts_with("sempre")
            || conteudo.starts_with("erro ")
            || conteudo.starts_with("caso ")
            || conteudo == "padrao";
        while blocos
            .last()
            .is_some_and(|bloco| !bloco.explicito && nivel <= bloco.indentacao && !continuacao)
        {
            if let Some(bloco) = blocos.pop() {
                fechar(&mut saida, bloco);
            }
        }

        if conteudo.ends_with("->:") {
            saida.push(indentacao(&format!("{conteudo} {{"), nivel));
            blocos.push(Bloco {
                indentacao: nivel,
                tipo: TipoBloco::Completo,
                explicito: false,
            });
            continue;
        }

        let cabecalho_explicito = [
            "funcao ",
            "publico funcao ",
            "privado funcao ",
            "protegido estatico funcao ",
            "async funcao ",
            "exportar funcao ",
            "estrutura ",
            "enumeracao ",
            "classe ",
            "quando ",
            "se ",
            "sendo ",
            "senao",
            "escolha ",
            "avaliar ",
            "caso ",
            "cada ",
            "enquanto ",
            "para ",
            "pulso(",
            "durante ",
            "repetir ",
            "critico",
            "privilegio ",
            "montagem ",
            "tente",
            "erro ",
            "sempre",
            "com ",
        ]
        .iter()
        .any(|prefixo| conteudo.starts_with(prefixo))
            && conteudo.ends_with(": {");

        if cabecalho_explicito {
            let tipo = if conteudo.starts_with("caso ") {
                TipoBloco::Caso
            } else if conteudo.starts_with("escolha ") || conteudo.starts_with("avaliar ") {
                TipoBloco::Escolha
            } else if conteudo.starts_with("quando ")
                || conteudo.starts_with("se ")
                || conteudo.starts_with("sendo ")
                || conteudo.starts_with("senao")
            {
                TipoBloco::Condicional
            } else {
                TipoBloco::Completo
            };
            saida.push(linha.to_owned());
            blocos.push(Bloco {
                indentacao: nivel,
                tipo,
                explicito: true,
            });
            continue;
        }

        if conteudo == "}" {
            if let Some(bloco) = blocos.last_mut() {
                if bloco.explicito && bloco.indentacao == nivel {
                    blocos.pop();
                    saida.push(linha.to_owned());
                    continue;
                }
            }
        }

        if conteudo.starts_with("senao")
            || conteudo.starts_with("sempre")
            || conteudo.starts_with("erro ")
        {
            if let Some(bloco) = blocos.last() {
                if !bloco.explicito && bloco.indentacao == nivel {
                    let tipo = bloco.tipo;
                    saida.push(indentacao("}", nivel));
                    saida.push(indentacao(&format!("{conteudo}: {{"), nivel));
                    blocos.pop();
                    blocos.push(Bloco {
                        indentacao: nivel,
                        tipo,
                        explicito: false,
                    });
                    continue;
                }
            }
        }

        if conteudo.starts_with("caso ") || conteudo == "padrao" {
            if blocos.last().is_some_and(|bloco| {
                !bloco.explicito && bloco.tipo == TipoBloco::Caso && bloco.indentacao == nivel
            }) {
                if let Some(bloco) = blocos.pop() {
                    fechar(&mut saida, bloco);
                }
            }
            saida.push(indentacao(&format!("{conteudo}: {{"), nivel));
            blocos.push(Bloco {
                indentacao: nivel,
                tipo: TipoBloco::Caso,
                explicito: false,
            });
            continue;
        }

        let (tipo, cabecalho) = if conteudo.starts_with("publico funcao ")
            || conteudo.starts_with("privado funcao ")
            || conteudo.starts_with("protegido estatico funcao ")
            || conteudo.starts_with("funcao ")
            || conteudo.starts_with("async funcao ")
            || conteudo.starts_with("exportar funcao ")
        {
            let assinatura = if conteudo.ends_with("(): {") {
                conteudo.to_owned()
            } else {
                conteudo.replace("): ", ") -> ")
            };
            let assinatura = if assinatura.ends_with(':') {
                format!("{assinatura} {{")
            } else if assinatura.ends_with("{") {
                assinatura
            } else {
                format!("{assinatura}: {{")
            };
            (Some(TipoBloco::Completo), Some(assinatura))
        } else if conteudo.starts_with("estrutura ")
            || conteudo.starts_with("enumeracao ")
            || conteudo.starts_with("classe ")
        {
            let assinatura = if conteudo.ends_with(':') {
                format!("{conteudo} {{")
            } else {
                format!("{conteudo}: {{")
            };
            (Some(TipoBloco::Completo), Some(assinatura))
        } else if conteudo.starts_with("escolha ") || conteudo.starts_with("avaliar ") {
            (Some(TipoBloco::Escolha), Some(format!("{conteudo}: {{")))
        } else if conteudo.starts_with("quando ")
            || conteudo.starts_with("se ")
            || conteudo.starts_with("sendo ")
        {
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
            || conteudo == "seguro"
            || conteudo.starts_with("seguro:")
            || conteudo == "critico"
            || conteudo.starts_with("critico:")
            || conteudo.starts_with("privilegio ")
            || conteudo.starts_with("montagem ")
            || conteudo == "tente"
            || conteudo.starts_with("erro ")
            || conteudo == "sempre"
            || conteudo.starts_with("com ")
        {
            let cabecalho = if conteudo.ends_with(':') {
                format!("{conteudo} {{")
            } else {
                format!("{conteudo}: {{")
            };
            (Some(TipoBloco::Completo), Some(cabecalho))
        } else {
            (None, None)
        };

        if let (Some(tipo), Some(cabecalho)) = (tipo, cabecalho) {
            saida.push(indentacao(&cabecalho, nivel));
            blocos.push(Bloco {
                indentacao: nivel,
                tipo,
                explicito: false,
            });
        } else {
            saida.push(linha.to_owned());
        }
    }

    while let Some(bloco) = blocos.pop() {
        if !bloco.explicito {
            fechar(&mut saida, bloco);
        }
    }
    saida.join("\n")
}

#[test]
fn gera_elf_executavel_com_codigo_de_saida() {
    let resultado = compilar_e_executar(
        "retorno",
        "funcao principal(): inteiro\n    devolver 42",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_sem_compilador_externo_no_path() {
    let (origem, executavel) = caminhos("sem-toolchain-c");
    let codigo = normalizar_blocos_indentados("funcao principal(): inteiro\n    devolver 42");
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
        "funcao principal(): inteiro\n    variavel total: inteiro = 10\n    total = total * 4 + 2\n    devolver total",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_valor_de_funcao_e_chamada_indireta() {
    let resultado = compilar_e_executar(
        "funcao-indireta",
        "funcao somar(valor: inteiro): inteiro\n    devolver valor + 1\nfuncao principal(): inteiro\n    variavel callback: funcao(inteiro): inteiro = somar\n    devolver callback(41)",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_parametros_e_retornos_decimais_no_abi_proprio() {
    let resultado = compilar_e_executar(
        "abi-decimal",
        "funcao ajustar(valor: decimal64): decimal64\n    devolver valor + 1.0\nfuncao principal(): inteiro\n    variavel total = ajustar(41.0)\n    quando total == 42.0\n        devolver 42\n    \n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_campos_decimais_em_estruturas() {
    let resultado = compilar_e_executar(
        "estrutura-decimal",
        "estrutura Medida:\n    valor: decimal64\n    codigo: inteiro\nfuncao principal(): inteiro\n    variavel medida = Medida { valor: 42.0, codigo: 7 }\n    quando medida.valor == 42.0\n        devolver medida.codigo + 35\n    \n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_elementos_decimais_em_tuplas() {
    let resultado = compilar_e_executar(
        "tupla-decimal",
        "funcao principal(): inteiro\n    variavel medida = (42.0, 7)\n    quando medida[0] == 42.0\n        devolver medida[1] + 35\n    \n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_payload_decimal_em_enumeracoes() {
    let resultado = compilar_e_executar(
        "enum-decimal",
        "enumeracao Leitura:\n    Numero(decimal64)\n    Codigo(inteiro)\nfuncao principal(): inteiro\n    variavel leitura = Leitura::Numero(42.0)\n    escolha leitura\n        caso Leitura::Numero(valor)\n            quando valor == 42.0\n                devolver 42\n        caso Leitura::Codigo(valor)\n            passar\n    \n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_funcoes_parametros_e_chamadas() {
    let resultado = compilar_e_executar(
        "funcao",
        "funcao dobrar(valor: inteiro): inteiro\n    devolver valor * 2\nfuncao principal(): inteiro\n    devolver dobrar(21)",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn copia_explicitamente_estrutura_com_campos_copiaveis_no_backend() {
    let resultado = compilar_e_executar(
        "copia-explicita",
        "estrutura Ponto:\n    x: inteiro\n    y: inteiro\nfuncao principal(): inteiro\n    variavel original = Ponto { x: 40, y: 2 }\n    variavel copia = copiar(original)\n    quando original.x + original.y == copia.x + copia.y\n        devolver 42\n    \n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn executa_copia_explicita_de_estrutura_no_backend_nativo() {
    let resultado = compilar_e_executar(
        "copia-estrutura",
        "estrutura Ponto:\n    x: inteiro8\nfuncao principal(): inteiro\n    variavel origem = Ponto { x: 7 }\n    variavel destino = copiar(origem)\n    quando origem.x == destino.x\n        devolver 42\n    \n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn abi_transporta_inteiro128_com_parte_alta_em_parametro_e_retorno() {
    let resultado = compilar_e_executar(
        "abi-inteiro128",
        "funcao transportar(valor: inteiro128): inteiro128\n    devolver valor\nfuncao principal(): inteiro\n    variavel original: inteiro128 = 1267650600228229401496703205376\n    variavel recebido = transportar(original)\n    quando recebido == original\n        devolver 42\n    \n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn abi_transporta_inteiro_u128_com_parte_alta_em_parametro_e_retorno() {
    let resultado = compilar_e_executar(
        "abi-inteiro-u128",
        "funcao transportar(valor: inteiro_u128): inteiro_u128\n    devolver valor\nfuncao principal(): inteiro\n    variavel original: inteiro_u128 = 340282366920938463463374607431768211455\n    variavel recebido = transportar(original)\n    quando recebido == original\n        devolver 42\n    \n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn executa_aritmetica_signed128_acima_de_64_bits() {
    let resultado = compilar_e_executar(
        "aritmetica-signed128",
        "funcao principal(): inteiro\n    variavel base: inteiro128 = 1267650600228229401496703205376\n    variavel dobro = base * 2\n    quando dobro / 2 == base\n        devolver 42\n    \n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn executa_aritmetica_unsigned128_acima_de_64_bits() {
    let resultado = compilar_e_executar(
        "aritmetica-unsigned128",
        "funcao principal(): inteiro\n    variavel base: inteiro_u128 = 18446744073709551619\n    variavel fator: inteiro_u128 = 3\n    variavel produto = base * fator\n    quando produto / fator == base\n        devolver 42\n    \n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn monomorfiza_funcao_generica_no_backend_nativo() {
    let resultado = compilar_e_executar(
        "generica",
        "funcao identidade<T>(valor: T): T\n    devolver valor\nfuncao principal(): inteiro\n    devolver identidade(42)",
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
    devolver 1"#,
    );
    assert_eq!(resultado.status.code(), Some(45));
}

#[test]
fn compila_potencia_inteira_por_quadratura() {
    let resultado = compilar_e_executar(
        "potencia",
        "funcao principal(): inteiro\n    devolver 3 ** 3",
    );
    assert_eq!(resultado.status.code(), Some(27));
}

#[test]
fn potencia_usa_expoente_calculado_em_tempo_de_execucao() {
    let resultado = compilar_e_executar(
        "potencia-dinamica",
        "funcao principal(): inteiro\n    variavel expoente: inteiro = 3\n    devolver 2 ** expoente",
    );
    assert_eq!(resultado.status.code(), Some(8));
}

#[test]
fn expoente_zero_produz_unidade() {
    let resultado = compilar_e_executar(
        "potencia-zero",
        "funcao principal(): inteiro\n    devolver 7 ** 0",
    );
    assert_eq!(resultado.status.code(), Some(1));
}

#[test]
fn divisao_por_zero_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "divisao-zero",
        "funcao principal(): inteiro\n    devolver 42 / 0",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_divisao_signed_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-divisao",
        "funcao principal(): inteiro\n    variavel minimo: inteiro = -9223372036854775807 - 1\n    devolver minimo / -1",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn expoente_acima_de_u32_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "expoente-acima-u32",
        "funcao principal(): inteiro\n    devolver 1 ** 4294967296",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn aceita_expoente_maximo_da_ir() {
    let resultado = compilar_e_executar(
        "expoente-maximo-u32",
        "funcao principal(): inteiro\n    devolver 1 ** 4294967295",
    );
    assert_eq!(resultado.status.code(), Some(1));
}

#[test]
fn overflow_de_resto_signed_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-resto",
        "funcao principal(): inteiro\n    variavel minimo: inteiro = -9223372036854775807 - 1\n    devolver minimo % -1",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn expoente_negativo_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "expoente-negativo",
        "funcao principal(): inteiro\n    devolver 2 ** -1",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_soma_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-soma",
        "funcao principal(): inteiro\n    devolver 9223372036854775807 + 1",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_multiplicacao_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-multiplicacao",
        "funcao principal(): inteiro\n    devolver 3037000500 * 3037000500",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_potencia_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-potencia",
        "funcao principal(): inteiro\n    devolver 2 ** 63",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_subtracao_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-subtracao",
        "funcao principal(): inteiro\n    devolver -9223372036854775807 - 2",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn overflow_de_negacao_termina_com_codigo_de_erro_estavel() {
    let resultado = compilar_e_executar(
        "overflow-negacao",
        "funcao principal(): inteiro\n    variavel minimo: inteiro = -9223372036854775807 - 1\n    devolver -minimo",
    );
    assert_eq!(resultado.status.code(), Some(2));
}

#[test]
fn aceita_literal_minimo_de_inteiro_assinado() {
    let resultado = compilar_e_executar(
        "inteiro-minimo-literal",
        "funcao principal(): inteiro\n    devolver -9223372036854775808",
    );
    assert_eq!(resultado.status.code(), Some(0));
}

#[test]
fn divisao_e_resto_negativos_seguem_semantica_signed() {
    let divisao = compilar_e_executar(
        "divisao-negativa",
        "funcao principal(): inteiro\n    devolver -7 / 2",
    );
    assert_eq!(divisao.status.code(), Some(253));
    let resto = compilar_e_executar(
        "resto-negativo",
        "funcao principal(): inteiro\n    devolver -5 % 2",
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
        devolver 1"#,
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
    quando verdadeiro ou (1 / 0 == 0)
        devolver 42
    devolver 2"#,
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
    devolver i"#,
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
        quando i == 8
            parar
        total = total + i
    devolver total"#,
    );
    assert_eq!(resultado.status.code(), Some(26));
}

#[test]
fn variavel_local_sombreia_global_sem_altera_la() {
    let resultado = compilar_e_executar(
        "global-sombreada",
        "variavel valor: inteiro = 10\nfuncao principal(): inteiro\n    variavel valor: inteiro = 42\n    devolver valor",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn inicializadores_globais_respeitam_ordem_de_declaracao() {
    let resultado = compilar_e_executar(
        "globais-ordem",
        "variavel base: inteiro = 10\nvariavel total: inteiro = base + 5\nfuncao principal(): inteiro\n    devolver total",
    );
    assert_eq!(resultado.status.code(), Some(15));
}

#[test]
fn variaveis_globais_sao_compartilhadas_entre_funcoes() {
    let resultado = compilar_e_executar(
        "globais-compartilhadas",
        "variavel total: inteiro = 40\nfuncao incrementar(): inteiro\n    devolver total + 2\nfuncao principal(): inteiro\n    total = incrementar()\n    devolver total",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_seis_parametros_pelo_abi_system_v() {
    let resultado = compilar_e_executar(
        "seis-parametros",
        "funcao somar(primeiro: inteiro, segundo: inteiro, terceiro: inteiro, quarto: inteiro, quinto: inteiro, sexto: inteiro): inteiro\n    devolver primeiro + segundo + terceiro + quarto + quinto + sexto\nfuncao principal(): inteiro\n    devolver somar(1, 2, 3, 4, 5, 6)",
    );
    assert_eq!(resultado.status.code(), Some(21));
}

#[test]
fn compila_chamadas_aninhadas_sem_perder_alinhamento() {
    let resultado = compilar_e_executar(
        "chamadas-aninhadas",
        "funcao dobrar(valor: inteiro): inteiro\n    devolver valor * 2\nfuncao somar(a: inteiro, b: inteiro): inteiro\n    devolver a + b\nfuncao principal(): inteiro\n    devolver somar(dobrar(2), dobrar(3))",
    );
    assert_eq!(resultado.status.code(), Some(10));
}

#[test]
fn preserva_sombreamento_de_variaveis_em_blocos() {
    let resultado = compilar_e_executar(
        "sombreamento",
        "funcao principal(): inteiro\n    variavel valor: inteiro = 1\n    \n    quando verdadeiro\n        variavel valor: inteiro = 41\n        devolver valor\n    devolver valor",
    );
    assert_eq!(resultado.status.code(), Some(41));
}

#[test]
fn compila_negacao_logica_unaria() {
    let resultado = compilar_e_executar(
        "negacao-logica",
        "funcao principal(): inteiro\n    quando nao falso\n        devolver 42\n    devolver 1",
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
    devolver fibonacci(n - 1) + fibonacci(n - 2)
funcao principal(): inteiro
    devolver fibonacci(10)"#,
    );
    assert_eq!(resultado.status.code(), Some(55));
}

fn compilar_e_executar_com_stdin(
    nome: &str,
    codigo_fonte: &str,
    entrada: &[u8],
) -> std::process::Output {
    let (origem, executavel) = caminhos(nome);
    let codigo = normalizar_blocos_indentados(codigo_fonte);
    fs::write(&origem, codigo).unwrap();
    let compilacao = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&executavel)
        .output()
        .unwrap();
    assert!(
        compilacao.status.success(),
        "falha de compilação: {}",
        String::from_utf8_lossy(&compilacao.stderr)
    );

    let mut processo = Command::new(&executavel)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executa ELF com stdin");
    let mut entrada_padrao = processo.stdin.take().unwrap();
    entrada_padrao.write_all(entrada).unwrap();
    drop(entrada_padrao);
    let resultado = processo.wait_with_output().unwrap();
    let _ = fs::remove_file(origem);
    let _ = fs::remove_file(executavel);
    resultado
}

#[test]
fn ler_texto_le_stdin_e_remove_quebra_de_linha_e_crlf() {
    let (origem, executavel) = caminhos("ler-texto-stdin");
    let codigo = normalizar_blocos_indentados(
        r#"funcao principal(): inteiro
    variavel entrada = ler_texto("Digite: ")
    exibir(entrada)
    quando entrada == "Nebula"
        devolver 42
    devolver 1"#,
    );
    fs::write(&origem, codigo).unwrap();
    let compilacao = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&executavel)
        .output()
        .unwrap();
    assert!(
        compilacao.status.success(),
        "falha de compilação: {}",
        String::from_utf8_lossy(&compilacao.stderr)
    );

    let mut processo = Command::new(&executavel)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executa ELF com stdin");
    let mut entrada_padrao = processo.stdin.take().unwrap();
    entrada_padrao.write_all(b"Nebula\r\n").unwrap();
    drop(entrada_padrao);
    let resultado = processo.wait_with_output().unwrap();
    assert_eq!(
        resultado.status.code(),
        Some(42),
        "stdout: {:?}; stderr: {:?}",
        String::from_utf8_lossy(&resultado.stdout),
        String::from_utf8_lossy(&resultado.stderr)
    );
    assert_eq!(resultado.stdout, b"Digite: Nebula\n");
    let _ = fs::remove_file(origem);
    let _ = fs::remove_file(executavel);
}

#[test]
fn ler_texto_aceita_fim_de_entrada_sem_dados() {
    let (origem, executavel) = caminhos("ler-texto-eof");
    let codigo = normalizar_blocos_indentados(
        "funcao principal(): vazio\n    variavel entrada = ler_texto()",
    );
    fs::write(&origem, codigo).unwrap();
    let compilacao = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .arg("compilar")
        .arg(&origem)
        .arg("-o")
        .arg(&executavel)
        .output()
        .unwrap();
    assert!(
        compilacao.status.success(),
        "falha de compilação: {}",
        String::from_utf8_lossy(&compilacao.stderr)
    );
    let execucao = Command::new(&executavel).output().unwrap();
    assert_eq!(execucao.status.code(), Some(0));
    let _ = fs::remove_file(origem);
    let _ = fs::remove_file(executavel);
}

#[test]
fn ler_texto_aceita_exatamente_4096_bytes() {
    let mut entrada = vec![b'a'; 4096];
    entrada.push(b'\n');
    let resultado = compilar_e_executar_com_stdin(
        "ler-texto-limite-exato",
        "funcao principal(): inteiro\n    variavel entrada = ler_texto()\n    quando comprimento(entrada) == 4096\n        devolver 42\n    devolver 1",
        &entrada,
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn ler_texto_rejeita_entrada_maior_que_4096_bytes() {
    let mut entrada = vec![b'a'; 4097];
    entrada.push(b'\n');
    let resultado = compilar_e_executar_com_stdin(
        "ler-texto-limite-excedido",
        "funcao principal(): vazio\n    variavel entrada = ler_texto()",
        &entrada,
    );
    assert_eq!(resultado.status.code(), Some(1));
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
        "funcao identidade(valor: inteiro): inteiro\n    devolver valor\nfuncao principal(): inteiro\n    devolver 40 + identidade(2)",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn compila_retorno_logico_e_constantes_booleanas() {
    let resultado = compilar_e_executar(
        "retorno-logico",
        "funcao habilitado(): logico\n    devolver verdadeiro\nfuncao principal(): inteiro\n    quando habilitado() e nao falso\n        devolver 42\n    devolver 1",
    );
    assert_eq!(resultado.status.code(), Some(42));
}

#[test]
fn json_valido_literal_funciona_no_backend_nativo() {
    let valido = compilar_e_executar(
        "json-valido-literal",
        r#"funcao principal(): inteiro
    quando json_valido("{\"chave\":1}")
        devolver 42
    devolver 1"#,
    );
    assert_eq!(valido.status.code(), Some(42));

    let invalido = compilar_e_executar(
        "json-invalido-literal",
        r#"funcao principal(): inteiro
    quando nao json_valido("{\"chave\":}")
        devolver 42
    devolver 1"#,
    );
    assert_eq!(invalido.status.code(), Some(42));
}

#[test]
fn json_canonico_literal_funciona_no_backend_nativo() {
    let resultado = compilar_e_executar(
        "json-canonico-literal",
        r#"exibir(json_canonico("{\"b\":2,\"a\":1}"))"#,
    );
    assert!(resultado.status.success());
    assert_eq!(resultado.stdout, b"{\"a\":1,\"b\":2}\n");
}
