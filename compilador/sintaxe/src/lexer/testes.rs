//! Testes de conformidade do Lexer Nebula.

use nebula_fonte::ArquivoFonte;

use super::{analisar_lexico, TipoToken};

fn fonte(codigo: &str) -> ArquivoFonte {
    ArquivoFonte::novo("lexer_teste.nbx", codigo)
}

#[test]
fn reconhece_palavras_reservadas_e_identificadores_unicode() {
    let resultado = analisar_lexico(&fonte("variavel ação = verdadeiro"));
    assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    assert_eq!(resultado.tokens[0].tipo, TipoToken::Variavel);
    assert_eq!(resultado.tokens[1].lexema, "ação");
    assert_eq!(resultado.tokens[3].tipo, TipoToken::Verdadeiro);
}

#[test]
fn reconhece_grafias_com_e_sem_acentos() {
    let resultado = analisar_lexico(&fonte("funcao função senao senão nao não publico público enumeracao enumeração"));
    assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    assert_eq!(resultado.tokens.iter().filter(|t| t.tipo == TipoToken::Funcao).count(), 2);
    assert_eq!(resultado.tokens.iter().filter(|t| t.tipo == TipoToken::Senao).count(), 2);
    assert_eq!(resultado.tokens.iter().filter(|t| t.tipo == TipoToken::Nao).count(), 2);
}

#[test]
fn reconhece_numeros_decimais_e_em_bases() {
    let resultado = analisar_lexico(&fonte("42 1_000 3.14 2e10 0xFF 0b101 0o77"));
    assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    assert_eq!(resultado.tokens.iter().filter(|t| t.tipo == TipoToken::Inteiro).count(), 5);
    assert_eq!(resultado.tokens.iter().filter(|t| t.tipo == TipoToken::Decimal).count(), 2);
}

#[test]
fn diagnostica_numero_em_base_invalido() {
    let resultado = analisar_lexico(&fonte("0b102"));
    assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N1005"));
}

#[test]
fn reconhece_textos_caracteres_e_escapes() {
    let resultado = analisar_lexico(&fonte("\"Nebula\\n\" 'á' '\\u{1F680}'"));
    assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    assert_eq!(resultado.tokens[0].tipo, TipoToken::Texto);
    assert_eq!(resultado.tokens[1].tipo, TipoToken::Caractere);
    assert_eq!(resultado.tokens[2].tipo, TipoToken::Caractere);
}

#[test]
fn diagnostica_texto_nao_terminado() {
    let resultado = analisar_lexico(&fonte("\"texto sem fim"));
    assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N1008"));
}

#[test]
fn reconhece_operadores_pelo_maior_prefixo() {
    let resultado = analisar_lexico(&fonte("== != <= >= <- ** .. :: = < >"));
    assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    let tipos: Vec<_> = resultado.tokens.iter().map(|t| t.tipo).collect();
    assert_eq!(tipos[..11], [
        TipoToken::Igual, TipoToken::Diferente, TipoToken::MenorIgual,
        TipoToken::MaiorIgual, TipoToken::Atribui, TipoToken::Potencia,
        TipoToken::Intervalo, TipoToken::Escopo, TipoToken::Atribui,
        TipoToken::Menor, TipoToken::Maior,
    ]);
}

#[test]
fn preserva_quebras_de_linha_e_suprime_quebras_em_parenteses() {
    let resultado = analisar_lexico(&fonte("a\nfuncao f(x,\n y)\n"));
    assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    assert_eq!(resultado.tokens.iter().filter(|t| t.tipo == TipoToken::NovaLinha).count(), 2);
}

#[test]
fn reconhece_crlf_como_uma_quebra() {
    let resultado = analisar_lexico(&fonte("a\r\nb"));
    assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    assert_eq!(resultado.tokens.iter().filter(|t| t.tipo == TipoToken::NovaLinha).count(), 1);
}

#[test]
fn rejeita_cr_isolado() {
    let resultado = analisar_lexico(&fonte("a\rb"));
    assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N1001"));
}

#[test]
fn reconhece_comentarios_e_preserva_quebras_em_bloco() {
    let resultado = analisar_lexico(&fonte("a // linha\n/* bloco\ncontinua */ b"));
    assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    assert_eq!(resultado.tokens.iter().filter(|t| t.tipo == TipoToken::NovaLinha).count(), 2);
}

#[test]
fn diagnostica_comentario_de_bloco_nao_terminado() {
    let resultado = analisar_lexico(&fonte("a /* sem fechamento"));
    assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N1002"));
}

#[test]
fn preserva_intervalos_em_bytes_para_unicode() {
    let resultado = analisar_lexico(&fonte("ação"));
    assert_eq!(resultado.tokens[0].intervalo.inicio, 0);
    assert_eq!(resultado.tokens[0].intervalo.fim, "ação".len());
    assert_eq!(resultado.tokens[0].lexema, "ação");
}

#[test]
fn sempre_encerra_com_token_fim() {
    let resultado = analisar_lexico(&fonte(""));
    assert_eq!(resultado.tokens.len(), 1);
    assert_eq!(resultado.tokens[0].tipo, TipoToken::Fim);
}
