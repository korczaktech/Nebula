//! Contratos de integração da AST vistos por consumidores externos do crate.

use nebula_fonte::ArquivoFonte;
use nebula_sintaxe::{
    AliasTipoAst, AtributoAst, CampoAst, CapturaAst, CasoEscolha, CasoEscolhaLocalizado, Expressao,
    ExpressaoLocalizada, NoAst, NoLocalizado, OperadorAst, PadraoAst, ParametroAst, Programa,
    ResultadoSintatico, TipoAst, TipoLocalizado, TipoToken, VarianteAst, Visibilidade, analisar,
};
use std::path::Path;

#[test]
fn api_publica_integrada_ao_parser_preserva_ast_e_localizacoes() {
    let fonte = ArquivoFonte::novo(
        "consumidor.nbx",
        "funcao principal(): {\n    variavel nome: texto = \"Nebula\"\n    devolver nome\n    fim\n}\nfim",
    );

    let resultado: ResultadoSintatico = analisar(&fonte);
    assert!(
        resultado.diagnosticos.is_empty(),
        "fonte válida não deve produzir diagnósticos: {:#?}",
        resultado.diagnosticos
    );

    let programa: &Programa = &resultado.programa;
    assert_eq!(
        programa.caminho.as_deref(),
        Some(Path::new("consumidor.nbx"))
    );
    assert!(matches!(
        programa.itens.first(),
        Some(NoAst::Funcao { nome, .. }) if nome == "principal"
    ));

    let no: Option<&NoLocalizado> = programa.iter_localizados().next();
    assert!(no.is_some(), "a API pública deve expor nós localizados");
    let expressao: Option<&ExpressaoLocalizada> = programa.iter_expressoes_localizadas().next();
    assert!(
        expressao.is_some(),
        "a API pública deve expor expressões localizadas"
    );
    let tipo: Option<&TipoLocalizado> = programa.iter_tipos_localizados().next();
    assert!(tipo.is_some(), "a API pública deve expor tipos localizados");

    assert!(programa.iter_localizados().all(|item| {
        item.intervalo.inicio <= item.intervalo.fim && item.intervalo.fim <= fonte.conteudo().len()
    }));
}

#[test]
fn tipos_publicos_da_ast_podem_ser_consumidos_sem_modulos_internos() {
    let tipo = TipoAst::analisar("Resultado<inteiro, texto>");
    assert!(matches!(tipo, TipoAst::Generico { argumentos, .. } if argumentos.len() == 2));

    let alias = AliasTipoAst {
        nome: "Identificador".into(),
        destino: TipoAst::Nome("inteiro".into()),
    };
    let parametro = ParametroAst {
        nome: "valor".into(),
        tipo: TipoAst::Nome("inteiro".into()),
    };
    let campo = CampoAst {
        nome: "codigo".into(),
        tipo: TipoAst::Nome("inteiro".into()),
    };
    let variante = VarianteAst {
        nome: "Falha".into(),
        campos: vec![TipoAst::Nome("texto".into())],
    };
    let atributo = AtributoAst {
        nome: "alinhamento".into(),
        argumentos: vec![Expressao::Literal("8".into())],
    };
    let captura = CapturaAst {
        tipo: Some("Erro".into()),
        corpo: vec![NoAst::Passar],
    };
    let caso = CasoEscolha {
        enumeracao: Some("Estado".into()),
        variante: Some("Falha".into()),
        vinculacoes: vec!["codigo".into()],
        expressao: None,
        corpo: vec![NoAst::Passar],
    };

    assert_eq!(alias.nome, "Identificador");
    assert_eq!(parametro.nome, "valor");
    assert_eq!(campo.nome, "codigo");
    assert_eq!(variante.nome, "Falha");
    assert_eq!(atributo.nome, "alinhamento");
    assert_eq!(captura.tipo.as_deref(), Some("Erro"));
    assert_eq!(
        caso.padrao(),
        PadraoAst::Variante {
            caminho: vec!["Estado".into(), "Falha".into()],
            vinculacoes: vec![PadraoAst::Vinculacao("codigo".into())],
        }
    );
    assert_eq!(
        OperadorAst::try_from(TipoToken::Reticencias),
        Ok(OperadorAst::Espalhamento)
    );

    // Confirma que a localização de casos também faz parte da API pública.
    let _: Option<CasoEscolhaLocalizado> = None;
    let _: Option<Visibilidade> = Some(Visibilidade::Publico);
}
