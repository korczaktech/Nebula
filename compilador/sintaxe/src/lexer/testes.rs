//! Testes de conformidade e robustez do Lexer Nebula.

use nebula_fonte::ArquivoFonte;

use super::{TipoToken, analisar_lexico};

fn fonte(codigo: &str) -> ArquivoFonte {
    ArquivoFonte::novo("lexer_teste.nbx", codigo)
}

fn possui_erro(codigo: &str, codigo_erro: &str) -> bool {
    analisar_lexico(&fonte(codigo))
        .diagnosticos
        .iter()
        .any(|d| d.codigo == codigo_erro)
}

#[test]
fn reconhece_palavras_reservadas_e_identificadores_unicode() {
    let resultado = analisar_lexico(&fonte("variavel ação = verdadeiro"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(resultado.tokens[0].tipo, TipoToken::Variavel);
    assert_eq!(resultado.tokens[1].lexema, "ação");
    assert_eq!(resultado.tokens[3].tipo, TipoToken::Verdadeiro);
}

#[test]
fn reconhece_grafias_com_e_sem_acentos() {
    let resultado = analisar_lexico(&fonte(
        "funcao função senao senão nao não publico público enumeracao enumeração",
    ));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    for tipo in [
        TipoToken::Funcao,
        TipoToken::Senao,
        TipoToken::Nao,
        TipoToken::Publico,
        TipoToken::Enumeracao,
    ] {
        assert_eq!(
            resultado.tokens.iter().filter(|t| t.tipo == tipo).count(),
            2
        );
    }
}

#[test]
fn reconhece_todas_as_palavras_reservadas_iniciais() {
    let resultado = analisar_lexico(&fonte(
        "variavel definir mutavel constante funcao quando senao escolha avaliar caso para cada em pulso enquanto durante repetir alterar devolver incluir como verdadeiro falso nulo fim parar encerrar seguir e ou nao publico privado tipo estrutura enumeracao",
    ));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert!(
        resultado
            .tokens
            .iter()
            .all(|t| t.tipo != TipoToken::Identificador)
    );
}

#[test]
fn reconhece_palavras_reservadas_futuras() {
    let resultado = analisar_lexico(&fonte("tentar capturar finalmente assinc"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::ReservadaFutura)
            .count(),
        4
    );
}

#[test]
fn identificadores_unicode_preservam_lexema_original() {
    let codigo = "ac\u{0327}a\u{0303}o";
    let resultado = analisar_lexico(&fonte(codigo));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(resultado.tokens[0].tipo, TipoToken::Identificador);
    assert_eq!(resultado.tokens[0].lexema, codigo);
}

#[test]
fn rejeita_espacos_unicode_nao_especificados() {
    assert!(possui_erro("a\u{00A0}b", "N1003"));
}

#[test]
fn reconhece_numeros_decimais_e_em_bases() {
    let resultado = analisar_lexico(&fonte("42 1_000 3.14 2e10 0xFF 0b101 0o77"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::Inteiro)
            .count(),
        5
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::Decimal)
            .count(),
        2
    );
}

#[test]
fn reconhece_variacoes_validas_de_numeros_e_separadores() {
    let resultado = analisar_lexico(&fonte(
        "0 00 1_2_3 0xA_B 0b1_0 0o7_0 1.0 0.25 1e2 1E+2 1e-2 1_000.25_00 9_9e1_0",
    ));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
}

#[test]
fn aceita_inteiros_maiores_que_os_limites_das_primitivas() {
    let gugol = format!("1{}", "0".repeat(100));
    let extremo = "9".repeat(4096);
    let codigo = format!("{gugol} {extremo}");
    let resultado = analisar_lexico(&fonte(&codigo));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(resultado.tokens[0].lexema, gugol);
    assert_eq!(resultado.tokens[1].lexema, extremo);
    assert_eq!(resultado.tokens[0].tipo, TipoToken::Inteiro);
    assert_eq!(resultado.tokens[1].tipo, TipoToken::Inteiro);
}

#[test]
fn aceita_expoentes_extremamente_grandes_sem_converter_o_valor() {
    let expoente = format!("1e{}", "9".repeat(2048));
    let resultado = analisar_lexico(&fonte(&expoente));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(resultado.tokens[0].lexema, expoente);
    assert_eq!(resultado.tokens[0].tipo, TipoToken::Decimal);
}

#[test]
fn ponto_final_nao_transforma_inteiro_em_decimal() {
    let resultado = analisar_lexico(&fonte("1. 1..2"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado.tokens.iter().map(|t| t.tipo).collect::<Vec<_>>(),
        vec![
            TipoToken::Inteiro,
            TipoToken::Ponto,
            TipoToken::Inteiro,
            TipoToken::Intervalo,
            TipoToken::Inteiro,
            TipoToken::Fim,
        ]
    );
}

#[test]
fn diagnostica_numeros_malformados_em_todas_as_bases() {
    for numero in [
        "0x", "0b", "0o", "0b2", "0o8", "0xGG", "0b102", "0x_FF", "0b_1", "0o7_", "0xA__B",
    ] {
        assert!(possui_erro(numero, "N1005"), "Literal inválido: {numero:?}");
    }
}

#[test]
fn diagnostica_separadores_e_expoentes_malformados() {
    for numero in [
        "1_", "1__2", "1e", "1e+", "1e-", "1e_2", "1e+_2", "1e2_", "1_._2",
    ] {
        assert!(
            !analisar_lexico(&fonte(numero)).diagnosticos.is_empty(),
            "Literal inválido: {numero:?}"
        );
    }
    assert!(possui_erro("1e+", "N1004"));
}

#[test]
fn reconhece_textos_caracteres_e_escapes() {
    let resultado = analisar_lexico(&fonte(
        r#""Nebula\n\r\t\0\\\"\'" 'á' '\u{1F680}' '\n' '\''"#,
    ));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(resultado.tokens[0].tipo, TipoToken::Texto);
    assert!(
        resultado.tokens[1..]
            .iter()
            .take(4)
            .all(|t| t.tipo == TipoToken::Caractere)
    );
}

#[test]
fn aceita_caracteres_unicode_de_um_escalar() {
    let resultado = analisar_lexico(&fonte("'🚀' '界' 'é' '\u{10FFFF}'"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::Caractere)
            .count(),
        4
    );
}

#[test]
fn rejeita_escapes_desconhecidos_e_unicode_invalido() {
    for literal in [
        r#""\q""#,
        r#""\u{}""#,
        r#""\u{ZZ}""#,
        r#""\u{D800}""#,
        r#""\u{110000}""#,
        r#"'\x'"#,
    ] {
        assert!(
            possui_erro(literal, "N1007"),
            "Literal inválido: {literal:?}"
        );
    }
}

#[test]
fn rejeita_caractere_vazio_com_multiplos_escalres_e_nao_terminado() {
    for literal in ["''", "'ab'", "'a", "'🚀x'"] {
        assert!(
            possui_erro(literal, "N1009"),
            "Literal inválido: {literal:?}"
        );
    }
}

#[test]
fn texto_nao_pode_continuar_em_quebra_fisica_apos_escape() {
    let resultado = analisar_lexico(&fonte("\"texto\\\ncontinua\""));
    assert!(
        resultado.diagnosticos.iter().any(|d| d.codigo == "N1006"),
        "{:?}",
        resultado.diagnosticos
    );
    assert!(
        resultado
            .tokens
            .iter()
            .any(|t| t.tipo == TipoToken::NovaLinha)
    );
}

#[test]
fn diagnostica_texto_nao_terminado() {
    assert!(possui_erro("\"texto sem fim", "N1008"));
}

#[test]
fn reconhece_operadores_pelo_maior_prefixo() {
    let resultado = analisar_lexico(&fonte("== != <= >= <- ** .. :: = < > : / * ."));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    let tipos: Vec<_> = resultado.tokens.iter().map(|t| t.tipo).collect();
    assert_eq!(
        tipos[..15],
        [
            TipoToken::Igual,
            TipoToken::Diferente,
            TipoToken::MenorIgual,
            TipoToken::MaiorIgual,
            TipoToken::Atribui,
            TipoToken::Potencia,
            TipoToken::Intervalo,
            TipoToken::Escopo,
            TipoToken::Atribui,
            TipoToken::Menor,
            TipoToken::Maior,
            TipoToken::DoisPontos,
            TipoToken::Divide,
            TipoToken::Vezes,
            TipoToken::Ponto,
        ]
    );
}

#[test]
fn preserva_quebras_de_linha_e_suprime_quebras_em_parenteses() {
    let resultado = analisar_lexico(&fonte("a\nfuncao f(x,\n y)\n"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::NovaLinha)
            .count(),
        2
    );
}

#[test]
fn reconhece_crlf_como_uma_quebra() {
    let resultado = analisar_lexico(&fonte("a\r\nb"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::NovaLinha)
            .count(),
        1
    );
}

#[test]
fn rejeita_cr_isolado() {
    let resultado = analisar_lexico(&fonte("a\rb"));
    assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N1001"));
}

#[test]
fn reconhece_comentarios_e_preserva_quebras_em_bloco() {
    let resultado = analisar_lexico(&fonte("a // linha\n/* bloco\ncontinua */ b"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::NovaLinha)
            .count(),
        2
    );
}

#[test]
fn comentario_de_linha_nao_consume_a_quebra() {
    let resultado = analisar_lexico(&fonte("x // comentario\ny"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::NovaLinha)
            .count(),
        1
    );
    assert_eq!(resultado.tokens[2].lexema, "y");
}

#[test]
fn reconhece_crlf_dentro_de_comentario_de_bloco() {
    let resultado = analisar_lexico(&fonte("a /* um\r\ndois */ b"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|t| t.tipo == TipoToken::NovaLinha)
            .count(),
        1
    );
}

#[test]
fn marcadores_de_comentario_dentro_de_literal_sao_texto() {
    let resultado = analisar_lexico(&fonte(r#""// texto /* literal */" '/' *"#));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(resultado.tokens[0].tipo, TipoToken::Texto);
    assert_eq!(resultado.tokens[1].tipo, TipoToken::Caractere);
    assert_eq!(resultado.tokens[2].tipo, TipoToken::Vezes);
}

#[test]
fn diagnostica_comentario_de_bloco_nao_terminado() {
    assert!(possui_erro("a /* sem fechamento", "N1002"));
}

#[test]
fn preserva_intervalos_em_bytes_para_unicode() {
    let resultado = analisar_lexico(&fonte("ação '🚀'"));
    assert_eq!(resultado.tokens[0].intervalo.inicio, 0);
    assert_eq!(resultado.tokens[0].intervalo.fim, "ação".len());
    assert_eq!(resultado.tokens[0].lexema, "ação");
    assert_eq!(resultado.tokens[1].intervalo.inicio, "ação ".len());
    assert_eq!(resultado.tokens[1].intervalo.fim, "ação '🚀'".len());
}

#[test]
fn sempre_encerra_com_token_fim_mesmo_apos_erro() {
    for codigo in ["", "@", "\"texto", "0b2", "/* aberto"] {
        let resultado = analisar_lexico(&fonte(codigo));
        assert_eq!(resultado.tokens.last().unwrap().tipo, TipoToken::Fim);
    }
}

#[test]
fn delimitadores_nao_causam_underflow_na_continuacao_de_linha() {
    let resultado = analisar_lexico(&fonte("))) ((( [ ]"));
    assert!(
        resultado
            .tokens
            .iter()
            .any(|t| t.tipo == TipoToken::FechaPar)
    );
    assert!(
        resultado
            .tokens
            .iter()
            .any(|t| t.tipo == TipoToken::AbreColchete)
    );
}

#[test]
fn delimitador_de_fechamento_errado_nao_fecha_outro_tipo() {
    let resultado = analisar_lexico(&fonte("([)\n]\ny"));
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|token| token.tipo == TipoToken::NovaLinha)
            .count(),
        0
    );
}

#[test]
fn normaliza_palavras_reservadas_unicode_em_nfc() {
    let resultado = analisar_lexico(&fonte("função"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(resultado.tokens[0].tipo, TipoToken::Funcao);
}

#[test]
fn reconhece_pontuacao_e_operadores_de_um_caractere() {
    let resultado = analisar_lexico(&fonte("(){}[],;:+-*/%!=<>"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .map(|token| token.tipo)
            .collect::<Vec<_>>(),
        vec![
            TipoToken::AbrePar,
            TipoToken::FechaPar,
            TipoToken::AbreChave,
            TipoToken::FechaChave,
            TipoToken::AbreColchete,
            TipoToken::FechaColchete,
            TipoToken::Virgula,
            TipoToken::PontoVirgula,
            TipoToken::DoisPontos,
            TipoToken::Mais,
            TipoToken::Menos,
            TipoToken::Vezes,
            TipoToken::Divide,
            TipoToken::Resto,
            TipoToken::Negacao,
            TipoToken::Atribui,
            TipoToken::Menor,
            TipoToken::Maior,
            TipoToken::Fim,
        ]
    );
}

#[test]
fn preserva_quebras_em_chaves_e_suprime_em_parenteses() {
    let resultado = analisar_lexico(&fonte("{\n x\n}\n(\n y\n)\n"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(
        resultado
            .tokens
            .iter()
            .filter(|token| token.tipo == TipoToken::NovaLinha)
            .count(),
        4
    );
}

#[test]
fn comentario_de_bloco_fecha_no_primeiro_delimitador() {
    let resultado = analisar_lexico(&fonte("a /* externo /* interno */ b"));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(resultado.tokens[0].lexema, "a");
    assert_eq!(resultado.tokens[1].lexema, "b");
}

#[test]
fn entrada_extensa_nao_causa_pane() {
    let codigo = format!("{}\n", "variavel x = 1234567890 ".repeat(10_000));
    let resultado = analisar_lexico(&fonte(&codigo));
    assert!(
        resultado.diagnosticos.is_empty(),
        "{:?}",
        resultado.diagnosticos
    );
    assert_eq!(resultado.tokens.last().unwrap().tipo, TipoToken::Fim);
}
