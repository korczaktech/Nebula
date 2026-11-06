//! Testes públicos da estrutura e da integração da AST.

use super::{
    AliasTipoAst, CasoEscolha, Expressao, NoAst, OperadorAst, PadraoAst, TipoAst, Visibilidade,
};
use crate::analisar;
use nebula_fonte::ArquivoFonte;

fn normalizar_blocos_legados(codigo: &str) -> String {
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
        etapa: u8,
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

        let cabecalho_explicito = [
            "funcao ",
            "publico funcao ",
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
                etapa: 0,
            });
            continue;
        }

        if conteudo == "fim" {
            if let Some(bloco) = blocos.last_mut() {
                if bloco.explicito && bloco.indentacao == nivel {
                    if bloco.etapa == 0 {
                        bloco.etapa = 1;
                    } else if bloco.etapa == 2 {
                        blocos.pop();
                    }
                    saida.push(linha.to_owned());
                    continue;
                }
            }

            while blocos.last().is_some_and(|bloco| {
                !bloco.explicito && bloco.tipo == TipoBloco::Caso && bloco.indentacao > nivel
            }) {
                if let Some(bloco) = blocos.pop() {
                    fechar(&mut saida, bloco);
                }
            }

            if blocos
                .last()
                .is_some_and(|bloco| !bloco.explicito && bloco.indentacao == nivel)
            {
                if let Some(bloco) = blocos.pop() {
                    fechar(&mut saida, bloco);
                }
            } else {
                saida.push(linha.to_owned());
            }
            continue;
        }

        if conteudo == "}" {
            if let Some(bloco) = blocos.last_mut() {
                if bloco.explicito && bloco.indentacao == nivel && bloco.etapa == 1 {
                    if bloco.tipo == TipoBloco::Caso {
                        blocos.pop();
                    } else {
                        bloco.etapa = 2;
                    }
                    saida.push(linha.to_owned());
                    continue;
                }
            }
        }

        if conteudo.starts_with("senao") {
            if let Some(bloco) = blocos.last() {
                if !bloco.explicito
                    && bloco.tipo == TipoBloco::Condicional
                    && bloco.indentacao == nivel
                {
                    saida.push(indentacao("fim", nivel));
                    saida.push(indentacao("}", nivel));
                    saida.push(indentacao("senao: {", nivel));
                    blocos.pop();
                    blocos.push(Bloco {
                        indentacao: nivel,
                        tipo: TipoBloco::Condicional,
                        explicito: false,
                        etapa: 0,
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
                etapa: 0,
            });
            continue;
        }

        let (tipo, cabecalho) = if conteudo.starts_with("publico funcao ")
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
                explicito: false,
                etapa: 0,
            });
        } else {
            saida.push(linha.to_owned());
        }
    }

    saida.join("\n")
}

#[test]
fn tipo_generico_aninhado_preserva_estrutura_e_grafia() {
    let tipo = TipoAst::analisar("Resultado<[texto], Tupla<inteiro, logico>>");
    assert_eq!(
        tipo.para_texto(),
        "Resultado<[texto], Tupla<inteiro, logico>>"
    );
    assert!(matches!(
        tipo,
        TipoAst::Generico { argumentos, .. } if argumentos.len() == 2
    ));
}

#[test]
fn tipo_de_funcao_lista_tupla_e_caminho_sao_estruturados() {
    assert!(matches!(
        TipoAst::analisar("funcao(inteiro, texto):logico"),
        TipoAst::Funcao { parametros, retorno }
            if parametros.len() == 2 && *retorno == TipoAst::Nome("logico".into())
    ));
    assert_eq!(
        TipoAst::analisar("[inteiro]"),
        TipoAst::Lista(Box::new(TipoAst::Nome("inteiro".into())))
    );
    assert_eq!(
        TipoAst::analisar("Modulo::Pessoa"),
        TipoAst::Caminho(vec!["Modulo".into(), "Pessoa".into()])
    );
    assert_eq!(
        TipoAst::analisar("(inteiro, texto)"),
        TipoAst::Tupla(vec![
            TipoAst::Nome("inteiro".into()),
            TipoAst::Nome("texto".into())
        ])
    );
}

