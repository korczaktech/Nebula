//! Serialização JSON determinística e validação estrita de entrada.

use std::collections::BTreeMap;
use std::fmt;

/// Valor JSON sem tipos implícitos ou extensões não padronizadas.
#[derive(Clone, Debug, PartialEq)]
pub enum ValorJson {
    /// Valor nulo.
    Nulo,
    /// Valor lógico.
    Logico(bool),
    /// Número IEEE-754 finito.
    Numero(f64),
    /// Texto Unicode.
    Texto(String),
    /// Sequência ordenada de valores.
    Lista(Vec<ValorJson>),
    /// Objeto com chaves ordenadas deterministicamente.
    Objeto(BTreeMap<String, ValorJson>),
}

/// Erro de análise ou serialização JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErroJson {
    /// Posição em bytes da entrada onde o erro foi detectado.
    pub deslocamento: usize,
    /// Explicação legível do erro.
    pub mensagem: String,
}

impl fmt::Display for ErroJson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (byte {})", self.mensagem, self.deslocamento)
    }
}

impl std::error::Error for ErroJson {}

const LIMITE_PROFUNDIDADE: usize = 128;

/// Analisa uma entrada JSON UTF-8 e rejeita chaves duplicadas e conteúdo extra.
pub fn analisar_json(entrada: &str) -> Result<ValorJson, ErroJson> {
    let mut parser = Parser { entrada, indice: 0 };
    let valor = parser.valor(0)?;
    parser.espacos();
    if parser.indice != entrada.len() {
        return Err(parser.erro("Conteúdo adicional após o valor JSON."));
    }
    Ok(valor)
}

/// Serializa JSON com ordem determinística de chaves e sem números não finitos.
pub fn serializar_json(valor: &ValorJson) -> Result<String, ErroJson> {
    let mut saida = String::new();
    serializar_valor(valor, &mut saida, 0)?;
    Ok(saida)
}

struct Parser<'a> {
    entrada: &'a str,
    indice: usize,
}

