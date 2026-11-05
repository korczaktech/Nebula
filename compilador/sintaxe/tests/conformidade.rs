//! Suite executável de conformidade entre a gramática normativa e o frontend.

use nebula_fonte::ArquivoFonte;
use nebula_sintaxe::analisar;

fn analisar_codigo(codigo: &str) -> nebula_sintaxe::ResultadoSintatico {
    analisar(&ArquivoFonte::novo("conformidade.nbx", codigo))
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
            "publico funcao identidade<T>(valor: T): T\n    devolver valor\nfim",
        ),
        (
            "funcao privada e parametros",
            "privado funcao combinar<T, U>(a: [T], b: U): funcao(T): U\n    devolver b\nfim",
        ),
        ("funcao sem retorno", "funcao executar()\n    passar\nfim"),
        (
            "tipos simples e qualificados",
            "tipo A = inteiro\ntipo B <- Modulo::Interno::Tipo",
        ),
        (
            "tipos compostos",
            "tipo Lista = [[Resultado<inteiro>]]\ntipo Funcao = funcao(inteiro, texto): Resultado<[texto]>",
        ),
        (
            "estrutura vazia",
            "estrutura Vazia:\n    marcador: inteiro\nfim",
        ),
        (
            "estrutura com campos e separadores",
            "estrutura Pessoa:\n    nome: texto\n    idade: inteiro\n    ativo: logico\nfim",
        ),
        (
            "enumeracao com variantes",
            "enumeracao Estado:\n    Vazio\n    Unidade()\n    Valor(inteiro, texto)\nfim",
        ),
        ("inclusao com alias", "incluir Biblioteca::Texto como Texto"),
        ("inclusao sem alias", "incluir Biblioteca::Interna"),
        (
            "quando e senao",
            "quando verdadeiro\n    exibir(1)\nsenao\n    exibir(0)\nfim",
        ),
        (
            "quando encadeado",
            "quando a\n    passar\nsendo b\n    passar\nsenao\n    passar\nfim",
        ),
        (
            "escolha e padroes",
            "escolha valor\n    caso Resultado::Sucesso(texto)\n        exibir(texto)\n    caso Resultado::Falha(codigo)\n        passar\n    caso _\n        passar\nfim",
        ),
        (
            "alias avaliar",
            "avaliar valor\n    caso Estado::Pronto\n        passar\nfim",
        ),
        (
            "lacos enquanto",
            "enquanto verdadeiro\n    parar\nfim\ndurante falso\n    seguir\nfim\nrepetir verdadeiro\n    encerrar\nfim",
        ),
        (
            "iteracao cada",
            "cada item em [1, 2, 3]\n    exibir(item)\nfim",
        ),
        ("pulso", "pulso(100)\n    exibir(1)\nfim"),
        (
            "para com declaracao",
            "para (variavel i = 0; i < 3; i = i + 1)\n    exibir(i)\nfim",
        ),
        (
            "para com expressoes opcionais",
            "para (inicio = 0; ; passo = passo + 1)\n    passar\nfim\npara (; condicao; )\n    passar\nfim\npara (;; )\n    passar\nfim",
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
            "Pessoa {}\nPessoa { nome: \"Ana\", idade: 20 }",
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
            "funcao executar()\n    variavel ativo = verdadeiro\n    enquanto ativo\n        ativo = falso\n    fim\nfim",
        ),
        (
            "bloco com instrucao vazia",
            "funcao sem_corpo()\n    passar\nfim",
        ),
        (
            "aliases sem acentos",
            "funcao principal()\n    quando nao falso\n        passar\n    senao\n        passar\n    fim\nfim\nenumeracao Estado:\n    Pronto\nfim\npublico funcao iniciar()\n    passar\nfim",
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
        ("fechamento ausente", "N2009", "funcao f() {"),
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
            "estrutura Pessoa:\n    nome: texto\n    nome: inteiro\nfim",
        ),
        (
            "separador de campo ausente",
            "N2032",
            "estrutura Pessoa:\n    nome: texto idade: inteiro\nfim",
        ),
        (
            "abertura de enumeracao ausente",
            "N2033",
            "enumeracao Estado Ativo }",
        ),
        (
            "variante duplicada",
            "N2034",
            "enumeracao Estado:\n    Ativo\n    Ativo\nfim",
        ),
        (
            "payload malformado",
            "N2035",
            "enumeracao Estado:\n    Ativo(inteiro,)\nfim",
        ),
        (
            "enumeracao vazia",
            "N2036",
            "enumeracao Estado:\n    variavel",
        ),
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
        ("caso ausente", "N2041", "escolha valor:\n    outro\nfim"),
        (
            "padrao invalido",
            "N2042",
            "escolha valor:\n    caso Variante\n        passar\n    fim\nfim",
        ),
        (
            "curinga duplicado",
            "N2043",
            "escolha valor:\n    caso _\n        passar\n    caso _\n        passar\nfim",
        ),
        (
            "escolha vazia",
            "N2044",
            "escolha valor:\n    variavel x = 1\nfim",
        ),
        (
            "vinculo de payload ausente",
            "N2045",
            "escolha valor:\n    caso Estado::Erro(1)\n        passar\nfim",
        ),
        (
            "vinculo duplicado",
            "N2046",
            "escolha valor:\n    caso Estado::Erro(item, item)\n        passar\nfim",
        ),
        ("alterar sem atribuicao", "N2047", "alterar total"),
        (
            "padrao de variante duplicado",
            "N2048",
            "escolha valor:\n    caso Estado::Erro\n        passar\n    caso Estado::Erro\n        passar\nfim",
        ),
        (
            "parametro duplicado",
            "N2050",
            "funcao f(item: inteiro, item: texto)\n    passar\nfim",
        ),
        ("fechamento sem abertura", "N2051", "variavel valor = 1)"),
    ];

    for (nome, codigo_esperado, codigo_fonte) in casos {
        let resultado = analisar_codigo(codigo_fonte);
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
    let resultado = analisar_codigo(codigo);
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
incluir Biblioteca::Texto como Texto
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
            "funcao f(x: inteiro, x: texto)\n    passar\nfim",
        ),
        (
            "genérico sem fechamento",
            "funcao f<T(valor: T)\n    passar\nfim",
        ),
        ("tipo genérico vazio", "variavel x: Mapa<> = nulo"),
        ("lista sem fechamento", "variavel x = [1, 2"),
        ("tupla sem fechamento", "variavel x = (1, 2"),
        ("chamada sem fechamento", "variavel x = executar(1, 2"),
        ("acesso sem nome", "variavel x = objeto."),
        ("atributo sem nome", "@ funcao f()\n    passar\nfim"),
        ("lambda com parâmetro inválido", "variavel f = (1, 2) -> 3"),
        ("bloco contextual sem corpo", "critico"),
        ("classe sem bloco", "classe Pessoa"),
        (
            "estrutura sem tipo de campo",
            "estrutura Pessoa:\n    nome:\nfim",
        ),
        (
            "enumeração com payload incompleto",
            "enumeracao Estado:\n    Falha(inteiro,)\nfim",
        ),
        (
            "escolha sem padrão",
            "escolha valor\n    caso\n        passar\nfim",
        ),
        (
            "caso com tokens extras",
            "escolha valor\n    caso Estado::Pronto extra\n        passar\nfim",
        ),
        ("tente sem cláusula", "tente\n    passar\nfim"),
        ("com sem recurso", "com\n    passar\nfim"),
        (
            "para com cabeçalho inválido",
            "para (variavel i = ; ; )\n    passar\nfim",
        ),
        ("condicional sem corpo", "quando verdadeiro\nfim"),
        ("fechamento isolado", "variavel x = 1]"),
    ];
    for (nome, codigo) in invalidos {
        let resultado = analisar_codigo(codigo);
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
        "funcao quebrada(x inteiro)\n    passar\nfim\nvariavel preservada = 2",
        "estrutura Quebrada:\n    campo sem_tipo\nfim\nvariavel preservada = 3",
        "enumeracao Quebrada:\n    Falha(inteiro,)\nfim\nvariavel preservada = 4",
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