#[test]
fn declaracoes_expoem_acesso_tipado_sem_perder_api_do_parser() {
    let variavel = NoAst::Variavel {
        nome: "itens".into(),
        tipo: Some("Vetor<Resultado<inteiro, texto>>".into()),
        valor: Expressao::Lista(vec![]),
        constante: false,
    };
    assert_eq!(
        variavel.tipo_variavel(),
        Some(TipoAst::Generico {
            nome: "Vetor".into(),
            argumentos: vec![TipoAst::Generico {
                nome: "Resultado".into(),
                argumentos: vec![
                    TipoAst::Nome("inteiro".into()),
                    TipoAst::Nome("texto".into())
                ]
            }]
        })
    );

    let alias = NoAst::Tipo {
        nome: "Identificador".into(),
        destino: "Modulo::Pessoa".into(),
    };
    assert_eq!(
        alias.destino_alias(),
        Some(AliasTipoAst {
            nome: "Identificador".into(),
            destino: TipoAst::Caminho(vec!["Modulo".into(), "Pessoa".into()])
        })
    );

    let funcao = NoAst::Funcao {
        visibilidade: Some(Visibilidade::Publico),
        assincrona: false,
        estatica: false,
        nome: "converter".into(),
        genericos: vec![],
        parametros: vec![("valor".into(), "[inteiro]".into())],
        parametros_padrao: vec![None],
        retorno: Some("texto".into()),
        corpo: vec![],
    };
    assert_eq!(
        funcao.parametros_tipados().expect("parâmetros tipados")[0].tipo,
        TipoAst::Lista(Box::new(TipoAst::Nome("inteiro".into())))
    );
}

#[test]
fn padrao_e_operadores_possuem_representacao_propria() {
    let caso = CasoEscolha {
        enumeracao: Some("Estado".into()),
        variante: Some("Falha".into()),
        vinculacoes: vec!["codigo".into()],
        expressao: None,
        corpo: vec![],
    };
    assert_eq!(
        caso.padrao(),
        PadraoAst::Variante {
            caminho: vec!["Estado".into(), "Falha".into()],
            vinculacoes: vec![PadraoAst::Vinculacao("codigo".into())]
        }
    );
    assert_eq!(
        OperadorAst::try_from(crate::TipoToken::Mais),
        Ok(OperadorAst::Adicao)
    );
    assert_eq!(
        OperadorAst::try_from(crate::TipoToken::Reticencias),
        Ok(OperadorAst::Espalhamento)
    );
    assert!(OperadorAst::try_from(crate::TipoToken::Identificador).is_err());
}

#[test]
fn parser_registra_intervalos_de_nos_aninhados() {
    let codigo = normalizar_blocos_legados(
        "funcao principal()\n    variavel nome: texto = \"Nebula\"\n    quando verdadeiro\n       devolver nome\n    fim\nfim",
    );
    let resultado = analisar(&ArquivoFonte::novo("ast.nbx", &codigo));
    assert!(
        resultado.diagnosticos.is_empty(),
        "programa de teste deve ser válido: {:#?}",
        resultado.diagnosticos
    );
    assert!(
        resultado
            .programa
            .itens
            .iter()
            .any(|no| matches!(no, NoAst::Funcao { .. }))
    );
    assert!(
        resultado
            .programa
            .iter_localizados()
            .any(|localizado| matches!(&localizado.no, NoAst::Variavel { .. })),
        "variável aninhada precisa de localização própria"
    );
    assert!(
        resultado
            .programa
            .iter_localizados()
            .any(|localizado| matches!(&localizado.no, NoAst::Devolver(_))),
        "retorno aninhado precisa de localização própria"
    );
    assert!(
        resultado
            .programa
            .iter_expressoes_localizadas()
            .any(|localizado| matches!(&localizado.expressao, Expressao::Nome(_))),
        "expressões precisam de localização própria"
    );
    assert!(
        resultado
            .programa
            .iter_tipos_localizados()
            .any(|localizado| localizado.tipo == TipoAst::Nome("texto".into())),
        "tipos precisam de localização própria"
    );

    for localizado in resultado.programa.iter_localizados() {
        assert!(localizado.intervalo.inicio <= localizado.intervalo.fim);
        assert!(localizado.intervalo.fim <= codigo.len());
        assert!(codigo.is_char_boundary(localizado.intervalo.inicio));
        assert!(codigo.is_char_boundary(localizado.intervalo.fim));
    }
    for localizado in resultado.programa.iter_expressoes_localizadas() {
        assert!(localizado.intervalo.fim <= codigo.len());
        assert!(codigo.is_char_boundary(localizado.intervalo.inicio));
        assert!(codigo.is_char_boundary(localizado.intervalo.fim));
    }
    for localizado in resultado.programa.iter_tipos_localizados() {
        assert!(localizado.intervalo.fim <= codigo.len());
        assert!(codigo.is_char_boundary(localizado.intervalo.inicio));
        assert!(codigo.is_char_boundary(localizado.intervalo.fim));
    }
}

