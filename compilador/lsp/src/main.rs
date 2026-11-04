//! Servidor Language Server Protocol da linguagem Nebula.
//!
//! Transporte JSON-RPC 2.0 por stdin/stdout, conforme o enquadramento LSP
//! Content-Length. Mensagens de diagnóstico e logs nunca usam stdout.

#![forbid(unsafe_code)]

#[rustfmt::skip]
mod implementacao {
use nebula_fonte::{ArquivoFonte, IntervaloFonte};
use nebula_sintaxe::{analisar, analisar_lexico, NoAst, Programa, TipoToken};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};

#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self { Self::Object(map) => map.get(key), _ => None }
    }
    fn string(&self) -> Option<&str> {
        match self { Self::String(value) => Some(value), _ => None }
    }
    fn number(&self) -> Option<i64> {
        match self { Self::Number(value) => Some(*value), _ => None }
    }
    fn encode(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Self::Number(value) => out.push_str(&value.to_string()),
            Self::String(value) => quote(value, out),
            Self::Array(values) => {
                out.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 { out.push(','); }
                    value.encode(out);
                }
                out.push(']');
            }
            Self::Object(values) => {
                out.push('{');
                for (index, (key, value)) in values.iter().enumerate() {
                    if index > 0 { out.push(','); }
                    quote(key, out);
                    out.push(':');
                    value.encode(out);
                }
                out.push('}');
            }
        }
    }
}

fn quote(value: &str, out: &mut String) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
}

struct JsonParser<'a> { bytes: &'a [u8], at: usize }
impl<'a> JsonParser<'a> {
    fn parse(input: &'a str) -> Result<Json, String> {
        let mut parser = Self { bytes: input.as_bytes(), at: 0 };
        let value = parser.value()?;
        parser.space();
        if parser.at != parser.bytes.len() { return Err("Dados após o JSON.".into()); }
        Ok(value)
    }
    fn space(&mut self) { while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) { self.at += 1; } }
    fn value(&mut self) -> Result<Json, String> {
        self.space();
        match self.bytes.get(self.at).copied() {
            Some(b'n') => { self.word(b"null")?; Ok(Json::Null) }
            Some(b't') => { self.word(b"true")?; Ok(Json::Bool(true)) }
            Some(b'f') => { self.word(b"false")?; Ok(Json::Bool(false)) }
            Some(b'"') => self.string().map(Json::String),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err("Valor JSON inválido.".into()),
        }
    }
    fn word(&mut self, word: &[u8]) -> Result<(), String> {
        if self.bytes.get(self.at..self.at + word.len()) == Some(word) { self.at += word.len(); Ok(()) }
        else { Err("Literal JSON inválido.".into()) }
    }
    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut result = String::new();
        while let Some(byte) = self.bytes.get(self.at).copied() {
            self.at += 1;
            match byte {
                b'"' => return Ok(result),
                b'\\' => {
                    let escaped = *self.bytes.get(self.at).ok_or("Escape incompleto.")?;
                    self.at += 1;
                    match escaped {
                        b'"' => result.push('"'), b'\\' => result.push('\\'),
                        b'/' => result.push('/'), b'b' => result.push('\u{0008}'),
                        b'f' => result.push('\u{000c}'), b'n' => result.push('\n'),
                        b'r' => result.push('\r'), b't' => result.push('\t'),
                        b'u' => {
                            let end = self.at.checked_add(4).ok_or("Escape Unicode inválido.")?;
                            let digits = self.bytes.get(self.at..end).ok_or("Escape Unicode incompleto.")?;
                            let text = std::str::from_utf8(digits).map_err(|_| "Escape Unicode inválido.")?;
                            let code = u16::from_str_radix(text, 16).map_err(|_| "Escape Unicode inválido.")?;
                            self.at = end;
                            let ch = char::from_u32(code as u32).ok_or("Escape Unicode inválido.")?;
                            result.push(ch);
                        }
                        _ => return Err("Escape JSON inválido.".into()),
                    }
                }
                0..=31 => return Err("Caractere de controle em string JSON.".into()),
                _ => {
                    let start = self.at - 1;
                    let tail = std::str::from_utf8(&self.bytes[start..]).map_err(|_| "UTF-8 inválido.")?;
                    let ch = tail.chars().next().ok_or("String incompleta.")?;
                    result.push(ch);
                    self.at = start + ch.len_utf8();
                }
            }
        }
        Err("String JSON não terminada.".into())
    }
    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        if self.bytes[self.at] == b'-' { self.at += 1; }
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) { self.at += 1; }
        let value = std::str::from_utf8(&self.bytes[start..self.at]).map_err(|_| "Número inválido.")?
            .parse::<i64>().map_err(|_| "Número fora do limite.")?;
        Ok(Json::Number(value))
    }
    fn array(&mut self) -> Result<Json, String> {
        self.at += 1; self.space();
        let mut values = Vec::new();
        if self.bytes.get(self.at) == Some(&b']') { self.at += 1; return Ok(Json::Array(values)); }
        loop {
            values.push(self.value()?); self.space();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b']') => { self.at += 1; break; }
                _ => return Err("Esperado ',' ou ']'.".into()),
            }
        }
        Ok(Json::Array(values))
    }
    fn object(&mut self) -> Result<Json, String> {
        self.at += 1; self.space();
        let mut values = BTreeMap::new();
        if self.bytes.get(self.at) == Some(&b'}') { self.at += 1; return Ok(Json::Object(values)); }
        loop {
            self.space();
            if self.bytes.get(self.at) != Some(&b'"') { return Err("Chave JSON inválida.".into()); }
            let key = self.string()?; self.space();
            if self.bytes.get(self.at) != Some(&b':') { return Err("Esperado ':'.".into()); }
            self.at += 1;
            values.insert(key, self.value()?); self.space();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b'}') => { self.at += 1; break; }
                _ => return Err("Esperado ',' ou '}'.".into()),
            }
        }
        Ok(Json::Object(values))
    }
}

