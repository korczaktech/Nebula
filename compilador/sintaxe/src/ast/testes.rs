//! Testes públicos da estrutura e da integração da AST.

use super::{
    AliasTipoAst, CasoEscolha, Expressao, NoAst, OperadorAst, PadraoAst, TipoAst, Visibilidade,
};
use crate::analisar;
use nebula_fonte::ArquivoFonte;

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
    assert!(OperadorAst::try_from(crate::TipoToken::Identificador).is_err());
}

#[test]
fn parser_registra_intervalos_de_nos_aninhados() {
    let codigo = "funcao principal()\n    variavel nome: texto = \"Nebula\"\n    quando verdadeiro\n       devolver nome\n    fim\nfim";
    let resultado = analisar(&ArquivoFonte::novo("ast.nbx", codigo));
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
incluir Modulo::Texto como Texto
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

    let resultado = analisar(&ArquivoFonte::novo("nos.nbx", codigo));
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
variavel construcao = Pessoa { nome: "Ana" }
"#;
    let resultado = analisar(&ArquivoFonte::novo("expressoes.nbx", codigo));
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
