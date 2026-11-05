//! Testes da nova especificação léxica Nebula.
use super::{TipoToken, analisar_lexico};
use nebula_fonte::ArquivoFonte;

fn fonte(codigo: &str) -> ArquivoFonte {
    ArquivoFonte::novo("lexer_teste.nbx", codigo)
}
fn analisar(codigo: &str) -> super::ResultadoLexico {
    analisar_lexico(&fonte(codigo))
}
fn possui_erro(codigo: &str, codigo_erro: &str) -> bool {
    analisar(codigo)
        .diagnosticos
        .iter()
        .any(|d| d.codigo == codigo_erro)
}

#[test]
fn reconhece_palavras_reservadas_da_sintaxe_principal() {
    let r = analisar(
        "variavel constante funcao classe herda se sendo senao para dentro enquanto escolha caso padrao pare prossiga passar retorne lambda ceder incluir biblioteca modulo de como tente erro sempre lancar verifique com assinc aguarda global externa isto novo apagar identico nao fora e ou sim verdadeiro falso nulo iguais tipoDe instanciaDe obter definir vazio exportar construtor estatico privado publico protegido",
    );
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    let esperados = [
        TipoToken::Variavel,
        TipoToken::Constante,
        TipoToken::Funcao,
        TipoToken::Classe,
        TipoToken::Herda,
        TipoToken::Se,
        TipoToken::Sendo,
        TipoToken::Senao,
        TipoToken::Para,
        TipoToken::Dentro,
        TipoToken::Enquanto,
        TipoToken::Escolha,
        TipoToken::Caso,
        TipoToken::Padrao,
        TipoToken::Pare,
        TipoToken::Prossiga,
        TipoToken::Passar,
        TipoToken::Retorne,
        TipoToken::Lambda,
        TipoToken::Ceder,
        TipoToken::Incluir,
        TipoToken::Biblioteca,
        TipoToken::Modulo,
        TipoToken::De,
        TipoToken::Como,
        TipoToken::Tente,
        TipoToken::Erro,
        TipoToken::Sempre,
        TipoToken::Lancar,
        TipoToken::Verifique,
        TipoToken::Com,
        TipoToken::Assinc,
        TipoToken::Aguarda,
        TipoToken::Global,
        TipoToken::Externa,
        TipoToken::Isto,
        TipoToken::Novo,
        TipoToken::Apagar,
        TipoToken::Identico,
        TipoToken::Nao,
        TipoToken::E,
        TipoToken::Ou,
        TipoToken::Sim,
        TipoToken::Verdadeiro,
        TipoToken::Falso,
        TipoToken::Nulo,
        TipoToken::Iguais,
        TipoToken::TipoDe,
        TipoToken::InstanciaDe,
        TipoToken::Obter,
        TipoToken::Definir,
        TipoToken::Vazio,
        TipoToken::Exportar,
        TipoToken::Construtor,
        TipoToken::Estatico,
        TipoToken::Privado,
        TipoToken::Publico,
        TipoToken::Protegido,
    ];
    let tokens: Vec<_> = r
        .tokens
        .iter()
        .filter(|t| t.tipo != TipoToken::Fim)
        .map(|t| t.tipo)
        .collect();
    assert_eq!(tokens, esperados);
}

#[test]
fn reconhece_palavras_de_baixo_nivel_e_deixa_apis_como_identificadores() {
    let r = analisar(
        "critico privilegio montagem u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 endereco enderecoFisico tamanho deslocamento ponteiro ponteiroBruto vetor hardware memoria cpu atomico dma",
    );
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    assert_eq!(r.tokens[0].tipo, TipoToken::Critico);
    assert_eq!(r.tokens[1].tipo, TipoToken::Privilegio);
    assert_eq!(r.tokens[2].tipo, TipoToken::Montagem);
    assert!(
        r.tokens[3..]
            .iter()
            .all(|t| t.tipo == TipoToken::Identificador)
    );
}

#[test]
fn emite_indent_dedent_e_preserva_fim_explicito() {
    let r = analisar("funcao principal()\n    se sim\n        exibir(\"oi\")\n    fim\nfim");
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    let tipos: Vec<_> = r.tokens.iter().map(|t| t.tipo).collect();
    assert_eq!(tipos.iter().filter(|t| **t == TipoToken::Indent).count(), 2);
    assert_eq!(tipos.iter().filter(|t| **t == TipoToken::Dedent).count(), 2);
    assert_eq!(tipos.last(), Some(&TipoToken::Fim));
}