#[derive(Clone)]
struct Document { text: String, version: i64 }
#[derive(Default)]
struct Server { documents: BTreeMap<String, Document>, exit: bool }

fn object(entries: impl IntoIterator<Item = (String, Json)>) -> Json {
    Json::Object(entries.into_iter().collect())
}
fn s(value: impl Into<String>) -> Json { Json::String(value.into()) }
fn n(value: usize) -> Json { Json::Number(value as i64) }
fn response(id: Json, result: Json) -> Json {
    object([("jsonrpc".into(), s("2.0")), ("id".into(), id), ("result".into(), result)])
}
fn notification(method: &str, params: Json) -> Json {
    object([("jsonrpc".into(), s("2.0")), ("method".into(), s(method)), ("params".into(), params)])
}
fn position(offset: usize, text: &str) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let column = before.rsplit('\n').next().unwrap_or("").encode_utf16().count();
    (line, column)
}
fn offset(text: &str, line: usize, character: usize) -> usize {
    let mut current_line = 0;
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        if current_line == line { break; }
        if ch == '\n' { current_line += 1; start = index + 1; }
    }
    if current_line < line { return text.len(); }
    let mut units = 0;
    for (index, ch) in text[start..].char_indices() {
        if ch == '\n' || units >= character { return start + index; }
        units += ch.len_utf16();
        if units > character { return start + index; }
    }
    text.len()
}
fn range(interval: Option<IntervaloFonte>, text: &str) -> Json {
    let (start, end) = interval.map_or(((0, 0), (0, 1)), |span| (position(span.inicio, text), position(span.fim, text)));
    object([
        ("start".into(), object([("line".into(), n(start.0)), ("character".into(), n(start.1))])),
        ("end".into(), object([("line".into(), n(end.0)), ("character".into(), n(end.1))])),
    ])
}
fn diagnostic_interval(message: &str, text: &str) -> Option<IntervaloFonte> {
    let first = message.find('\'')?;
    let tail = &message[first + 1..];
    let end = tail.find('\'')?;
    let name = &tail[..end];
    if name.is_empty() { return None; }
    let start = text.find(name)?;
    IntervaloFonte::novo(start, start + name.len())
}
fn diagnostics(uri: &str, document: &Document) -> Json {
    let file = ArquivoFonte::novo(uri, document.text.clone());
    let result = analisar(&file);
    let mut collected = result.diagnosticos;
    collected.extend(nebula_semantica::analisar_semantica(&result.programa).diagnosticos);
    let items = collected.iter().map(|diag| {
        let severity = match diag.severidade {
            nebula_diagnosticos::Severidade::Erro => 1,
            nebula_diagnosticos::Severidade::Aviso => 2,
            nebula_diagnosticos::Severidade::Informacao => 3,
        };
        object([
            ("range".into(), range(diag.intervalo.or_else(|| diagnostic_interval(&diag.mensagem, &document.text)), &document.text)),
            ("severity".into(), n(severity)),
            ("code".into(), s(diag.codigo)),
            ("source".into(), s("nebula")),
            ("message".into(), s(diag.mensagem.clone())),
        ])
    }).collect();
    notification("textDocument/publishDiagnostics", object([
        ("uri".into(), s(uri)),
        ("version".into(), Json::Number(document.version)),
        ("diagnostics".into(), Json::Array(items)),
    ]))
}
fn keywords() -> &'static [&'static str] {
    &["variavel", "definir", "mutavel", "constante", "alterar", "funcao", "quando", "senao", "escolha", "avaliar", "caso", "para", "cada", "em", "pulso", "enquanto", "durante", "repetir", "devolver", "incluir", "como", "verdadeiro", "falso", "nulo", "parar", "encerrar", "seguir", "e", "ou", "nao", "publico", "privado", "tipo", "estrutura", "enumeracao", "exibir"]
}
fn word_at(text: &str, at: usize) -> (usize, usize, String) {
    let at = at.min(text.len());
    let start = text[..at].char_indices().rev().find(|(_, ch)| !(*ch == '_' || ch.is_alphanumeric())).map_or(0, |(i, ch)| i + ch.len_utf8());
    let end = text[at..].char_indices().find(|(_, ch)| !(*ch == '_' || ch.is_alphanumeric())).map_or(text.len(), |(i, _)| at + i);
    (start, end, text[start..end].to_owned())
}
fn completion(document: &Document, uri: &str, line: usize, character: usize) -> Json {
    let at = offset(&document.text, line, character);
    let (_, _, prefix) = word_at(&document.text, at);
    let mut items = Vec::new();
    for keyword in keywords().iter().filter(|word| word.starts_with(&prefix)) {
        items.push(object([
            ("label".into(), s(*keyword)),
            ("kind".into(), n(14)),
            ("detail".into(), s("Palavra-chave Nebula")),
        ]));
    }
    let file = ArquivoFonte::novo(uri, document.text.clone());
    let parsed = analisar(&file);
    fn visit(nodes: &[NoAst], items: &mut Vec<Json>, prefix: &str) {
        for node in nodes {
            match node {
                NoAst::Variavel { nome, .. } | NoAst::Tipo { nome, .. } | NoAst::Estrutura { nome, .. } | NoAst::Enumeracao { nome, .. } => {
                    if nome.starts_with(prefix) {
                        items.push(object([("label".into(), s(nome)), ("kind".into(), n(6)), ("detail".into(), s("Símbolo do arquivo"))]));
                    }
                }
                NoAst::Funcao { nome, parametros, corpo, .. } => {
                    if nome.starts_with(prefix) {
                        items.push(object([("label".into(), s(nome)), ("kind".into(), n(3)),
                            ("detail".into(), s(format!("Função Nebula — {} parâmetro(s)", parametros.len())))]));
                    }
                    for (parametro, _) in parametros {
                        if parametro.starts_with(prefix) {
                            items.push(object([("label".into(), s(parametro)), ("kind".into(), n(6)),
                                ("detail".into(), s("Parâmetro da função"))]));
                        }
                    }
                    visit(corpo, items, prefix);
                }
                NoAst::Quando { entao, senao, .. } => { visit(entao, items, prefix); visit(senao, items, prefix); }
                NoAst::Enquanto { corpo, .. } | NoAst::Cada { corpo, .. } | NoAst::Pulso { corpo, .. } => visit(corpo, items, prefix),
                NoAst::Para { inicial, corpo, .. } => {
                    if let Some(inicial) = inicial { visit(std::slice::from_ref(inicial.as_ref()), items, prefix); }
                    visit(corpo, items, prefix);
                }
                NoAst::Escolha { casos, .. } => {
                    for caso in casos { visit(&caso.corpo, items, prefix); }
                }
                _ => {}
            }
        }
    }
    visit(&parsed.programa.itens, &mut items, &prefix);
    object([("isIncomplete".into(), Json::Bool(false)), ("items".into(), Json::Array(items))])
}
fn declared_name(node: &NoAst) -> Option<(&str, usize)> {
    match node {
        NoAst::Variavel { nome, .. } => Some((nome, 13)),
        NoAst::Funcao { nome, .. } => Some((nome, 12)),
        NoAst::Tipo { nome, .. } | NoAst::Estrutura { nome, .. } => Some((nome, 23)),
        NoAst::Enumeracao { nome, .. } => Some((nome, 10)),
        NoAst::Incluir { apelido: Some(nome), .. } => Some((nome, 2)),
        _ => None,
    }
}
fn find_identifier(fragment: &str, name: &str) -> Option<usize> {
    fragment.match_indices(name).find_map(|(start, matched)| {
        let before = fragment[..start].chars().next_back();
        let after = fragment[start + matched.len()..].chars().next();
        let valid_before = before.is_none_or(|ch| ch != '_' && !ch.is_alphanumeric());
        let valid_after = after.is_none_or(|ch| ch != '_' && !ch.is_alphanumeric());
        (valid_before && valid_after).then_some(start)
    })
}
fn declaration_locations(program: &Programa, text: &str, uri: &str) -> Vec<(String, usize, usize, usize)> {
    let mut locations = Vec::new();
    for located in &program.nos_localizados {
        let Some((name, kind)) = declared_name(&located.no) else { continue; };
        let Some(fragment) = text.get(located.intervalo.inicio..located.intervalo.fim) else { continue; };
        let Some(relative) = find_identifier(fragment, name) else { continue; };
        let start = located.intervalo.inicio + relative;
        locations.push((name.to_owned(), start, start + name.len(), kind));
        if let NoAst::Funcao { parametros, .. } = &located.no {
            for (parameter, _) in parametros {
                if let Some(relative) = find_identifier(fragment, parameter) {
                    let start = located.intervalo.inicio + relative;
                    locations.push((parameter.clone(), start, start + parameter.len(), 6));
                }
            }
        }
    }
    let _ = uri;
    locations
}
fn location_json(uri: &str, text: &str, start: usize, end: usize) -> Json {
    object([
        ("uri".into(), s(uri)),
        ("range".into(), range(IntervaloFonte::novo(start, end), text)),
    ])
}
fn symbols(program: &Programa, text: &str, uri: &str) -> Json {
    Json::Array(declaration_locations(program, text, uri).into_iter().map(|(name, start, end, kind)| {
        object([
            ("name".into(), s(name)),
            ("kind".into(), n(kind)),
            ("range".into(), range(IntervaloFonte::novo(start, end), text)),
            ("selectionRange".into(), range(IntervaloFonte::novo(start, end), text)),
        ])
    }).collect())
}
fn signature_help(document: &Document, uri: &str, line: usize, character: usize) -> Json {
    let at = offset(&document.text, line, character);
    let Some(open) = document.text[..at].rfind('(') else { return Json::Null; };
    let (_, _, function_name) = word_at(&document.text, open);
    if function_name.is_empty() { return Json::Null; }
    let active_parameter = document.text[open + 1..at].chars().filter(|ch| *ch == ',').count();
    let file = ArquivoFonte::novo(uri, document.text.clone());
    let parsed = analisar(&file);
    let Some((parameters, return_type)) = parsed.programa.itens.iter().find_map(|node| {
        if let NoAst::Funcao { nome, parametros, retorno, .. } = node {
            (nome == &function_name).then(|| (parametros, retorno))
        } else {
            None
        }
    }) else { return Json::Null; };
    let parameter_labels: Vec<String> = parameters.iter()
        .map(|(name, tipo)| format!("{name}: {tipo}"))
        .collect();
    let label = format!(
        "{}({}): {}",
        function_name,
        parameter_labels.join(", "),
        return_type.as_deref().unwrap_or("vazio")
    );
    let parameters_json = parameter_labels.into_iter().map(|label| {
        object([("label".into(), s(label))])
    }).collect();
    object([
        ("signatures".into(), Json::Array(vec![object([
            ("label".into(), s(label)),
            ("parameters".into(), Json::Array(parameters_json)),
        ])])),
        ("activeSignature".into(), n(0)),
        ("activeParameter".into(), n(active_parameter)),
    ])
}
fn references(text: &str, uri: &str, word: &str) -> Json {
    let file = ArquivoFonte::novo(uri, text.to_owned());
    let lexical = analisar_lexico(&file);
    Json::Array(lexical.tokens.into_iter()
        .filter(|token| token.tipo == TipoToken::Identificador && token.lexema == word)
        .map(|token| location_json(uri, text, token.intervalo.inicio, token.intervalo.fim))
        .collect())
}
fn handle(server: &mut Server, message: Json) -> Vec<Json> {
    let method = message.get("method").and_then(Json::string).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Json::Null);
    let id = message.get("id").cloned();
    let mut output = Vec::new();
    match method {
        "initialize" => {
            let capabilities = object([
                ("textDocumentSync".into(), n(2)),
                ("completionProvider".into(), object([("resolveProvider".into(), Json::Bool(false))])),
                ("hoverProvider".into(), Json::Bool(true)),\n                ("signatureHelpProvider".into(), object([("triggerCharacters".into(), Json::Array(vec![s("("), s(",")]))])),
                ("definitionProvider".into(), Json::Bool(true)),
                ("referencesProvider".into(), Json::Bool(true)),
                ("documentSymbolProvider".into(), Json::Bool(true)),
            ]);
            if let Some(id) = id {
                output.push(response(id, object([
                    ("capabilities".into(), capabilities),
                    ("serverInfo".into(), object([("name".into(), s("Nebula Language Server")), ("version".into(), s(env!("CARGO_PKG_VERSION")))])),
                ])));
            }
        }
        "initialized" | "$/setTrace" | "workspace/didChangeConfiguration" => {}
        "textDocument/didOpen" | "textDocument/didChange" => {
            let document = params.get("textDocument").unwrap_or(&Json::Null);
            let uri = document.get("uri").and_then(Json::string).unwrap_or("").to_owned();
            if !uri.is_empty() {
                let version = document.get("version").and_then(Json::number).unwrap_or(0);
                let text = if method.ends_with("didOpen") {
                    document.get("text").and_then(Json::string).unwrap_or("").to_owned()
                } else {
                    params.get("contentChanges").and_then(|v| match v { Json::Array(a) => a.last(), _ => None })
                        .and_then(|v| v.get("text")).and_then(Json::string).unwrap_or("").to_owned()
                };
                let doc = Document { text, version };
                server.documents.insert(uri.clone(), doc.clone());
                output.push(diagnostics(&uri, &doc));
            }
        }
        "textDocument/didClose" => {
            let uri = params.get("textDocument").and_then(|v| v.get("uri")).and_then(Json::string).unwrap_or("").to_owned();
            server.documents.remove(&uri);
            output.push(notification("textDocument/publishDiagnostics", object([
                ("uri".into(), s(uri)), ("diagnostics".into(), Json::Array(Vec::new())),
            ])));
        }
        "textDocument/completion" | "textDocument/hover" | "textDocument/definition" | "textDocument/references" | "textDocument/documentSymbol" | "textDocument/signatureHelp" => {
            let uri = params.get("textDocument").and_then(|v| v.get("uri")).and_then(Json::string).unwrap_or("");
            let result = server.documents.get(uri).map(|doc| match method {
                "textDocument/completion" => {
                    let pos = params.get("position").unwrap_or(&Json::Null);
                    completion(doc, uri, pos.get("line").and_then(Json::number).unwrap_or(0) as usize,
                        pos.get("character").and_then(Json::number).unwrap_or(0) as usize)
                }
                "textDocument/signatureHelp" => {
                    let pos = params.get("position").unwrap_or(&Json::Null);
                    signature_help(
                        doc,
                        uri,
                        pos.get("line").and_then(Json::number).unwrap_or(0) as usize,
                        pos.get("character").and_then(Json::number).unwrap_or(0) as usize,
                    )
                }
                "textDocument/documentSymbol" => {
                    let file = ArquivoFonte::novo(uri, doc.text.clone());
                    symbols(&analisar(&file).programa, &doc.text, uri)
                }
                "textDocument/hover" => {
                    let pos = params.get("position").unwrap_or(&Json::Null);
                    let at = offset(&doc.text, pos.get("line").and_then(Json::number).unwrap_or(0) as usize,
                        pos.get("character").and_then(Json::number).unwrap_or(0) as usize);
                    let (_, _, word) = word_at(&doc.text, at);
                    if word.is_empty() {
                        Json::Null
                    } else {
                        let file = ArquivoFonte::novo(uri, doc.text.clone());
                        let parsed = analisar(&file);
                        let mut detail = if keywords().contains(&word.as_str()) {
                            format!("Palavra-chave da linguagem Nebula: `{word}`.")
                        } else {
                            format!("Identificador `{word}` no documento atual.")
                        };
                        if let Some(node) = parsed.programa.itens.iter().find(|node| {
                            declared_name(node).is_some_and(|(name, _)| name == word.as_str())
                        }) {
                            match node {
                                NoAst::Funcao { parametros, retorno, .. } => {
                                    let args = parametros.iter().map(|(name, tipo)| format!("{name}: {tipo}"))
                                        .collect::<Vec<_>>().join(", ");
                                    let result = retorno.as_deref().unwrap_or("vazio");
                                    detail = format!("**Função Nebula**\n\n`funcao {word}({args}): {result}`");
                                }
                                NoAst::Variavel { tipo: Some(tipo), .. } => {
                                    detail = format!("**Variável Nebula**\n\n`{word}: {tipo}`");
                                }
                                NoAst::Tipo { destino, .. } => {
                                    detail = format!("**Alias de tipo**\n\n`tipo {word} = {destino}`");
                                }
                                NoAst::Estrutura { .. } => detail = format!("**Estrutura Nebula**\n\n`estrutura {word}`"),
                                NoAst::Enumeracao { .. } => detail = format!("**Enumeração Nebula**\n\n`enumeracao {word}`"),
                                _ => {}
                            }
                        } else {
                            let semantica = nebula_semantica::analisar_semantica(&parsed.programa);
                            if let Some(tipo) = semantica.tipos_globais.get(&word) {
                                detail = format!("**Variável Nebula**\n\n`{word}: {tipo:?}`");
                            }
                        }
                        object([("contents".into(), object([
                            ("kind".into(), s("markdown")),
                            ("value".into(), s(detail)),
                        ]))])
                    }
                }
                "textDocument/definition" | "textDocument/references" => {
                    let pos = params.get("position").unwrap_or(&Json::Null);
                    let at = offset(&doc.text, pos.get("line").and_then(Json::number).unwrap_or(0) as usize,
                        pos.get("character").and_then(Json::number).unwrap_or(0) as usize);
                    let (_, _, word) = word_at(&doc.text, at);
                    if word.is_empty() { Json::Null } else if method == "textDocument/references" {
                        references(&doc.text, uri, &word)
                    } else {
                        let file = ArquivoFonte::novo(uri, doc.text.clone());
                        let parsed = analisar(&file);
                        declaration_locations(&parsed.programa, &doc.text, uri).into_iter()
                            .find(|(name, _, _, _)| name == &word)
                            .map(|(_, start, end, _)| Json::Array(vec![location_json(uri, &doc.text, start, end)]))
                            .unwrap_or(Json::Null)
                    }
                }
                _ => Json::Null,
            }).unwrap_or(Json::Null);
            if let Some(id) = id { output.push(response(id, result)); }
        }
        "shutdown" => {
            if let Some(id) = id { output.push(response(id, Json::Null)); }
        }
        "exit" => { server.exit = true; }
        _ => {
            if let Some(id) = id {
                output.push(object([
                    ("jsonrpc".into(), s("2.0")),
                    ("id".into(), id),
                    ("error".into(), object([("code".into(), Json::Number(-32601)), ("message".into(), s("Método não implementado."))])),
                ]));
            }
        }
    }
    output
}

