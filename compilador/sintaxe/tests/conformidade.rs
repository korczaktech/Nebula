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
            "variavel a = 1\ndefinir b: inteiro = 2\nmutavel c <- 3\nmutável d: texto <- \"Nebula\"\nconstante ativo = verdadeiro",
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
            "funcao vazio()\n    passar\nfim",
        ),
        (
            "grafias ortograficas",
            "função principal()\n    quando não falso\n        passar\n    senao\n        passar\n    fim\nfim\nenumeração Estado:\n    Pronto\nfim\npúblico função iniciar()\n    passar\nfim",
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
        ("construcao futura", "N2016", "assinc funcao f() {}"),
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
        ("enumeracao vazia", "N2036", "enumeracao Estado:\nfim"),
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
        ("escolha vazia", "N2044", "escolha valor:\nfim"),
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
estrutura Pessoa:\n    nome: texto\n    idade: inteiro\nfim
enumeracao Estado:\n    Ativo\n    Inativo\nfim
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