#[test]
fn parser_produz_todos_os_nos_declarativos_e_de_controle() {
    let codigo = r#"
variavel valor = 1
constante fixa = 2
tipo Identificador = inteiro
estrutura Pessoa:
    nome: texto
fim
enumeracao Estado:
    Pronto
    Falha(texto)
fim
incluir(modulo: Texto, biblioteca: Modulo) como Texto
funcao executar(item: inteiro): vazio
    quando verdadeiro
        exibir(item)
    senao
        passar
    fim
    escolha Estado::Pronto
        caso Estado::Pronto
            passar
        caso _
            passar
    fim
    enquanto falso
        passar
    fim
    cada elemento em [1, 2]
        passar
    fim
    pulso(10)
        passar
    fim
    para (;;)
        passar
    fim
    devolver
    parar
    seguir
fim

executar(1)
"#;
    let resultado = analisar(&ArquivoFonte::novo(
        "nos.nbx",
        normalizar_blocos_legados(codigo),
    ));
    assert!(
        resultado.diagnosticos.is_empty(),
        "programa com todos os nós deve ser válido: {:#?}",
        resultado.diagnosticos
    );

    let nos: Vec<_> = resultado
        .programa
        .iter_localizados()
        .map(|localizado| &localizado.no)
        .collect();
    assert!(nos.iter().any(|no| matches!(no, NoAst::Variavel { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Funcao { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Tipo { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Estrutura { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Enumeracao { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Incluir { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Quando { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Escolha { .. })));
    assert!(
        resultado.programa.iter_casos_localizados().next().is_some(),
        "cada caso de escolha precisa de localização própria"
    );
    for localizado in resultado.programa.iter_casos_localizados() {
        assert!(localizado.intervalo.inicio <= localizado.intervalo.fim);
        assert!(localizado.intervalo.fim <= codigo.len());
        assert!(codigo.is_char_boundary(localizado.intervalo.inicio));
        assert!(codigo.is_char_boundary(localizado.intervalo.fim));
    }
    assert!(nos.iter().any(|no| matches!(no, NoAst::Enquanto { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Cada { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Pulso { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Para { .. })));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Devolver(_))));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Parar)));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Seguir)));
    assert!(nos.iter().any(|no| matches!(no, NoAst::Expressao(_))));
}

#[test]
fn parser_produz_todas_as_formas_de_expressao() {
    let codigo = r#"
variavel unaria = -1
variavel binaria = 1 + 2
variavel atribuicao = destino = 2
variavel chamada = executar(1)
variavel indexada = itens[0]
variavel membro = pessoa.nome
variavel lista = [1, 2]
variavel tupla = (1, 2)
variavel construcao = Pessoa { nome: "Ana", fim }
"#;
    let resultado = analisar(&ArquivoFonte::novo(
        "expressoes.nbx",
        normalizar_blocos_legados(codigo),
    ));
    assert!(
        resultado.diagnosticos.is_empty(),
        "expressões de referência devem ser válidas: {:#?}",
        resultado.diagnosticos
    );

    let expressoes: Vec<_> = resultado
        .programa
        .iter_expressoes_localizadas()
        .map(|localizado| &localizado.expressao)
        .collect();
    assert!(
        expressoes
            .iter()
            .any(|e| matches!(e, Expressao::Unaria { .. }))
    );
    assert!(
        expressoes
            .iter()
            .any(|e| matches!(e, Expressao::Binaria { .. }))
    );
    assert!(
        expressoes
            .iter()
            .any(|e| matches!(e, Expressao::Atribuicao { .. }))
    );
    assert!(
        expressoes
            .iter()
            .any(|e| matches!(e, Expressao::Chamada { .. }))
    );
    assert!(
        expressoes
            .iter()
            .any(|e| matches!(e, Expressao::Indexacao { .. }))
    );
    assert!(
        expressoes
            .iter()
            .any(|e| matches!(e, Expressao::Membro { .. }))
    );
    assert!(expressoes.iter().any(|e| matches!(e, Expressao::Lista(_))));
    assert!(expressoes.iter().any(|e| matches!(e, Expressao::Tupla(_))));
    assert!(
        expressoes
            .iter()
            .any(|e| matches!(e, Expressao::ConstrucaoEstrutura { .. }))
    );
}

#[test]
fn recuperacao_preserva_hierarquia_e_localizacoes_da_ast_aninhada() {
    let codigo = normalizar_blocos_legados(
        "funcao principal()\n    quando verdadeiro\n        variavel inicio = 1\n        enquanto falso\n            variavel lista = [1, , 3]\n            variavel preservada = 2\n        fim\n        variavel depois = 3\n    senao\n        variavel alternativa = 4\n    fim\n    variavel final = 5\nfim",
    );
    let resultado = analisar(&ArquivoFonte::novo("recuperacao.nbx", &codigo));

    assert!(
        !resultado.diagnosticos.is_empty(),
        "a expressão malformada precisa produzir diagnóstico"
    );
    let Some(NoAst::Funcao { nome, corpo, .. }) = resultado.programa.itens.first() else {
        panic!(
            "a função externa precisa permanecer na AST parcial: {:?}",
            resultado.programa.itens
        );
    };
    assert_eq!(nome, "principal");

    let Some(NoAst::Quando { entao, senao, .. }) =
        corpo.iter().find(|no| matches!(no, NoAst::Quando { .. }))
    else {
        panic!("o bloco condicional externo precisa permanecer íntegro: {corpo:?}");
    };
    assert!(
        entao
            .iter()
            .any(|no| matches!(no, NoAst::Variavel { nome, .. } if nome == "inicio"))
    );
    assert!(
        entao
            .iter()
            .any(|no| matches!(no, NoAst::Variavel { nome, .. } if nome == "depois"))
    );
    assert!(
        senao
            .iter()
            .any(|no| matches!(no, NoAst::Variavel { nome, .. } if nome == "alternativa"))
    );

    let Some(NoAst::Enquanto {
        corpo: corpo_laco, ..
    }) = entao.iter().find(|no| matches!(no, NoAst::Enquanto { .. }))
    else {
        panic!("o laço interno precisa permanecer na AST: {entao:?}");
    };
    assert!(
        corpo_laco
            .iter()
            .any(|no| matches!(no, NoAst::Variavel { nome, .. } if nome == "lista"))
    );
    assert!(
        corpo_laco
            .iter()
            .any(|no| matches!(no, NoAst::Variavel { nome, .. } if nome == "preservada"))
    );
    assert!(
        corpo
            .iter()
            .any(|no| matches!(no, NoAst::Variavel { nome, .. } if nome == "final"))
    );

    for nome in [
        "inicio",
        "lista",
        "preservada",
        "depois",
        "alternativa",
        "final",
    ] {
        assert!(
            resultado.programa.iter_localizados().any(
                |localizado| matches!(&localizado.no, NoAst::Variavel { nome: encontrado, .. } if encontrado == nome)
            ),
            "a localização do nó '{nome}' foi perdida durante a recuperação"
        );
    }
    for tipo in ["funcao", "quando", "enquanto"] {
        let encontrado = resultado.programa.iter_localizados().any(|localizado| {
            let trecho = &codigo[localizado.intervalo.inicio..localizado.intervalo.fim];
            trecho.starts_with(tipo)
        });
        assert!(
            encontrado,
            "a localização do bloco '{tipo}' precisa ser preservada"
        );
    }

    for localizado in resultado.programa.iter_localizados() {
        assert!(localizado.intervalo.inicio <= localizado.intervalo.fim);
        assert!(localizado.intervalo.fim <= codigo.len());
        assert!(codigo.is_char_boundary(localizado.intervalo.inicio));
        assert!(codigo.is_char_boundary(localizado.intervalo.fim));
    }
    for localizado in resultado.programa.iter_expressoes_localizadas() {
        assert!(localizado.intervalo.inicio <= localizado.intervalo.fim);
        assert!(localizado.intervalo.fim <= codigo.len());
        assert!(codigo.is_char_boundary(localizado.intervalo.inicio));
        assert!(codigo.is_char_boundary(localizado.intervalo.fim));
    }
}

#[test]
fn recuperacao_aninhada_e_deterministica_e_nao_contamina_analise_seguinte() {
    let codigo = "funcao principal()\n    variavel quebrada = Pessoa { dados: calcular([1, , 2], 3), outro: 4, fim }\n    quando verdadeiro\n        variavel preservada = 7\n    fim\nfim\nvariavel posterior = 9";
    let codigo = normalizar_blocos_legados(codigo);
    let fonte = ArquivoFonte::novo("determinismo.nbx", &codigo);
    let primeira = analisar(&fonte);
    let segunda = analisar(&fonte);

    assert_eq!(primeira, segunda, "a recuperação deve ser determinística");
    assert!(!primeira.diagnosticos.is_empty());
    assert!(
        matches!(
            primeira.programa.itens.first(),
            Some(NoAst::Funcao { corpo, .. })
                if corpo.iter().any(|no| matches!(no, NoAst::Quando { entao, .. }
                    if entao.iter().any(|interno| matches!(interno, NoAst::Variavel { nome, .. } if nome == "preservada"))))
        ),
        "a hierarquia interna válida precisa sobreviver ao erro: {:?}",
        primeira.programa.itens
    );
    assert!(
        primeira
            .programa
            .itens
            .iter()
            .any(|no| matches!(no, NoAst::Variavel { nome, .. } if nome == "posterior")),
        "o erro aninhado não pode contaminar a declaração posterior de nível superior"
    );
}

#[test]
fn tipos_genericos_preservam_argumentos_constantes_e_tipos_de_funcao_sem_retorno() {
    assert_eq!(
        TipoAst::analisar("Vetor<inteiro, 4096>"),
        TipoAst::Generico {
            nome: "Vetor".into(),
            argumentos: vec![
                TipoAst::Nome("inteiro".into()),
                TipoAst::Constante("4096".into())
            ]
        }
    );
    assert_eq!(
        TipoAst::analisar("funcao(inteiro)"),
        TipoAst::Funcao {
            parametros: vec![TipoAst::Nome("inteiro".into())],
            retorno: Box::new(TipoAst::Nome("vazio".into()))
        }
    );
    for literal in ["0xFF", "0b1010", "0o755", "1_024"] {
        assert_eq!(
            TipoAst::analisar(&format!("Vetor<inteiro, {literal}>")),
            TipoAst::Generico {
                nome: "Vetor".into(),
                argumentos: vec![
                    TipoAst::Nome("inteiro".into()),
                    TipoAst::Constante(literal.into())
                ]
            },
            "literal constante {literal} deve manter sua base e grafia"
        );
    }
}

#[test]
fn operadores_ast_cobrem_operadores_aritmeticos_logicos_bitwise_e_compostos() {
    use crate::TipoToken as T;
    let pares = [
        (T::DivideInteira, OperadorAst::DivisaoInteira),
        (T::SomaAtribui, OperadorAst::SomaAtribuicao),
        (
            T::DivideInteiraAtribui,
            OperadorAst::DivisaoInteiraAtribuicao,
        ),
        (T::CoalesceAtribui, OperadorAst::CoalesceAtribuicao),
        (T::EBit, OperadorAst::EBit),
        (T::OuBit, OperadorAst::OuBit),
        (T::XorBit, OperadorAst::XorBit),
        (T::NaoBit, OperadorAst::NaoBit),
        (T::DeslocaEsquerda, OperadorAst::DeslocamentoEsquerda),
        (T::DeslocaDireita, OperadorAst::DeslocamentoDireita),
        (
            T::DeslocaDireitaSemSinal,
            OperadorAst::DeslocamentoDireitaSemSinal,
        ),
        (T::AcessoOpcional, OperadorAst::AcessoOpcional),
        (T::DiferenteEstrito, OperadorAst::DiferenteEstrito),
        (T::Dentro, OperadorAst::Dentro),
        (T::Fora, OperadorAst::Fora),
        (T::TipoDe, OperadorAst::TipoDe),
        (T::InstanciaDe, OperadorAst::InstanciaDe),
    ];
    for (token, esperado) in pares {
        assert_eq!(
            OperadorAst::try_from(token),
            Ok(esperado),
            "token {token:?}"
        );
    }
}

#[test]
fn padrao_preserva_expressao_complexa_sem_serializar_debug() {
    let expressao = Expressao::Binaria {
        esquerda: Box::new(Expressao::Nome("valor".into())),
        operador: crate::TipoToken::Mais,
        direita: Box::new(Expressao::Literal("1".into())),
    };
    let caso = CasoEscolha {
        enumeracao: None,
        variante: None,
        vinculacoes: vec![],
        expressao: Some(expressao.clone()),
        corpo: vec![],
    };
    assert_eq!(caso.padrao(), PadraoAst::Expressao(expressao));
}
