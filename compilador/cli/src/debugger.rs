use nebula_fonte::{ArquivoFonte, IntervaloFonte};
use nebula_ir::{
    baixar, executar_controlado, ComandoDepuracao, EventoControleDepuracao, EventoDepuracao,
    InstrucaoIr, ProgramaIr, ValorIr,
};
use nebula_semantica::ModulosDisponiveis;
use nebula_sintaxe::analisar;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread;

enum Entrada {
    Requisicao(Value),
    Evento(EventoControleDepuracao),
}

struct Estado {
    fonte: ArquivoFonte,
    comandos: Sender<ComandoDepuracao>,
    atual: Option<EventoDepuracao>,
    localizacoes: Vec<IntervaloFonte>,
    caminhos: Vec<String>,
}

pub fn iniciar(caminho: &Path, continuar_inicializacao: bool) -> Result<(), String> {
    let (fonte, programa) = carregar_programa(caminho)?;
    let localizacoes = localizacoes_programa(&programa);
    let (tx_entrada, rx_entrada) = mpsc::channel();
    let tx_leitor = tx_entrada.clone();
    thread::spawn(move || ler_requisicoes(tx_leitor));
    drop(tx_entrada);

    let mut saida = std::io::stdout().lock();
    let mut sequencia = 1_i64;
    let mut estado: Option<Estado> = None;

    while let Ok(entrada) = rx_entrada.recv() {
        match entrada {
            Entrada::Requisicao(requisicao) => {
                let seq = requisicao.get("seq").and_then(Value::as_i64).unwrap_or(0);
                let comando = requisicao.get("command").and_then(Value::as_str).unwrap_or("");
                let argumentos = requisicao.get("arguments").cloned().unwrap_or_else(|| json!({}));
                match comando {
                    "initialize" => {
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({
                            "supportsConfigurationDoneRequest": true,
                            "supportsTerminateRequest": true,
                            "supportsRestartRequest": false,
                            "supportsStepBack": false,
                            "supportsConditionalBreakpoints": false,
                            "supportsHitConditionalBreakpoints": false,
                            "supportsEvaluateForHovers": false,
                            "supportsSetVariable": false,
                            "supportsExceptionInfoRequest": false
                        }))?;
                        evento(&mut saida, &mut sequencia, "initialized", json!({}))?;
                    }
                    "launch" => {
                        let programa_lancado = argumentos.get("program")
                            .and_then(Value::as_str)
                            .map(PathBuf::from)
                            .unwrap_or_else(|| caminho.to_path_buf());
                        if programa_lancado != caminho.to_path_buf() {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "O arquivo solicitado difere do arquivo iniciado pelo CLI.")?;
                            continue;
                        }
                        let (tx_comandos, rx_comandos) = mpsc::channel();
                        let (tx_eventos, rx_eventos) = mpsc::channel();
                        let ir = programa.clone();
                        thread::spawn(move || {
                            let _ = executar_controlado(&ir, 1_000_000, rx_comandos, tx_eventos);
                        });
                        let tx = tx_entrada.clone();
                        thread::spawn(move || {
                            while let Ok(evento) = rx_eventos.recv() {
                                if tx.send(Entrada::Evento(evento)).is_err() { break; }
                            }
                        });
                        estado = Some(Estado {
                            fonte: fonte.clone(),
                            comandos: tx_comandos.clone(),
                            atual: None,
                            localizacoes: localizacoes.clone(),
                            caminhos: vec![caminho.to_string_lossy().into_owned()],
                        });
                        if continuar_inicializacao {
                            let _ = tx_comandos.send(ComandoDepuracao::Continuar);
                        }
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({}))?;
                    }
                    "setBreakpoints" => {
                        let Some(estado) = estado.as_mut() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        let linhas = argumentos.get("breakpoints").and_then(Value::as_array)
                            .cloned().unwrap_or_default();
                        let mut offsets = Vec::new();
                        let mut respostas = Vec::new();
                        for ponto in linhas {
                            let linha = ponto.get("line").and_then(Value::as_u64).unwrap_or(0) as usize;
                            let offset = offset_linha(&estado.fonte, linha);
                            let intervalo = offset.and_then(|offset| estado.localizacoes.iter()
                                .find(|intervalo| intervalo.inicio == offset || estado.fonte.linha_coluna(intervalo.inicio).is_some_and(|(l, _)| l == linha)));
                            if let Some(intervalo) = intervalo {
                                offsets.push(intervalo.inicio);
                                let (linha_real, coluna) = estado.fonte.linha_coluna(intervalo.inicio).unwrap_or((linha, 1));
                                respostas.push(json!({"id": respostas.len()+1, "verified": true, "line": linha_real, "column": coluna}));
                            } else {
                                respostas.push(json!({"id": respostas.len()+1, "verified": false, "line": linha, "message": "Nenhuma instrução executável nesta linha."}));
                            }
                        }
                        estado.comandos.send(ComandoDepuracao::DefinirPontosFonte(offsets))
                            .map_err(|erro| format!("Não foi possível atualizar breakpoints: {erro}"))?;
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({"breakpoints": respostas}))?;
                    }
                    "configurationDone" | "setExceptionBreakpoints" => {
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({}))?;
                    }
                    "threads" => {
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({
                            "threads": [{"id": 1, "name": "Nebula Runtime"}]
                        }))?;
                    }
                    "stackTrace" => {
                        let Some(estado) = estado.as_ref() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        let atual = estado.atual.as_ref();
                        let (linha, coluna) = atual.and_then(|evento| evento.intervalo_fonte)
                            .and_then(|intervalo| estado.fonte.linha_coluna(intervalo.inicio))
                            .unwrap_or((1, 1));
                        let mut nomes = atual.map(|evento| evento.pilha_chamadas.clone()).unwrap_or_default();
                        if nomes.is_empty() { nomes.push("Programa".into()); }
                        let frames = nomes.iter().rev().enumerate().map(|(indice, nome)| json!({
                            "id": indice as i64 + 1,
                            "name": nome,
                            "line": linha,
                            "column": coluna,
                            "source": {"name": estado.caminhos.first().cloned().unwrap_or_default(), "path": estado.caminhos.first().cloned().unwrap_or_default()}
                        })).collect::<Vec<_>>();
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({"stackFrames": frames, "totalFrames": frames.len()}))?;
                    }
                    "scopes" => {
                        let Some(estado) = estado.as_ref() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        let escopos = estado.atual.as_ref().map(|evento| evento.escopos.len()).unwrap_or(0);
                        let mut scopes = Vec::new();
                        for indice in (0..escopos).rev() {
                            scopes.push(json!({
                                "name": if indice == 0 { "Globais" } else { "Locais" },
                                "variablesReference": indice as i64 + 1,
                                "expensive": false
                            }));
                        }
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({"scopes": scopes}))?;
                    }
                    "variables" => {
                        let Some(estado) = estado.as_ref() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        let referencia = argumentos.get("variablesReference").and_then(Value::as_u64).unwrap_or(0) as usize;
                        let variaveis = estado.atual.as_ref()
                            .and_then(|evento| evento.escopos.get(referencia.saturating_sub(1)))
                            .map(|escopo| escopo.iter().map(|(nome, valor)| json!({
                                "name": nome,
                                "value": format!("{valor:?}"),
                                "type": tipo_valor(valor),
                                "variablesReference": 0
                            })).collect::<Vec<_>>())
                            .unwrap_or_default();
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({"variables": variaveis}))?;
                    }
                    "continue" | "next" | "stepOut" => {
                        let Some(estado) = estado.as_ref() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        let acao = match comando {
                            "next" => ComandoDepuracao::Avancar,
                            "stepOut" => ComandoDepuracao::Sair,
                            _ => ComandoDepuracao::Continuar,
                        };
                        estado.comandos.send(acao).map_err(|erro| format!("Falha ao enviar comando de depuração: {erro}"))?;
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({"allThreadsContinued": true}))?;
                        evento(&mut saida, &mut sequencia, "continued", json!({"threadId": 1, "allThreadsContinued": true}))?;
                    }
                    "pause" => {
                        responder_erro(&mut saida, &mut sequencia, seq, comando, "A pausa só ocorre em pontos de parada ou por avanço de instrução.")?;
                    }
                    "terminate" | "disconnect" => {
                        if let Some(estado) = estado.as_ref() {
                            let _ = estado.comandos.send(ComandoDepuracao::Encerrar);
                        }
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({}))?;
                        evento(&mut saida, &mut sequencia, "terminated", json!({}))?;
                        break;
                    }
                    _ => responder_erro(&mut saida, &mut sequencia, seq, comando, "Comando DAP não suportado pela Nebula.")?,
                }
            }
            Entrada::Evento(evento_recebido) => {
                match evento_recebido {
                    EventoControleDepuracao::Pausado(atual) => {
                        let Some(estado) = estado.as_mut() else { continue; };
                        let primeira = estado.atual.is_none();
                        estado.atual = Some(atual.clone());
                        if primeira && continuar_inicializacao { continue; }
                        let (linha, coluna) = atual.intervalo_fonte
                            .and_then(|intervalo| estado.fonte.linha_coluna(intervalo.inicio))
                            .unwrap_or((1, 1));
                        evento(&mut saida, &mut sequencia, "stopped", json!({
                            "reason": if primeira { "entry" } else { "step" },
                            "description": format!("Nebula pausada na linha {linha}, coluna {coluna}."),
                            "threadId": 1,
                            "allThreadsStopped": true
                        }))?;
                    }
                    EventoControleDepuracao::Finalizado(resultado) => {
                        if let Some(estado) = estado.as_mut() {
                            match resultado {
                                Ok(resultado) => {
                                    for linha in resultado.saida {
                                        evento(&mut saida, &mut sequencia, "output", json!({"category":"stdout","output":format!("{linha}\n")}))?;
                                    }
                                }
                                Err(erro) => {
                                    evento(&mut saida, &mut sequencia, "output", json!({"category":"stderr","output":format!("{erro}\n")}))?;
                                }
                            }
                        }
                        evento(&mut saida, &mut sequencia, "terminated", json!({}))?;
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

fn carregar_programa(caminho: &Path) -> Result<(ArquivoFonte, ProgramaIr), String> {
    let bytes = std::fs::read(caminho)
        .map_err(|erro| format!("não foi possível ler '{}': {erro}", caminho.display()))?;
    let conteudo = String::from_utf8(bytes).map_err(|erro| {
        format!("arquivo não contém UTF-8 válido no byte {}", erro.utf8_error().valid_up_to())
    })?;
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let sintaxe = analisar(&fonte);
    if !sintaxe.diagnosticos.is_empty() {
        return Err(sintaxe.diagnosticos.iter()
            .map(|diagnostico| format!("{} — {}", diagnostico.codigo, diagnostico.mensagem))
            .collect::<Vec<_>>().join("\n"));
    }
    let (modulos, programas_modulos, _) = super::carregar_modulos(caminho, &sintaxe.programa)?;
    let resultado = baixar(&sintaxe.programa, &modulos);
    if !resultado.diagnosticos.is_empty() {
        return Err(resultado.diagnosticos.join("\n"));
    }
    let mut programa = resultado.programa.expect("IR validada");
    super::ligar_modulos(&mut programa, programas_modulos);
    Ok((fonte, programa))
}

fn localizacoes_programa(programa: &ProgramaIr) -> Vec<IntervaloFonte> {
    fn coletar(instrucao: &InstrucaoIr, saida: &mut Vec<IntervaloFonte>) {
        match instrucao {
            InstrucaoIr::Localizada { intervalo, instrucao } => {
                saida.push(*intervalo);
                coletar(instrucao, saida);
            }
            InstrucaoIr::Bloco(itens) => for item in itens { coletar(item, saida); },
            InstrucaoIr::Se { entao, senao, .. } => {
                for item in entao.iter().chain(senao) { coletar(item, saida); }
            }
            InstrucaoIr::Escolha { casos, .. } => for caso in casos {
                for item in &caso.corpo { coletar(item, saida); }
            },
            InstrucaoIr::Enquanto { corpo, .. } | InstrucaoIr::Cada { corpo, .. }
            | InstrucaoIr::Pulso { corpo, .. } => for item in corpo { coletar(item, saida); },
            InstrucaoIr::Para { inicial, corpo, .. } => {
                if let Some(inicial) = inicial { coletar(inicial, saida); }
                for item in corpo { coletar(item, saida); }
            }
            _ => {}
        }
    }
    let mut saida = Vec::new();
    for instrucao in &programa.instrucoes { coletar(instrucao, &mut saida); }
    for funcao in programa.funcoes.values() {
        for instrucao in &funcao.corpo { coletar(instrucao, &mut saida); }
    }
    saida
}

fn offset_linha(fonte: &ArquivoFonte, linha: usize) -> Option<usize> {
    if linha == 0 { return None; }
    let mut offset = 0;
    for (indice, conteudo) in fonte.conteudo().split_inclusive('\n').enumerate() {
        if indice + 1 == linha {
            let recuo = conteudo.len() - conteudo.trim_start().len();
            return Some(offset + recuo);
        }
        offset += conteudo.len();
    }
    if linha == fonte.conteudo().lines().count() + 1 && fonte.conteudo().is_empty() {
        return Some(0);
    }
    None
}

fn tipo_valor(valor: &ValorIr) -> &'static str {
    match valor {
        ValorIr::Inteiro(_) | ValorIr::InteiroSemSinal(_) => "inteiro",
        ValorIr::Decimal(_) => "decimal",
        ValorIr::Logico(_) => "lógico",
        ValorIr::Texto(_) => "texto",
        ValorIr::Caractere(_) => "caractere",
        ValorIr::Funcao(_) => "função",
        ValorIr::Lista(_) => "lista",
        ValorIr::Estrutura { .. } => "estrutura",
        ValorIr::Intervalo(_, _) | ValorIr::IntervaloSemSinal(_, _) | ValorIr::IntervaloDecimal(_, _) => "intervalo",
        ValorIr::Nulo => "nulo",
        ValorIr::Vazio => "vazio",
    }
}

fn ler_requisicoes(saida: Sender<Entrada>) {
    let stdin = std::io::stdin();
    let mut leitor = BufReader::new(stdin.lock());
    loop {
        let mut tamanho = None;
        let mut linha = String::new();
        loop {
            linha.clear();
            match leitor.read_line(&mut linha) {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            if linha == "\r\n" || linha == "\n" { break; }
            if let Some(valor) = linha.strip_prefix("Content-Length:") {
                tamanho = valor.trim().parse::<usize>().ok();
            }
        }
        let Some(tamanho) = tamanho else { continue; };
        let mut corpo = vec![0; tamanho];
        if leitor.read_exact(&mut corpo).is_err() { return; }
        let Ok(requisicao) = serde_json::from_slice::<Value>(&corpo) else { continue; };
        if saida.send(Entrada::Requisicao(requisicao)).is_err() { return; }
    }
}

fn responder(
    saida: &mut impl Write,
    sequencia: &mut i64,
    request_seq: i64,
    comando: &str,
    sucesso: bool,
    corpo: Value,
) -> Result<(), String> {
    let mensagem = json!({
        "seq": *sequencia,
        "type": "response",
        "request_seq": request_seq,
        "success": sucesso,
        "command": comando,
        "body": corpo
    });
    *sequencia += 1;
    escrever_mensagem(saida, &mensagem)
}

fn responder_erro(
    saida: &mut impl Write,
    sequencia: &mut i64,
    request_seq: i64,
    comando: &str,
    mensagem: &str,
) -> Result<(), String> {
    let corpo = json!({"error": {"id": 1, "format": mensagem, "showUser": true}});
    responder(saida, sequencia, request_seq, comando, false, corpo)
}

fn evento(
    saida: &mut impl Write,
    sequencia: &mut i64,
    nome: &str,
    corpo: Value,
) -> Result<(), String> {
    let mensagem = json!({
        "seq": *sequencia,
        "type": "event",
        "event": nome,
        "body": corpo
    });
    *sequencia += 1;
    escrever_mensagem(saida, &mensagem)
}

fn escrever_mensagem(saida: &mut impl Write, mensagem: &Value) -> Result<(), String> {
    let corpo = serde_json::to_vec(mensagem).map_err(|erro| erro.to_string())?;
    write!(saida, "Content-Length: {}\r\n\r\n", corpo.len())
        .and_then(|_| saida.write_all(&corpo))
        .and_then(|_| saida.flush())
        .map_err(|erro| format!("falha ao escrever protocolo DAP: {erro}"))
}