impl Parser<'_> {
    fn erro(&self, mensagem: &str) -> ErroJson {
        ErroJson {
            deslocamento: self.indice,
            mensagem: mensagem.to_owned(),
        }
    }

    fn atual(&self) -> Option<u8> {
        self.entrada.as_bytes().get(self.indice).copied()
    }

    fn espacos(&mut self) {
        while matches!(self.atual(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.indice += 1;
        }
    }

    fn valor(&mut self, profundidade: usize) -> Result<ValorJson, ErroJson> {
        if profundidade > LIMITE_PROFUNDIDADE {
            return Err(self.erro("Profundidade máxima de JSON excedida."));
        }
        self.espacos();
        match self.atual() {
            Some(b'n') => {
                self.literal(b"null")?;
                Ok(ValorJson::Nulo)
            }
            Some(b't') => {
                self.literal(b"true")?;
                Ok(ValorJson::Logico(true))
            }
            Some(b'f') => {
                self.literal(b"false")?;
                Ok(ValorJson::Logico(false))
            }
            Some(b'"') => self.texto().map(ValorJson::Texto),
            Some(b'[') => self.lista(profundidade + 1),
            Some(b'{') => self.objeto(profundidade + 1),
            Some(b'-' | b'0'..=b'9') => self.numero(),
            Some(_) => Err(self.erro("Token JSON inválido.")),
            None => Err(self.erro("Fim inesperado da entrada JSON.")),
        }
    }

    fn literal(&mut self, esperado: &[u8]) -> Result<(), ErroJson> {
        let fim = self.indice.saturating_add(esperado.len());
        if self.entrada.as_bytes().get(self.indice..fim) == Some(esperado) {
            self.indice = fim;
            Ok(())
        } else {
            Err(self.erro("Literal JSON inválido."))
        }
    }

    fn texto(&mut self) -> Result<String, ErroJson> {
        if self.atual() != Some(b'"') {
            return Err(self.erro("Esperado texto JSON."));
        }
        self.indice += 1;
        let mut saida = String::new();
        loop {
            let Some(byte) = self.atual() else {
                return Err(self.erro("Texto JSON não foi fechado."));
            };
            match byte {
                b'"' => {
                    self.indice += 1;
                    return Ok(saida);
                }
                b'\\' => {
                    self.indice += 1;
                    let Some(escape) = self.atual() else {
                        return Err(self.erro("Escape incompleto em texto JSON."));
                    };
                    self.indice += 1;
                    match escape {
                        b'"' => saida.push('"'),
                        b'\\' => saida.push('\\'),
                        b'/' => saida.push('/'),
                        b'b' => saida.push('\u{0008}'),
                        b'f' => saida.push('\u{000c}'),
                        b'n' => saida.push('\n'),
                        b'r' => saida.push('\r'),
                        b't' => saida.push('\t'),
                        b'u' => {
                            let primeiro = self.hex4()?;
                            let escalar = if (0xd800..=0xdbff).contains(&primeiro) {
                                if self.entrada.as_bytes().get(self.indice..self.indice + 2)
                                    != Some(b"\\u")
                                {
                                    return Err(self.erro("Par substituto Unicode incompleto."));
                                }
                                self.indice += 2;
                                let segundo = self.hex4()?;
                                if !(0xdc00..=0xdfff).contains(&segundo) {
                                    return Err(self.erro("Par substituto Unicode inválido."));
                                }
                                0x10000
                                    + ((u32::from(primeiro) - 0xd800) << 10)
                                    + (u32::from(segundo) - 0xdc00)
                            } else if (0xdc00..=0xdfff).contains(&primeiro) {
                                return Err(self.erro("Substituto Unicode isolado."));
                            } else {
                                u32::from(primeiro)
                            };
                            let caractere = char::from_u32(escalar)
                                .ok_or_else(|| self.erro("Escalar Unicode inválido."))?;
                            saida.push(caractere);
                        }
                        _ => return Err(self.erro("Escape JSON desconhecido.")),
                    }
                }
                0x00..=0x1f => return Err(self.erro("Controle não escapado dentro de texto JSON.")),
                0x20..=0x7f => {
                    saida.push(char::from(byte));
                    self.indice += 1;
                }
                _ => {
                    let caractere = self.entrada[self.indice..]
                        .chars()
                        .next()
                        .ok_or_else(|| self.erro("UTF-8 inválido em texto JSON."))?;
                    saida.push(caractere);
                    self.indice += caractere.len_utf8();
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, ErroJson> {
        let mut valor = 0_u16;
        for _ in 0..4 {
            let Some(byte) = self.atual() else {
                return Err(self.erro("Escape Unicode incompleto."));
            };
            let digito = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => return Err(self.erro("Dígito hexadecimal inválido em escape Unicode.")),
            };
            valor = (valor << 4) | u16::from(digito);
            self.indice += 1;
        }
        Ok(valor)
    }

    fn numero(&mut self) -> Result<ValorJson, ErroJson> {
        let inicio = self.indice;
        if self.atual() == Some(b'-') {
            self.indice += 1;
        }
        match self.atual() {
            Some(b'0') => {
                self.indice += 1;
                if matches!(self.atual(), Some(b'0'..=b'9')) {
                    return Err(self.erro("Número JSON não pode conter zero inicial."));
                }
            }
            Some(b'1'..=b'9') => {
                self.indice += 1;
                while matches!(self.atual(), Some(b'0'..=b'9')) {
                    self.indice += 1;
                }
            }
            _ => return Err(self.erro("Parte inteira do número JSON inválida.")),
        }
        if self.atual() == Some(b'.') {
            self.indice += 1;
            let inicio_fracao = self.indice;
            while matches!(self.atual(), Some(b'0'..=b'9')) {
                self.indice += 1;
            }
            if inicio_fracao == self.indice {
                return Err(self.erro("Parte fracionária do número JSON está vazia."));
            }
        }
        if matches!(self.atual(), Some(b'e' | b'E')) {
            self.indice += 1;
            if matches!(self.atual(), Some(b'+' | b'-')) {
                self.indice += 1;
            }
            let inicio_expoente = self.indice;
            while matches!(self.atual(), Some(b'0'..=b'9')) {
                self.indice += 1;
            }
            if inicio_expoente == self.indice {
                return Err(self.erro("Expoente do número JSON está vazio."));
            }
        }
        let numero = self.entrada[inicio..self.indice]
            .parse::<f64>()
            .map_err(|_| self.erro("Número JSON não pôde ser interpretado."))?;
        if !numero.is_finite() {
            return Err(self.erro("Número JSON excede o intervalo finito suportado."));
        }
        Ok(ValorJson::Numero(numero))
    }

    fn lista(&mut self, profundidade: usize) -> Result<ValorJson, ErroJson> {
        self.indice += 1;
        self.espacos();
        let mut itens = Vec::new();
        if self.atual() == Some(b']') {
            self.indice += 1;
            return Ok(ValorJson::Lista(itens));
        }
        loop {
            itens.push(self.valor(profundidade)?);
            self.espacos();
            match self.atual() {
                Some(b',') => {
                    self.indice += 1;
                    self.espacos();
                }
                Some(b']') => {
                    self.indice += 1;
                    return Ok(ValorJson::Lista(itens));
                }
                _ => return Err(self.erro("Esperado ',' ou ']' em lista JSON.")),
            }
        }
    }

    fn objeto(&mut self, profundidade: usize) -> Result<ValorJson, ErroJson> {
        self.indice += 1;
        self.espacos();
        let mut campos = BTreeMap::new();
        if self.atual() == Some(b'}') {
            self.indice += 1;
            return Ok(ValorJson::Objeto(campos));
        }
        loop {
            self.espacos();
            let chave = self.texto()?;
            self.espacos();
            if self.atual() != Some(b':') {
                return Err(self.erro("Esperado ':' após chave de objeto JSON."));
            }
            self.indice += 1;
            let valor = self.valor(profundidade)?;
            if campos.insert(chave, valor).is_some() {
                return Err(self.erro("Chave duplicada em objeto JSON."));
            }
            self.espacos();
            match self.atual() {
                Some(b',') => {
                    self.indice += 1;
                    self.espacos();
                }
                Some(b'}') => {
                    self.indice += 1;
                    return Ok(ValorJson::Objeto(campos));
                }
                _ => return Err(self.erro("Esperado ',' ou '}' em objeto JSON.")),
            }
        }
    }
}

fn serializar_valor(
    valor: &ValorJson,
    saida: &mut String,
    profundidade: usize,
) -> Result<(), ErroJson> {
    if profundidade > LIMITE_PROFUNDIDADE {
        return Err(ErroJson {
            deslocamento: 0,
            mensagem: "Profundidade máxima de serialização JSON excedida.".to_owned(),
        });
    }
    match valor {
        ValorJson::Nulo => saida.push_str("null"),
        ValorJson::Logico(true) => saida.push_str("true"),
        ValorJson::Logico(false) => saida.push_str("false"),
        ValorJson::Numero(numero) if numero.is_finite() => saida.push_str(&numero.to_string()),
        ValorJson::Numero(_) => {
            return Err(ErroJson {
                deslocamento: 0,
                mensagem: "JSON não permite NaN ou infinito.".to_owned(),
            });
        }
        ValorJson::Texto(texto) => serializar_texto(texto, saida),
        ValorJson::Lista(itens) => {
            saida.push('[');
            for (indice, item) in itens.iter().enumerate() {
                if indice > 0 {
                    saida.push(',');
                }
                serializar_valor(item, saida, profundidade + 1)?;
            }
            saida.push(']');
        }
        ValorJson::Objeto(campos) => {
            saida.push('{');
            for (indice, (chave, valor)) in campos.iter().enumerate() {
                if indice > 0 {
                    saida.push(',');
                }
                serializar_texto(chave, saida);
                saida.push(':');
                serializar_valor(valor, saida, profundidade + 1)?;
            }
            saida.push('}');
        }
    }
    Ok(())
}

fn serializar_texto(texto: &str, saida: &mut String) {
    use std::fmt::Write as _;
    saida.push('"');
    for caractere in texto.chars() {
        match caractere {
            '"' => saida.push_str("\\\""),
            '\\' => saida.push_str("\\\\"),
            '\u{0008}' => saida.push_str("\\b"),
            '\u{000c}' => saida.push_str("\\f"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            c if c <= '\u{001f}' => {
                let _ = write!(saida, "\\u{:04x}", u32::from(c));
            }
            c => saida.push(c),
        }
    }
    saida.push('"');
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn analisa_e_serializa_objeto_de_forma_deterministica() {
        let valor = analisar_json(r#"{"z":2,"a":[true,null,"Nebula"]}"#).unwrap();
        assert_eq!(
            serializar_json(&valor).unwrap(),
            r#"{"a":[true,null,"Nebula"],"z":2}"#
        );
    }

    #[test]
    fn trata_escape_unicode_e_pares_substitutos() {
        let valor = analisar_json(r#""\u0041\uD83E\uDE80""#).unwrap();
        assert_eq!(valor, ValorJson::Texto("A🪀".to_owned()));
        assert_eq!(
            serializar_json(&ValorJson::Texto("linha\n\t\"".to_owned())).unwrap(),
            r#""linha\n\t\"""#
        );
    }

    #[test]
    fn rejeita_chaves_duplicadas_e_numeros_invalidos() {
        assert!(analisar_json(r#"{"x":1,"x":2}"#).is_err());
        for invalido in ["01", "1.", "1e", ".5", "NaN", "--1"] {
            assert!(analisar_json(invalido).is_err(), "{invalido}");
        }
    }

    #[test]
    fn json_parser_resiste_a_entradas_adversariais_deterministicas() {
        for semente_inicial in 0..512_u32 {
            let mut estado = semente_inicial.wrapping_add(0x9e37_79b9);
            let mut bytes = Vec::with_capacity((semente_inicial as usize % 97) + 1);
            let tamanho = semente_inicial as usize % 97 + 1;
            for _ in 0..tamanho {
                estado = estado.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                bytes.push((estado >> 24) as u8);
            }
            let entrada = String::from_utf8_lossy(&bytes);
            if let Ok(valor) = analisar_json(&entrada) {
                let serializado =
                    serializar_json(&valor).expect("valor analisado deve ser serializável");
                let novamente =
                    analisar_json(&serializado).expect("serialização válida deve ser analisável");
                assert_eq!(valor, novamente);
            }
        }
    }

    #[test]
    fn rejeita_conteudo_adicional_e_limites_excedidos() {
        assert!(analisar_json("true false").is_err());
        let profundo = format!(
            "{}0{}",
            "[".repeat(LIMITE_PROFUNDIDADE + 2),
            "]".repeat(LIMITE_PROFUNDIDADE + 2)
        );
        assert!(analisar_json(&profundo).is_err());
    }

    #[test]
    fn rejeita_numero_nao_finito_durante_serializacao() {
        assert!(serializar_json(&ValorJson::Numero(f64::INFINITY)).is_err());
    }
}