#[test]
fn dedenta_varios_niveis_sem_criar_blocos_falsos_em_linhas_vazias() {
    let r = analisar("a\n\n    b\n        c\n    d\ne");
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    assert_eq!(
        r.tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::Indent)
            .count(),
        2
    );
    assert_eq!(
        r.tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::Dedent)
            .count(),
        2
    );
}

#[test]
fn diagnostica_tabulacao_e_nivel_de_indentacao_inexistente() {
    assert!(possui_erro("a\n\tb", "N1012"));
    assert!(possui_erro("a\n    b\n  c", "N1013"));
}

#[test]
fn reconhece_operadores_da_especificacao_principal() {
    let r = analisar(
        "+ - * / \\\\ % ** ^ == iguais != !== > < >= <= = =+ =- =* =/ =\\\\ =% =** ++ -- -> && || ! ?? ?. ??= ... & | ~ << >> >>> ? : :: @ .. .",
    );
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    let tipos: Vec<_> = r
        .tokens
        .iter()
        .filter(|t| t.tipo != TipoToken::Fim)
        .map(|t| t.tipo)
        .collect();
    for tipo in [
        TipoToken::Mais,
        TipoToken::Menos,
        TipoToken::Vezes,
        TipoToken::Divide,
        TipoToken::DivideInteira,
        TipoToken::Resto,
        TipoToken::Potencia,
        TipoToken::XorBit,
        TipoToken::Igual,
        TipoToken::Iguais,
        TipoToken::Diferente,
        TipoToken::DiferenteEstrito,
        TipoToken::Maior,
        TipoToken::Menor,
        TipoToken::MaiorIgual,
        TipoToken::MenorIgual,
        TipoToken::Atribui,
        TipoToken::SomaAtribui,
        TipoToken::SubtraiAtribui,
        TipoToken::MultiplicaAtribui,
        TipoToken::DivideAtribui,
        TipoToken::DivideInteiraAtribui,
        TipoToken::RestoAtribui,
        TipoToken::PotenciaAtribui,
        TipoToken::Incrementa,
        TipoToken::Decrementa,
        TipoToken::Seta,
        TipoToken::ELogico,
        TipoToken::OuLogico,
        TipoToken::Negacao,
        TipoToken::Coalesce,
        TipoToken::AcessoOpcional,
        TipoToken::CoalesceAtribui,
        TipoToken::Reticencias,
        TipoToken::EBit,
        TipoToken::OuBit,
        TipoToken::NaoBit,
        TipoToken::DeslocaEsquerda,
        TipoToken::DeslocaDireita,
        TipoToken::DeslocaDireitaSemSinal,
        TipoToken::Interrogacao,
        TipoToken::DoisPontos,
        TipoToken::Escopo,
        TipoToken::Arroba,
        TipoToken::Intervalo,
        TipoToken::Ponto,
    ] {
        assert!(tipos.contains(&tipo), "operador ausente: {tipo:?}");
    }
}

#[test]
fn reconhece_alias_convencional_de_divisao_inteira() {
    let r = analisar("//= 2");
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    assert_eq!(r.tokens[0].tipo, TipoToken::DivideInteiraAtribui);
}

#[test]
fn respeita_maior_correspondencia_de_operadores() {
    let r = analisar(">>> >> >= > !== != ** * ... .. . :: : ?? ??= ?. ?");
    let tipos: Vec<_> = r
        .tokens
        .iter()
        .filter(|t| t.tipo != TipoToken::Fim)
        .map(|t| t.tipo)
        .collect();
    assert_eq!(
        tipos,
        [
            TipoToken::DeslocaDireitaSemSinal,
            TipoToken::DeslocaDireita,
            TipoToken::MaiorIgual,
            TipoToken::Maior,
            TipoToken::DiferenteEstrito,
            TipoToken::Diferente,
            TipoToken::Potencia,
            TipoToken::Vezes,
            TipoToken::Reticencias,
            TipoToken::Intervalo,
            TipoToken::Ponto,
            TipoToken::Escopo,
            TipoToken::DoisPontos,
            TipoToken::CoalesceAtribui,
            TipoToken::AcessoOpcional,
            TipoToken::Interrogacao
        ]
    );
}

