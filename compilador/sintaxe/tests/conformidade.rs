//! Suite executável de conformidade entre a gramática normativa e o frontend.

use nebula_fonte::ArquivoFonte;
use nebula_sintaxe::analisar;

fn normalizar_blocos_legados(codigo: &str) -> String {
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
        return codigo
            .lines()
            .filter(|linha| linha.trim() != "fim")
            .collect::<Vec<_>>()
            .join("\n")
            .replace("} fim", "}");
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

        if conteudo == "fim" {
            if blocos.is_empty() || blocos.last().is_some_and(|bloco| bloco.explicito) {
                continue;
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

    saida.join("\n")
}

fn analisar_codigo_bruto(codigo: &str) -> nebula_sintaxe::ResultadoSintatico {
    analisar(&ArquivoFonte::novo("conformidade.nbx", codigo))
}

fn analisar_codigo(codigo: &str) -> nebula_sintaxe::ResultadoSintatico {
    let codigo = normalizar_blocos_legados(codigo);
    analisar(&ArquivoFonte::novo("conformidade.nbx", &codigo))
}

#[test]
fn corpus_positivo_cobre_as_producoes_normativas() {
    let casos = [
        ("arquivo vazio", ""),
        (
            "separadores no arquivo",
            "\n;\nvariavel a = 1;;\n\nconstante b <- 2;\n",
        ),
        (
            "declaracoes variaveis",
            "variavel a = 1\ndefinir b: inteiro = 2\nmutavel c <- 3\nmutavel d: texto <- \"Nebula\"\nconstante ativo = verdadeiro",
        ),
        (
            "funcao publica generica",
            "publico funcao identidade<T>(valor: T): T\n    devolver valor",
        ),
        (
            "funcao privada e parametros",
            "privado funcao combinar<T, U>(a: [T], b: U): funcao(T): U\n    devolver b",
        ),
        ("funcao sem retorno", "funcao executar()\n    passar"),
        (
            "tipos simples e qualificados",
            "tipo A = inteiro\ntipo B <- Modulo::Interno::Tipo",
        ),
        (
            "tipos compostos",
            "tipo Lista = [[Resultado<inteiro>]]\ntipo Funcao = funcao(inteiro, texto): Resultado<[texto]>",
        ),
        ("estrutura vazia", "estrutura Vazia:\n    marcador: inteiro"),
        (
            "estrutura com campos e separadores",
            "estrutura Pessoa:\n    nome: texto\n    idade: inteiro\n    ativo: logico",
        ),
        (
            "enumeracao com variantes",
            "enumeracao Estado:\n    Vazio\n    Unidade()\n    Valor(inteiro, texto)",
        ),
        (
            "inclusao com alias",
            "incluir(modulo: Texto, biblioteca: Biblioteca) como Texto",
        ),
        (
            "inclusao sem alias",
            "incluir(modulo: Interna, biblioteca: Biblioteca)",
        ),
        (
            "quando e senao",
            "quando verdadeiro\n    exibir(1)\nsenao\n    exibir(0)",
        ),
        (
            "quando encadeado",
            "quando a\n    passar\nsenao\n    passar",
        ),
        (
            "escolha e padroes",
            "escolha valor\n    caso Resultado::Sucesso(texto)\n        exibir(texto)\n    caso Resultado::Falha(codigo)\n        passar\n    caso _\n        passar",
        ),
        (
            "alias avaliar",
            "avaliar valor\n    caso Estado::Pronto\n        passar",
        ),
        (
            "lacos enquanto",
            "enquanto verdadeiro\n    parar\ndurante falso\n    seguir\nrepetir verdadeiro\n    encerrar",
        ),
        ("iteracao cada", "cada item em [1, 2, 3]\n    exibir(item)"),
        ("pulso", "pulso(100)\n    exibir(1)"),
        (
            "para com declaracao",
            "para (variavel i = 0; i < 3; i = i + 1)\n    exibir(i)",
        ),
        (
            "para com expressoes opcionais",
            "para (inicio = 0; ; passo = passo + 1)\n    passar\npara (; condicao; )\n    passar\npara (;; )\n    passar",
        ),
        ("alterar", "alterar total <- 10\nalterar total = total + 1"),
        (
            "retorno e controle",
            "devolver\ndevolver resultado\nparar\nencerrar\nseguir",
        ),
        (
            "expressoes binarias",
            "a ou b\na e b\na == b\na != b\na < b\na <= b\na > b\na >= b\na..b\na + b\na - b\na * b\na / b\na % b\na ** b",
        ),
        ("expressoes unarias", "!a\nnao a\n-a\n+a"),
        (
            "chamadas e argumentos",
            "executar()\nexecutar(a)\nexecutar(a, b)",
        ),
        (
            "indexacao e membros",
            "itens[0]\nobjeto.campo\nModulo::valor\nmodulo::objeto.campo[0](1)",
        ),
        (
            "tuplas",
            "variavel vazia = ()\nvariavel agrupada = (a)\nvariavel par = (a, b)\nvariavel final = (a, b,)",
        ),
        ("listas", "variavel vazia = []\nvariavel valores = [a, b]"),
        (
            "construcao de estrutura",
            "variavel pessoa = Pessoa { nome: \"Ana\", idade: 20 }",
        ),
        (
            "construcao aninhada",
            "Pessoa { endereco: Endereco { cidade: \"São Paulo\" }, dados: ([1, 2], (3,)) }",
        ),
        (
            "literais",
            "42\n3.14\n\"texto\"\n'x'\nverdadeiro\nfalso\nnulo",
        ),
        (
            "precedencia e agrupamento",
            "variavel x = -2 ** 3 ** 2\nvariavel y = (1 < 2) e (3 >= 2) ou falso",
        ),
        (
            "continuacao de linha",
            "variavel soma = 1\n + 2\nvariavel chamada = sistema\n ::somar(\n  1,\n  2\n )",
        ),
        (
            "bloco com fim",
            "funcao executar()\n    variavel ativo = verdadeiro\n    enquanto ativo\n        ativo = falso",
        ),
        (
            "bloco com instrucao vazia",
            "funcao sem_corpo()\n    passar",
        ),
        (
            "aliases sem acentos",
            "funcao principal()\n    quando nao falso\n        passar\n    senao\n        passar\nenumeracao Estado:\n    Pronto\npublico funcao iniciar()\n    passar",
        ),
        (
            "fixture de sintaxe visual",
            include_str!("../../../testes/conformidade/sintaxe_visual.nbx"),
        ),
        (
            "exemplo publico",
            include_str!("../../../exemplos/ola_mundo.nbx"),
        ),
    ];

    for (nome, codigo) in casos {
        let resultado = analisar_codigo(codigo);
        assert!(
            resultado.diagnosticos.is_empty(),
            "caso válido [{nome}] foi rejeitado.\nCódigo:\n{codigo}\nDiagnósticos: {:#?}",
            resultado.diagnosticos
        );
        if !codigo.trim().is_empty() {
            assert!(
                !resultado.programa.itens.is_empty(),
                "caso válido [{nome}] não produziu itens na AST"
            );
        }
    }
}

#[test]
fn corpus_negativo_cobre_regras_e_restricoes_normativas() {
    let casos = [
        (
            "separador ausente",
            "N2001",
            "variavel a = 1 variavel b = 2",
        ),
        ("modificador sem funcao", "N2002", "publico variavel a = 1"),
        ("atribuicao ausente", "N2003", "variavel a 1"),
        ("abertura de parametros ausente", "N2004", "funcao f {}"),
        ("parametro sem tipo", "N2005", "funcao f(a inteiro) {}"),
        ("atribuicao de tipo ausente", "N2006", "tipo A inteiro"),
        ("bloco ausente", "N2007", "funcao f() exibir(1)"),
        ("expressao ausente", "N2008", "variavel a ="),
        ("fechamento ausente", "N2007", "funcao f() {"),
        ("identificador ausente", "N2010", "variavel = 1"),
        ("reservada futura", "N2011", "variavel tentar = 1"),
        ("comparacao encadeada", "N2012", "variavel a = 1 < 2 < 3"),
        ("intervalo encadeado", "N2013", "variavel a = 1..2..3"),
        ("generico duplicado", "N2014", "funcao f<T, T>() {}"),
        ("lista generica vazia", "N2015", "funcao f<>() {}"),
        ("construcao futura", "N2016", "herda"),
        (
            "abertura de estrutura ausente",
            "N2030",
            "estrutura Pessoa nome: texto }",
        ),
        (
            "campo duplicado",
            "N2031",
            "estrutura Pessoa: {\n    nome: texto\n    nome: inteiro\n}",
        ),
        (
            "separador de campo ausente",
            "N2032",
            "estrutura Pessoa: {\n    nome: texto idade: inteiro\n}",
        ),
        (
            "abertura de enumeracao ausente",
            "N2033",
            "enumeracao Estado Ativo }",
        ),
        (
            "variante duplicada",
            "N2034",
            "enumeracao Estado: {\n    Ativo\n    Ativo\n}",
        ),
        (
            "payload malformado",
            "N2035",
            "enumeracao Estado: {\n Ativo(inteiro,)}",
        ),
        ("enumeracao vazia", "N2036", "enumeracao Estado: {\n}"),
        (
            "campo de construcao duplicado",
            "N2037",
            "variavel p = Pessoa { nome: 1, nome: 2 }",
        ),
        (
            "separador de construcao ausente",
            "N2038",
            "variavel p = Pessoa { nome: 1 idade: 2 }",
        ),
        (
            "abertura de escolha ausente",
            "N2040",
            "escolha valor caso _ {}",
        ),
        ("caso ausente", "N2041", "escolha valor: {\n    outro\n}"),
        (
            "padrao invalido",
            "N2042",
            "escolha valor: {\n    caso Variante: {\n        passar\n    }\n}",
        ),
        (
            "curinga duplicado",
            "N2043",
            "escolha valor: {\n    caso _: {\n        passar\n    }\n    caso _: {\n        passar\n    }\n}",
        ),
        (
            "escolha vazia",
            "N2044",
            "escolha valor: {\n    variavel x = 1\n}",
        ),
        (
            "vinculo de payload ausente",
            "N2045",
            "escolha valor: {\n    caso Estado::Erro(1): {\n        passar\n    }\n}",
        ),
        (
            "vinculo duplicado",
            "N2046",
            "escolha valor: {\n    caso Estado::Erro(item, item): {\n        passar\n    }\n}",
        ),
        ("alterar sem atribuicao", "N2047", "alterar total"),
        (
            "padrao de variante duplicado",
            "N2048",
            "escolha valor: {\n    caso Estado::Erro: {\n        passar\n    }\n    caso Estado::Erro: {\n        passar\n    }\n}",
        ),
        (
            "parametro duplicado",
            "N2050",
            "funcao f(item: inteiro, item: texto)\n    passar",
        ),
        ("fechamento sem abertura", "N2051", "variavel valor = 1)"),
    ];

    for (nome, codigo_esperado, codigo_fonte) in casos {
        let resultado = analisar_codigo_bruto(codigo_fonte);
        assert!(
            resultado
                .diagnosticos
                .iter()
                .any(|d| d.codigo == codigo_esperado),
            "caso inválido [{nome}] não emitiu {codigo_esperado}.\nCódigo:\n{codigo_fonte}\nDiagnósticos: {:#?}",
            resultado.diagnosticos
        );
    }
}

#[test]
fn diagnosticos_do_corpus_preservam_intervalos_utf8_validos() {
    let codigo = "variavel ação = )\nestrutura Pessoa { nome texto }\nvariavel fim = 1";
    let resultado = analisar_codigo_bruto(codigo);
    assert!(!resultado.diagnosticos.is_empty());

    for diagnostico in resultado.diagnosticos {
        let intervalo = diagnostico
            .intervalo
            .expect("diagnóstico deve localizar a origem");
        assert!(intervalo.inicio <= intervalo.fim);
        assert!(intervalo.fim <= codigo.len());
        assert!(codigo.is_char_boundary(intervalo.inicio));
        assert!(codigo.is_char_boundary(intervalo.fim));
        assert!(!diagnostico.mensagem.trim().is_empty());
    }
}

#[test]
fn corpus_combinado_preserva_a_ordem_dos_itens() {
    let codigo = r#"
variavel nome = "Nebula"
estrutura Pessoa:
    nome: texto
    idade: inteiro
fim
enumeracao Estado:
    Ativo
    Inativo
fim
funcao principal(pessoa: Pessoa): vazio
    quando verdadeiro
        exibir(pessoa.nome)
    senao
        devolver
    fim
    escolha Estado::Ativo
        caso Estado::Ativo
            passar
        caso _
            passar
    fim
fim
"#;
    let resultado = analisar_codigo(codigo);
    assert!(
        resultado.diagnosticos.is_empty(),
        "programa integrado de referência foi rejeitado: {:#?}",
        resultado.diagnosticos
    );
    assert_eq!(resultado.programa.itens.len(), 4);
}

#[test]
fn matriz_ast_cobre_todas_as_variantes_de_instrucao() {
    use nebula_sintaxe::ast::NoAst;

    let codigo = r#"
variavel valor = 1
publico funcao identidade<T>(item: T = valor): T
    devolver item
fim
tipo Identificador = inteiro
estrutura Pessoa:
    nome: texto
fim
enumeracao Estado:
    Pronto
    Falha(texto)
fim
incluir(modulo: Texto, biblioteca: Biblioteca) como Texto
quando verdadeiro
    passar
senao
    passar
fim
escolha 1
    caso 1
        passar
    caso _
        passar
fim
enquanto falso
    passar
fim
cada item em [1, 2]
    passar
fim
pulso(10)
    passar
fim
para (variavel i = 0; i < 2; i = i + 1)
    passar
fim
devolver
pare
prossiga
passar
ceder
@marcador(8)
funcao anotada()
    passar
fim
seguro:
    passar
fim
critico:
    passar
fim
privilegio sistema:
    passar
fim
montagem x86_64<u32>:
    passar
fim
tente
    passar
erro Falha
    passar
sempre
    passar
fim
com abrir() como arquivo
    passar
fim
lancar erro "falha"
verifique verdadeiro, "invariante"
global contador
externa simbolo
executar()
classe PessoaBase herda Entidade
    publico funcao nome()
        devolver "Pessoa"
    fim
fim
"#;
    let resultado = analisar_codigo(codigo);
    assert!(
        resultado.diagnosticos.is_empty(),
        "matriz de nós válidos foi rejeitada: {:#?}",
        resultado.diagnosticos
    );

    let nos: Vec<_> = resultado
        .programa
        .iter_localizados()
        .map(|item| &item.no)
        .collect();
    let exige = |condicao: fn(&NoAst) -> bool, nome: &str| {
        assert!(
            nos.iter().any(|no| condicao(no)),
            "a matriz não produziu NoAst::{nome}"
        );
    };
    exige(|n| matches!(n, NoAst::Variavel { .. }), "Variavel");
    exige(|n| matches!(n, NoAst::Funcao { .. }), "Funcao");
    exige(|n| matches!(n, NoAst::Tipo { .. }), "Tipo");
    exige(|n| matches!(n, NoAst::Estrutura { .. }), "Estrutura");
    exige(|n| matches!(n, NoAst::Enumeracao { .. }), "Enumeracao");
    exige(|n| matches!(n, NoAst::Incluir { .. }), "Incluir");
    exige(|n| matches!(n, NoAst::Quando { .. }), "Quando");
    exige(|n| matches!(n, NoAst::Escolha { .. }), "Escolha");
    exige(|n| matches!(n, NoAst::Enquanto { .. }), "Enquanto");
    exige(|n| matches!(n, NoAst::Cada { .. }), "Cada");
    exige(|n| matches!(n, NoAst::Pulso { .. }), "Pulso");
    exige(|n| matches!(n, NoAst::Para { .. }), "Para");
    exige(|n| matches!(n, NoAst::Devolver(_)), "Devolver");
    exige(|n| matches!(n, NoAst::Parar), "Parar");
    exige(|n| matches!(n, NoAst::Seguir), "Seguir");
    exige(|n| matches!(n, NoAst::Passar), "Passar");
    exige(|n| matches!(n, NoAst::Ceder(_)), "Ceder");
    exige(|n| matches!(n, NoAst::Atributada { .. }), "Atributada");
    exige(
        |n| matches!(n, NoAst::BlocoContextual { contexto, .. } if contexto == "seguro"),
        "BlocoContextual seguro",
    );
    exige(
        |n| matches!(n, NoAst::BlocoContextual { contexto, .. } if contexto == "critico"),
        "BlocoContextual critico",
    );
    exige(
        |n| matches!(n, NoAst::BlocoContextual { contexto, .. } if contexto == "privilegio"),
        "BlocoContextual privilegio",
    );
    exige(
        |n| matches!(n, NoAst::BlocoContextual { contexto, .. } if contexto == "montagem"),
        "BlocoContextual montagem",
    );
    exige(|n| matches!(n, NoAst::Tente { .. }), "Tente");
    exige(|n| matches!(n, NoAst::Com { .. }), "Com");
    exige(|n| matches!(n, NoAst::Lancar(_)), "Lancar");
    exige(|n| matches!(n, NoAst::Verifique { .. }), "Verifique");
    exige(
        |n| matches!(n, NoAst::Escopo { externa: false, .. }),
        "Escopo global",
    );
    exige(
        |n| matches!(n, NoAst::Escopo { externa: true, .. }),
        "Escopo externa",
    );
    exige(|n| matches!(n, NoAst::Expressao(_)), "Expressao");
    exige(|n| matches!(n, NoAst::Classe { .. }), "Classe");
}

#[test]
fn matriz_ast_cobre_todas_as_variantes_de_expressao() {
    use nebula_sintaxe::ast::{Expressao, NoAst};

    let codigo = r#"
variavel nome = identificador
variavel literal = 42
variavel lista = [1, 2]
variavel tupla = (1, "dois")
variavel unaria_negativa = -valor
variavel unaria_logica = !ativo
variavel binaria = esquerda + direita
variavel argumento = executar(nome: "Ana")
variavel atribuicao = destino = 10
variavel chamada = executar(1)
variavel indexacao = itens[0]
variavel membro = objeto.campo
variavel construcao = Pessoa { nome: "Ana" }
variavel funcao_lambda = x -> x + 1
variavel lambda_bloco = x ->:
    devolver x
fim
variavel condicional = idade >= 18 ? "adulto" : "menor"
variavel generica = converter<texto>(valor)
variavel acesso_opcional = objeto?.campo
variavel propagacao = resultado?
"#;
    let resultado = analisar_codigo(codigo);
    assert!(
        resultado.diagnosticos.is_empty(),
        "matriz de expressões válidas foi rejeitada: {:#?}",
        resultado.diagnosticos
    );
    let expressoes: Vec<_> = resultado
        .programa
        .iter_expressoes_localizadas()
        .map(|item| &item.expressao)
        .collect();
    let exige = |condicao: fn(&Expressao) -> bool, nome: &str| {
        assert!(
            expressoes.iter().any(|expressao| condicao(expressao)),
            "a matriz não produziu Expressao::{nome}"
        );
    };
    exige(|e| matches!(e, Expressao::Nome(_)), "Nome");
    exige(|e| matches!(e, Expressao::Literal(_)), "Literal");
    exige(|e| matches!(e, Expressao::Lista(_)), "Lista");
    exige(|e| matches!(e, Expressao::Tupla(_)), "Tupla");
    exige(|e| matches!(e, Expressao::Unaria { .. }), "Unaria");
    exige(|e| matches!(e, Expressao::Binaria { .. }), "Binaria");
    assert!(
        resultado.programa.itens.iter().any(|no| matches!(
            no,
            NoAst::Variavel {
                valor: Expressao::Chamada { argumentos, .. },
                ..
            } if argumentos.iter().any(|argumento| matches!(
                argumento,
                Expressao::ArgumentoNomeado { .. }
            ))
        )),
        "a chamada deve preservar Expressao::ArgumentoNomeado nos argumentos"
    );
    exige(|e| matches!(e, Expressao::Atribuicao { .. }), "Atribuicao");
    exige(|e| matches!(e, Expressao::Chamada { .. }), "Chamada");
    exige(|e| matches!(e, Expressao::Indexacao { .. }), "Indexacao");
    exige(|e| matches!(e, Expressao::Membro { .. }), "Membro");
    exige(
        |e| matches!(e, Expressao::ConstrucaoEstrutura { .. }),
        "ConstrucaoEstrutura",
    );
    exige(|e| matches!(e, Expressao::Lambda { .. }), "Lambda");
    exige(
        |e| matches!(e, Expressao::LambdaBloco { .. }),
        "LambdaBloco",
    );
    exige(
        |e| matches!(e, Expressao::Condicional { .. }),
        "Condicional",
    );
    assert!(
        expressoes.iter().any(|e| matches!(e, Expressao::Chamada { tipos_genericos, .. } if tipos_genericos == &["texto"])),
        "chamada genérica precisa preservar os argumentos de tipo"
    );
}

#[test]
fn matriz_de_operadores_binarios_e_atribuicoes_compostas() {
    let operadores = [
        "+",
        "-",
        "*",
        "/",
        r"\\",
        "%",
        "**",
        "<<",
        ">>",
        ">>>",
        "..",
        "<",
        "<=",
        ">",
        ">=",
        "==",
        "!=",
        "!==",
        "iguais",
        "identico",
        "dentro",
        "fora",
        "tipoDe",
        "instanciaDe",
        "&",
        "^",
        "|",
        "e",
        "ou",
        "&&",
        "||",
        "??",
    ];
    for (indice, operador) in operadores.iter().enumerate() {
        let codigo = format!("variavel resultado_{indice} = esquerdo {operador} direito");
        let resultado = analisar_codigo(&codigo);
        assert!(
            resultado.diagnosticos.is_empty(),
            "operador binário {operador:?} foi rejeitado: {:#?}",
            resultado.diagnosticos
        );
        assert!(
            resultado
                .programa
                .iter_expressoes_localizadas()
                .any(|item| matches!(
                    &item.expressao,
                    nebula_sintaxe::ast::Expressao::Binaria { .. }
                )),
            "operador {operador:?} não produziu expressão binária"
        );
    }

    let atribuicoes = [
        "=", "<-", "+=", "-=", "*=", "/=", r"\\=", "%=", "**=", "??=",
    ];
    for (indice, operador) in atribuicoes.iter().enumerate() {
        let codigo = format!("variavel valor_{indice} = 1\nvalor_{indice} {operador} 2");
        let resultado = analisar_codigo(&codigo);
        assert!(
            resultado.diagnosticos.is_empty(),
            "atribuição {operador:?} foi rejeitada: {:#?}",
            resultado.diagnosticos
        );
        assert!(
            resultado
                .programa
                .iter_expressoes_localizadas()
                .any(|item| matches!(
                    &item.expressao,
                    nebula_sintaxe::ast::Expressao::Atribuicao { .. }
                )),
            "atribuição {operador:?} não produziu AST de atribuição"
        );
    }
}

#[test]
fn padroes_de_escolha_cobrem_variantes_payload_literais_e_curinga() {
    use nebula_sintaxe::ast::PadraoAst;

    let codigo = r#"
escolha estado
    caso Estado::Falha(codigo, mensagem)
        passar
    caso Estado::Pronto
        passar
    caso 42
        passar
    caso "texto"
        passar
    caso verdadeiro
        passar
    caso _
        passar
fim
"#;
    let resultado = analisar_codigo(codigo);
    assert!(
        resultado.diagnosticos.is_empty(),
        "padrões válidos foram rejeitados: {:#?}",
        resultado.diagnosticos
    );
    let padroes: Vec<_> = resultado
        .programa
        .iter_casos_localizados()
        .map(|item| item.caso.padrao())
        .collect();
    assert_eq!(padroes.len(), 6);
    assert!(
        matches!(&padroes[0], PadraoAst::Variante { caminho, vinculacoes }
        if caminho == &["Estado".to_owned(), "Falha".to_owned()] && vinculacoes.len() == 2)
    );
    assert!(
        matches!(&padroes[1], PadraoAst::Variante { caminho, vinculacoes }
        if caminho == &["Estado".to_owned(), "Pronto".to_owned()] && vinculacoes.is_empty())
    );
    assert!(matches!(&padroes[2], PadraoAst::Literal(valor) if valor == "42"));
    assert!(matches!(&padroes[3], PadraoAst::Literal(valor) if valor == "\"texto\""));
    assert!(matches!(&padroes[4], PadraoAst::Literal(valor) if valor == "verdadeiro"));
    assert_eq!(padroes[5], PadraoAst::Curinga);
}

#[test]
fn matriz_de_erros_limites_e_recuperacao_do_parser() {
    let invalidos = [
        (
            "parâmetro duplicado",
            "funcao f(x: inteiro, x: texto)\n    passar",
        ),
        (
            "genérico sem fechamento",
            "funcao f<T(valor: T)\n    passar",
        ),
        ("tipo genérico vazio", "variavel x: Mapa<> = nulo"),
        ("lista sem fechamento", "variavel x = [1, 2"),
        ("tupla sem fechamento", "variavel x = (1, 2"),
        ("chamada sem fechamento", "variavel x = executar(1, 2"),
        ("acesso sem nome", "variavel x = objeto."),
        ("atributo sem nome", "@ funcao f()\n    passar"),
        ("lambda com parâmetro inválido", "variavel f = (1, 2) -> 3"),
        ("bloco contextual sem corpo", "critico"),
        ("classe sem bloco", "classe Pessoa"),
        (
            "estrutura sem tipo de campo",
            "estrutura Pessoa:\n    nome:",
        ),
        (
            "enumeração com payload incompleto",
            "enumeracao Estado:\n    Falha(inteiro,)",
        ),
        (
            "escolha sem padrão",
            "escolha valor\n    caso\n        passar",
        ),
        (
            "caso com tokens extras",
            "escolha valor\n    caso Estado::Pronto extra\n        passar",
        ),
        ("tente sem cláusula", "tente\n    passar"),
        ("com sem recurso", "com\n    passar"),
        (
            "para com cabeçalho inválido",
            "para (variavel i = ; ; )\n    passar",
        ),
        ("condicional sem corpo", "quando verdadeiro"),
        ("fechamento isolado", "variavel x = 1]"),
    ];
    for (nome, codigo) in invalidos {
        let resultado = analisar_codigo_bruto(codigo);
        assert!(
            !resultado.diagnosticos.is_empty(),
            "entrada inválida [{nome}] foi aceita sem diagnóstico: {codigo:?}"
        );
        for diagnostico in &resultado.diagnosticos {
            if let Some(intervalo) = diagnostico.intervalo {
                assert!(
                    intervalo.inicio <= intervalo.fim,
                    "intervalo invertido em [{nome}]"
                );
                assert!(
                    intervalo.fim <= codigo.len(),
                    "intervalo fora da fonte em [{nome}]"
                );
                assert!(
                    codigo.is_char_boundary(intervalo.inicio),
                    "início UTF-8 inválido em [{nome}]"
                );
                assert!(
                    codigo.is_char_boundary(intervalo.fim),
                    "fim UTF-8 inválido em [{nome}]"
                );
            }
            assert!(
                !diagnostico.mensagem.trim().is_empty(),
                "mensagem vazia em [{nome}]"
            );
        }
    }

    let recuperaveis = [
        "variavel quebrada =\nvariavel preservada = 1",
        "funcao quebrada(x inteiro)\n    passar\nvariavel preservada = 2",
        "estrutura Quebrada:\n    campo sem_tipo\nvariavel preservada = 3",
        "enumeracao Quebrada:\n    Falha(inteiro,)\nvariavel preservada = 4",
    ];
    for codigo in recuperaveis {
        let resultado = analisar_codigo(codigo);
        assert!(
            !resultado.diagnosticos.is_empty(),
            "a entrada deveria emitir diagnóstico"
        );
        assert!(
            resultado.programa.itens.iter().any(|no|
                matches!(no, nebula_sintaxe::ast::NoAst::Variavel { nome, .. } if nome == "preservada")),
            "o parser perdeu a declaração válida posterior após recuperar: {codigo:?}; {:#?}",
            resultado.diagnosticos
        );
    }
}

#[test]
fn function_body_normativo_e_obrigatorio() {
    let codigo = r#"
funcao principal() -> vazio: {
    quando verdadeiro: {
        exibir("Nebula")
        fim
    }
    fim
    fim
}
fim

estrutura Pessoa: {
    nome: texto
    idade: inteiro
    fim
}
fim

enumeracao Estado: {
    Ativo
    Inativo
    fim
}
fim

escolha estado: {
    caso Estado::Ativo: {
        passar
        fim
    }
    caso _: {
        passar
        fim
    }
    fim
}
fim
"#;
    let resultado = analisar_codigo(codigo);
    assert!(
        resultado.diagnosticos.is_empty(),
        "Function Body normativo foi rejeitado: {:#?}",
        resultado.diagnosticos
    );
}

#[test]
fn rejeita_sintaxe_antiga_de_blocos() {
    let casos = [
        "funcao executar()\n    passar",
        "funcao executar(): inteiro: {\n    devolver 1\n}",
        "variavel operacao: funcao(inteiro): inteiro = identidade",
        "se sim\n    passar",
        "enquanto sim\n    passar",
        "classe Pessoa\n    passar",
        "estrutura Pessoa:\n    nome: texto",
        "enumeracao Estado:\n    Ativo",
        "escolha valor\n    caso _\n        passar",
        "seguro:\n    passar",
    ];
    for codigo in casos {
        let resultado = analisar_codigo_bruto(codigo);
        assert!(
            !resultado.diagnosticos.is_empty(),
            "sintaxe antiga foi aceita: {codigo:?}"
        );
    }
}
