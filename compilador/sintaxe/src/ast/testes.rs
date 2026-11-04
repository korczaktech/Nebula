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
        nome: "converter".into(),
        genericos: vec![],
        parametros: vec![("valor".into(), "[inteiro]".into())],
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
    let codigo = "funcao principal() {\n variavel nome = \"Nebula\"\n quando verdadeiro { devolver nome }\n}";
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
            .any(|localizado| matches!(localizado.no, NoAst::Variavel { .. })),
        "variável aninhada precisa de localização própria"
    );
    assert!(
        resultado
            .programa
            .iter_localizados()
            .any(|localizado| matches!(localizado.no, NoAst::Devolver(_))),
        "retorno aninhado precisa de localização própria"
    );
    for localizado in resultado.programa.iter_localizados() {
        assert!(localizado.intervalo.inicio <= localizado.intervalo.fim);
        assert!(localizado.intervalo.fim <= codigo.len());
        assert!(codigo.is_char_boundary(localizado.intervalo.inicio));
        assert!(codigo.is_char_boundary(localizado.intervalo.fim));
    }
}