fn write_message(stdout: &mut impl Write, message: &Json) -> io::Result<()> {
    let mut body = String::new();
    message.encode(&mut body);
    write!(stdout, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    stdout.flush()
}
fn read_message(reader: &mut impl Read) -> io::Result<Option<String>> {
    let mut headers = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if reader.read(&mut byte)? == 0 {
            return if headers.is_empty() { Ok(None) } else { Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Cabeçalho LSP incompleto.")) };
        }
        headers.push(byte[0]);
        if headers.ends_with(b"\r\n\r\n") { break; }
        if headers.len() > 8192 { return Err(io::Error::new(io::ErrorKind::InvalidData, "Cabeçalho LSP excede 8 KiB.")); }
    }
    let header = std::str::from_utf8(&headers).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Cabeçalho não UTF-8."))?;
    let length = header.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().ok()).flatten()
    }).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Content-Length ausente ou inválido."))?;
    if length > 16 * 1024 * 1024 { return Err(io::Error::new(io::ErrorKind::InvalidData, "Mensagem LSP excede 16 MiB.")); }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    String::from_utf8(body).map(Some).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "JSON LSP não UTF-8."))
}
fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    let mut server = Server::default();
    while let Some(body) = read_message(&mut input)? {
        let message = match JsonParser::parse(&body) {
            Ok(message) => message,
            Err(error) => {
                eprintln!("Nebula LSP: mensagem JSON inválida: {error}");
                continue;
            }
        };
        for reply in handle(&mut server, message) { write_message(&mut output, &reply)?; }
        if server.exit { break; }
    }
    Ok(())
}
pub(super) fn iniciar() {
    if let Err(error) = run() {
        eprintln!("Nebula LSP: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{Document, Json, JsonParser, Server, completion, declaration_locations, handle, object, offset, position, quote, read_message, references, s, signature_help};
    use nebula_fonte::ArquivoFonte;
    use nebula_sintaxe::analisar;
    use std::io::Cursor;

    #[test]
    fn json_roundtrip_handles_unicode_and_escapes() {
        let input = r#"{"mensagem":"ação\\nNebula","ativo":true,"itens":[1,null]}"#;
        let parsed = JsonParser::parse(input).expect("JSON válido");
        let mut encoded = String::new();
        parsed.encode(&mut encoded);
        assert_eq!(JsonParser::parse(&encoded).expect("JSON serializado"), parsed);
    }

    #[test]
    fn json_rejects_trailing_data_and_invalid_escape() {
        assert!(JsonParser::parse("{} lixo").is_err());
        assert!(JsonParser::parse(r#"{"x":"\q"}"#).is_err());
    }

    #[test]
    fn lsp_positions_use_utf16_columns() {
        let text = "ação\n😀x";
        assert_eq!(position("ação\n😀".len(), text), (1, 2));
        assert_eq!(offset(text, 1, 2), "ação\n😀".len());
    }

    #[test]
    fn initialize_advertises_editor_capabilities() {
        let mut server = Server::default();
        let request = object([
            ("jsonrpc".into(), s("2.0")),
            ("id".into(), Json::Number(1)),
            ("method".into(), s("initialize")),
            ("params".into(), object([])),
        ]);
        let output = handle(&mut server, request);
        assert_eq!(output.len(), 1);
        let capabilities = output[0].get("result").and_then(|value| value.get("capabilities"));
        assert!(capabilities.and_then(|value| value.get("completionProvider")).is_some());
        assert!(capabilities.and_then(|value| value.get("definitionProvider")).is_some());
        assert!(capabilities.and_then(|value| value.get("signatureHelpProvider")).is_some());
    }

    #[test]
    fn document_open_publishes_compiler_diagnostics() {
        let mut server = Server::default();
        let request = object([
            ("jsonrpc".into(), s("2.0")),
            ("method".into(), s("textDocument/didOpen")),
            ("params".into(), object([
                ("textDocument".into(), object([
                    ("uri".into(), s("file:///teste.nbx")),
                    ("version".into(), Json::Number(1)),
                    ("text".into(), s("variavel valor = 1")),
                ])),
            ])),
        ]);
        let output = handle(&mut server, request);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].get("method").and_then(Json::string), Some("textDocument/publishDiagnostics"));
    }

    #[test]
    fn references_ignore_text_and_comments() {
        let found = references("variavel valor = 1\n// valor\nexibir(valor)", "file:///teste.nbx", "valor");
        let Json::Array(items) = found else { panic!("resultado deveria ser uma lista"); };
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn signature_help_returns_function_parameters_and_active_argument() {
        let text = "funcao somar(a: inteiro, b: inteiro) { devolver a }\\nvariavel resultado = somar(1, ";
        let document = Document { text: text.to_owned(), version: 1 };
        let result = signature_help(&document, "file:///teste.nbx", 1, text.lines().last().unwrap().encode_utf16().count());
        let Json::Array(signatures) = result.get("signatures").expect("assinatura") else {
            panic!("assinaturas deveriam ser uma lista");
        };
        assert_eq!(signatures.len(), 1);
        assert_eq!(result.get("activeParameter").and_then(Json::number), Some(1));
    }

    #[test]
    fn lsp_framing_reads_utf8_content_length() {
        let body = r#"{"jsonrpc":"2.0","params":{"emoji":"😀"}}"#;
        let frame = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let mut reader = Cursor::new(frame.into_bytes());
        assert_eq!(read_message(&mut reader).expect("frame válido").as_deref(), Some(body));
    }

    #[test]
    fn function_parameters_are_indexed_as_definitions_and_completions() {
        let text = "funcao somar(a: inteiro, b: inteiro) { devolver a }";
        let file = ArquivoFonte::novo("file:///teste.nbx", text);
        let parsed = analisar(&file);
        let declarations = declaration_locations(&parsed.programa, text, "file:///teste.nbx");
        assert!(declarations.iter().any(|(name, _, _, _)| name == "a"));
        assert!(declarations.iter().any(|(name, _, _, _)| name == "b"));
        let document = Document { text: text.to_owned(), version: 1 };
        let result = completion(&document, "file:///teste.nbx", 0, text.len());
        let Json::Array(items) = result.get("items").expect("lista de conclusões") else {
            panic!("conclusões deveriam ser uma lista");
        };
        assert!(items.iter().any(|item| item.get("label").and_then(Json::string) == Some("a")));
    }

    #[test]
    fn json_string_escaping_is_valid() {
        let mut encoded = String::new();
        quote("linha\n\"Nebula\"", &mut encoded);
        assert_eq!(JsonParser::parse(&encoded), Ok(Json::String("linha\n\"Nebula\"".into())));
    }
}

}

fn main() {
    implementacao::iniciar();
}