#[test]
fn reconhece_identificadores_unicode_e_normaliza_classificacao() {
    let r = analisar("ação ação função função público publico");
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    assert_eq!(r.tokens[0].tipo, TipoToken::Identificador);
    assert_eq!(r.tokens[0].lexema, "ação");
    assert_eq!(r.tokens[1].tipo, TipoToken::Identificador);
    assert_eq!(r.tokens[1].lexema, "ação");
    assert_eq!(r.tokens[2].tipo, TipoToken::Funcao);
    assert_eq!(r.tokens[3].tipo, TipoToken::Funcao);
    assert_eq!(r.tokens[4].tipo, TipoToken::Publico);
    assert_eq!(r.tokens[5].tipo, TipoToken::Publico);
}

#[test]
fn reconhece_numeros_decimais_expoentes_e_bases() {
    let r = analisar("0 42 1_000 3.14 0.25 2e10 1E+2 1e-2 0xFF 0b101 0o77");
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    assert_eq!(
        r.tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::Inteiro)
            .count(),
        6
    );
    assert_eq!(
        r.tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::Decimal)
            .count(),
        5
    );
}

#[test]
fn diagnostica_numeros_malformados() {
    for n in [
        "0x", "0b", "0o", "0b2", "0o8", "0xGG", "0b102", "0x_FF", "0b_1", "0o7_", "0xA__B", "1_",
        "1__2", "1e", "1e+", "1e-",
    ] {
        assert!(
            !analisar(n).diagnosticos.is_empty(),
            "literal inválido aceito: {n}"
        );
    }
}

#[test]
fn reconhece_texto_caractere_texto_multilinha_e_interpolacao() {
    let r = analisar(
        r#""Nebula\n" 'Nebula' 'ação' """linha 1
linha 2""" "#,
    );
    let interpolado = analisar("\x60Ola, nome\x60");
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    assert!(
        interpolado.diagnosticos.is_empty(),
        "{:?}",
        interpolado.diagnosticos
    );
    assert!(r.tokens.iter().any(|t| t.tipo == TipoToken::Texto));
    assert_eq!(
        r.tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::Texto)
            .count(),
        4
    );
    assert!(
        interpolado
            .tokens
            .iter()
            .any(|t| t.tipo == TipoToken::TextoInterpolado)
    );
}

#[test]
fn diagnostica_escapes_e_caracteres_invalidos() {
    for literal in [
        r#""\q""#,
        r#""\u{}""#,
        r#""\u{D800}""#,
        r#""\u{110000}""#,
        r#"'ab'"#,
        "''",
    ] {
        assert!(
            !analisar(literal).diagnosticos.is_empty(),
            "literal inválido aceito: {literal}"
        );
    }
}

#[test]
fn reconhece_comentarios_de_linha_e_bloco_sem_perder_quebras() {
    let r =
        analisar("variavel a = 1 # comentario\n// outro\n/* bloco\ncontinua */\nvariavel b = 2");
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    assert_eq!(
        r.tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::NovaLinha)
            .count(),
        4
    );
    assert_eq!(
        r.tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::Variavel)
            .count(),
        2
    );
}

#[test]
fn nova_linha_e_suprimida_em_parenteses_e_colchetes() {
    let r = analisar("chamar(\n  1,\n  2\n)\nlista = [\n  1,\n  2\n]");
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    assert_eq!(
        r.tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::NovaLinha)
            .count(),
        1
    );
}

#[test]
fn chaves_nao_suprimem_quebras_de_linha() {
    let r = analisar("Pessoa {\n nome: \"Ana\",\n idade: 20\n}");
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    assert!(
        r.tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::NovaLinha)
            .count()
            >= 3
    );
}

#[test]
fn diagnostica_delimitadores_desbalanceados_e_cr_isolado() {
    assert!(possui_erro("funcao x(", "N1011"));
    assert!(possui_erro(")", "N1015"));
    assert!(possui_erro("([)]", "N1014"));
    assert!(possui_erro("a\rb", "N1001"));
}

#[test]
fn preserva_offsets_em_bytes_e_lexemas_originais() {
    let codigo = "variavel ação = 1";
    let r = analisar(codigo);
    assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    let t = r.tokens.iter().find(|t| t.lexema == "ação").unwrap();
    assert_eq!(&codigo[t.intervalo.inicio..t.intervalo.fim], "ação");
    assert_eq!(t.intervalo.inicio, "variavel ".len());
}

#[test]
fn emite_fim_unico_e_dedents_no_final_do_arquivo() {
    let r = analisar("inicio\n    meio");
    assert_eq!(
        r.tokens.iter().filter(|t| t.tipo == TipoToken::Fim).count(),
        1
    );
    assert_eq!(r.tokens.last().unwrap().tipo, TipoToken::Fim);
    assert_eq!(
        r.tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::Dedent)
            .count(),
        1
    );
}
