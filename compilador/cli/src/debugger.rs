use nebula_fonte::{ArquivoFonte, IntervaloFonte};
use nebula_ir::{
    baixar, executar_controlado, ComandoDepuracao, EventoControleDepuracao, EventoDepuracao,
    InstrucaoIr, ProgramaIr, ValorIr,
};
use nebula_sintaxe::analisar;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread;

enum Entrada {
    Requisicao(Value),
    Evento(EventoControleDepuracao),
    Fechou,
}

struct Estado {
    fontes: HashMap<PathBuf, ArquivoFonte>,
    caminho_principal: PathBuf,
    comandos: Sender<ComandoDepuracao>,
    atual: Option<EventoDepuracao>,
    localizacoes: Vec<(PathBuf, IntervaloFonte)>,
    expansoes: HashMap<usize, Vec<(String, ValorIr)>>,
    proxima_referencia: usize,
    pontos_parada: HashSet<(PathBuf, usize)>,
    encerrando: bool,
}

pub fn iniciar(caminho: &Path, continuar_inicializacao: bool) -> Result<(), String> {
    let (fonte, programa) = carregar_programa(caminho)?;
    let caminho_principal = normalizar_caminho(caminho, caminho);
    let localizacoes = localizacoes_programa(&programa).into_iter()
        .map(|(origem, intervalo)| (normalizar_caminho(&origem, &caminho_principal), intervalo))
        .collect::<Vec<_>>();
    let mut fontes = HashMap::new();
    fontes.insert(caminho_principal.clone(), ArquivoFonte::novo(caminho_principal.clone(), fonte.conteudo().to_owned()));
    for (origem, _) in &localizacoes {
        if !fontes.contains_key(origem) {
            if let Ok(conteudo) = std::fs::read_to_string(origem) {
                fontes.insert(origem.clone(), ArquivoFonte::novo(origem.clone(), conteudo));
            }
        }
    }
    let (tx_entrada, rx_entrada) = mpsc::channel();
    let tx_leitor = tx_entrada.clone();
    thread::spawn(move || ler_requisicoes(tx_leitor));

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
                            "supportsEvaluateForHovers": true,
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
                        if normalizar_caminho(&programa_lancado, &caminho_principal) != caminho_principal {
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
                            fontes: fontes.clone(),
                            caminho_principal: caminho_principal.clone(),
                            comandos: tx_comandos.clone(),
                            atual: None,
                            localizacoes: localizacoes.clone(),
                            expansoes: HashMap::new(),
                            proxima_referencia: 1,
                            pontos_parada: HashSet::new(),
                            encerrando: false,
                        });
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({}))?;
                        evento(&mut saida, &mut sequencia, "thread", json!({"reason":"started","threadId":1}))?;
                    }
                    "setBreakpoints" => {
                        let Some(estado) = estado.as_mut() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        let caminho_solicitado = argumentos.get("source")
                            .and_then(|source| source.get("path"))
                            .and_then(Value::as_str)
                            .map(PathBuf::from)
                            .unwrap_or_else(|| estado.caminho_principal.clone());
                        let caminho_solicitado = normalizar_caminho(&caminho_solicitado, &estado.caminho_principal);
                        let fonte = estado.fontes.get(&caminho_solicitado);
                        let linhas = argumentos.get("breakpoints").and_then(Value::as_array)
                            .cloned().unwrap_or_default();
                        let mut pontos = Vec::new();
                        let mut respostas = Vec::new();
                        for ponto in linhas {
                            let linha = ponto.get("line").and_then(Value::as_u64).unwrap_or(0) as usize;
                            let localizacao = fonte.and_then(|fonte| {
                                offset_linha(fonte, linha)?;
                                estado.localizacoes.iter()
                                    .find(|(origem, intervalo)| origem == &caminho_solicitado
                                        && linha_coluna_dap(fonte, intervalo.inicio).is_some_and(|(linha_real, _)| linha_real == linha))
                                    .cloned()
                            });
                            if let (Some((origem, intervalo)), Some(fonte)) = (localizacao, fonte) {
                                pontos.push((origem, intervalo.inicio));
                                let (linha_real, coluna) = linha_coluna_dap(fonte, intervalo.inicio).unwrap_or((linha, 1));
                                respostas.push(json!({"id": respostas.len()+1, "verified": true, "line": linha_real, "column": coluna}));
                            } else {
                                let mensagem = if fonte.is_none() {
                                    "Arquivo-fonte não pertence ao programa depurado."
                                } else {
                                    "Nenhuma instrução executável nesta linha."
                                };
                                respostas.push(json!({"id": respostas.len()+1, "verified": false, "line": linha, "message": mensagem}));
                            }
                        }
                        estado.pontos_parada = pontos.iter().cloned().collect();
                        estado.comandos.send(ComandoDepuracao::DefinirPontosFonte(pontos))
                            .map_err(|erro| format!("Não foi possível atualizar breakpoints: {erro}"))?;
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({"breakpoints": respostas}))?;
                    }
                    "configurationDone" => {
                        if continuar_inicializacao {
                            if let Some(estado) = estado.as_ref() {
                                estado.comandos.send(ComandoDepuracao::Continuar)
                                    .map_err(|erro| format!("Falha ao iniciar execução: {erro}"))?;
                            }
                        }
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({}))?;
                    }
                    "setExceptionBreakpoints" => {
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
                        let nomes = atual.map(|evento| evento.pilha_chamadas.as_slice()).unwrap_or(&[]);
                        let mut frames = Vec::new();
                        for indice_topo in 0..nomes.len() {
                            let indice_funcao = nomes.len() - 1 - indice_topo;
                            let localizacao = atual.and_then(|evento| {
                                if indice_topo == 0 {
                                    localizacao_evento(evento, &estado.caminho_principal)
                                } else {
                                    localizacao_chamada(evento, indice_funcao + 1, &estado.caminho_principal)
                                }
                            });
                            let (caminho, intervalo) = localizacao.unwrap_or((estado.caminho_principal.clone(), IntervaloFonte { inicio: 0, fim: 0 }));
                            let (linha, coluna) = estado.fontes.get(&caminho)
                                .and_then(|fonte| linha_coluna_dap(fonte, intervalo.inicio))
                                .unwrap_or((1, 1));
                            frames.push(json!({
                                "id": indice_topo as i64 + 1,
                                "name": nomes[indice_funcao],
                                "line": linha,
                                "column": coluna,
                                "source": fonte_json(&caminho)
                            }));
                        }
                        let localizacao_programa = atual.and_then(|evento| {
                            if nomes.is_empty() {
                                localizacao_evento(evento, &estado.caminho_principal)
                            } else {
                                localizacao_chamada(evento, 0, &estado.caminho_principal)
                            }
                        });
                        let (caminho_programa, intervalo_programa) = localizacao_programa
                            .unwrap_or((estado.caminho_principal.clone(), IntervaloFonte { inicio: 0, fim: 0 }));
                        let (linha_programa, coluna_programa) = estado.fontes.get(&caminho_programa)
                            .and_then(|fonte| linha_coluna_dap(fonte, intervalo_programa.inicio))
                            .unwrap_or((1, 1));
                        frames.push(json!({
                            "id": nomes.len() as i64 + 1,
                            "name": "Programa",
                            "line": linha_programa,
                            "column": coluna_programa,
                            "source": fonte_json(&caminho_programa)
                        }));
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({"stackFrames": frames, "totalFrames": frames.len()}))?;
                    }
                    "scopes" => {
                        let Some(estado) = estado.as_ref() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        let Some(atual) = estado.atual.as_ref() else {
                            responder(&mut saida, &mut sequencia, seq, comando, true, json!({"scopes": []}))?;
                            continue;
                        };
                        let frame_id = argumentos.get("frameId").and_then(Value::as_u64).unwrap_or(1) as usize;
                        let quantidade_funcoes = atual.pilha_chamadas.len();
                        let (inicio, fim) = if quantidade_funcoes == 0 {
                            (0, atual.escopos.len())
                        } else if frame_id <= quantidade_funcoes {
                            let indice_funcao = quantidade_funcoes - frame_id;
                            let inicio = atual.limites_escopos.get(indice_funcao).copied().unwrap_or(0);
                            let fim = atual.limites_escopos.get(indice_funcao + 1).copied().unwrap_or(atual.escopos.len());
                            (inicio.min(atual.escopos.len()), fim.min(atual.escopos.len()))
                        } else if frame_id == quantidade_funcoes + 1 {
                            (0, atual.limites_escopos.first().copied().unwrap_or(atual.escopos.len()).min(atual.escopos.len()))
                        } else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "Quadro de pilha inexistente.")?;
                            continue;
                        };
                        let mut scopes = Vec::new();
                        for indice in (inicio..fim).rev() {
                            scopes.push(json!({
                                "name": if indice == 0 { "Globais".to_owned() } else { format!("Escopo {indice}") },
                                "variablesReference": indice as i64 + 1000,
                                "expensive": false
                            }));
                        }
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({"scopes": scopes}))?;
                    }
                    "variables" => {
                        let Some(estado) = estado.as_mut() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        let referencia = argumentos.get("variablesReference").and_then(Value::as_u64).unwrap_or(0) as usize;
                        let itens = if referencia >= 1000 {
                            estado.atual.as_ref()
                                .and_then(|evento| evento.escopos.get(referencia - 1000))
                                .map(|escopo| {
                                    let mut itens = escopo.iter().map(|(nome, valor)| (nome.clone(), valor.clone())).collect::<Vec<_>>();
                                    itens.sort_by(|a, b| a.0.cmp(&b.0));
                                    itens
                                })
                        } else {
                            estado.expansoes.get(&referencia).cloned()
                        }.unwrap_or_default();
                        let variaveis = criar_variaveis(estado, itens);
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({"variables": variaveis}))?;
                    }
                    "continue" | "next" | "stepIn" | "stepOut" => {
                        let Some(estado) = estado.as_ref() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        let acao = match comando {
                            "next" => {
                                let linha_atual = estado.atual.as_ref()
                                    .and_then(|evento| evento.intervalo_fonte)
                                    .and_then(|intervalo| linha_coluna_dap(&estado.fonte, intervalo.inicio))
                                    .map(|(linha, _)| linha);
                                let offsets = estado.localizacoes.iter().filter_map(|intervalo| {
                                    let (linha, _) = linha_coluna_dap(&estado.fonte, intervalo.inicio)?;
                                    (Some(linha) == linha_atual).then_some(intervalo.inicio)
                                }).collect();
                                ComandoDepuracao::AvancarLinha(offsets)
                            }
                            "stepIn" => ComandoDepuracao::Avancar,
                            "stepOut" => ComandoDepuracao::Sair,
                            _ => ComandoDepuracao::Continuar,
                        };
                        estado.comandos.send(acao).map_err(|erro| format!("Falha ao enviar comando de depuração: {erro}"))?;
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({"allThreadsContinued": true}))?;
                        evento(&mut saida, &mut sequencia, "continued", json!({"threadId": 1, "allThreadsContinued": true}))?;
                    }
                    "evaluate" => {
                        let Some(estado) = estado.as_mut() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        let expressao = argumentos.get("expression").and_then(Value::as_str).unwrap_or("").trim();
                        let frame_id = argumentos.get("frameId").and_then(Value::as_u64).unwrap_or(1) as usize;
                        let valor = estado.atual.as_ref().and_then(|evento| {
                            let (inicio, fim) = faixa_escopos(evento, frame_id)?;
                            evento.escopos.get(inicio..fim)?
                                .iter()
                                .rev()
                                .find_map(|escopo| escopo.get(expressao))
                                .cloned()
                        });
                        if let Some(valor) = valor {
                            let variavel = criar_variavel(estado, expressao.to_owned(), valor);
                            responder(&mut saida, &mut sequencia, seq, comando, true, json!({
                                "result": variavel["value"],
                                "type": variavel["type"],
                                "variablesReference": variavel["variablesReference"]
                            }))?;
                        } else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A expressão não corresponde a uma variável visível.")?;
                        }
                    }
                    "pause" => {
                        let Some(estado) = estado.as_ref() else {
                            responder_erro(&mut saida, &mut sequencia, seq, comando, "A sessão ainda não foi iniciada.")?;
                            continue;
                        };
                        estado.comandos.send(ComandoDepuracao::Pausar)
                            .map_err(|erro| format!("Falha ao solicitar pausa: {erro}"))?;
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({}))?;
                    }
                    "terminate" | "disconnect" => {
                        let sessao_ativa = if let Some(estado) = estado.as_mut() {
                            estado.encerrando = true;
                            let _ = estado.comandos.send(ComandoDepuracao::Encerrar);
                            true
                        } else {
                            false
                        };
                        responder(&mut saida, &mut sequencia, seq, comando, true, json!({}))?;
                        if !sessao_ativa {
                            evento(&mut saida, &mut sequencia, "terminated", json!({}))?;
                            break;
                        }
                    }
                    _ => responder_erro(&mut saida, &mut sequencia, seq, comando, "Comando DAP não suportado pela Nebula.")?,
                }
            }
            Entrada::Fechou => {
                if let Some(estado) = estado.as_mut() {
                    estado.encerrando = true;
                    let _ = estado.comandos.send(ComandoDepuracao::Encerrar);
                } else {
                    break;
                }
            }
            Entrada::Evento(evento_recebido) => {
                match evento_recebido {
                    EventoControleDepuracao::Pausado(atual) => {
                        let Some(estado) = estado.as_mut() else { continue; };
                        let primeira = estado.atual.is_none();
                        estado.atual = Some(atual.clone());
                        estado.expansoes.clear();
                        estado.proxima_referencia = 1;
                        if primeira && continuar_inicializacao { continue; }
                        let (linha, coluna) = atual.intervalo_fonte
                            .and_then(|intervalo| estado.fonte.linha_coluna(intervalo.inicio))
                            .unwrap_or((1, 1));
                        let motivo = if primeira {
                            "entry"
                        } else if atual.intervalo_fonte.is_some_and(|intervalo| estado.pontos_parada.contains(&intervalo.inicio)) {
                            "breakpoint"
                        } else {
                            "step"
                        };
                        evento(&mut saida, &mut sequencia, "stopped", json!({
                            "reason": motivo,
                            "description": format!("Nebula pausada na linha {linha}, coluna {coluna}."),
                            "threadId": 1,
                            "allThreadsStopped": true
                        }))?;
                    }
                    EventoControleDepuracao::Finalizado(resultado) => {
                        let encerrando = estado.as_ref().is_some_and(|estado| estado.encerrando);
                        match resultado {
                            Ok(resultado) => {
                                for linha in resultado.saida {
                                    evento(&mut saida, &mut sequencia, "output", json!({"category":"stdout","output":format!("{linha}\n")}))?;
                                }
                                evento(&mut saida, &mut sequencia, "exited", json!({"exitCode":0}))?;
                            }
                            Err(_) if encerrando => {
                                evento(&mut saida, &mut sequencia, "exited", json!({"exitCode":0}))?;
                            }
                            Err(erro) => {
                                evento(&mut saida, &mut sequencia, "output", json!({"category":"stderr","output":format!("{erro}\n")}))?;
                                evento(&mut saida, &mut sequencia, "exited", json!({"exitCode":1}))?;
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

fn localizacoes_programa(programa: &ProgramaIr) -> Vec<(PathBuf, IntervaloFonte)> {
    fn coletar(instrucao: &InstrucaoIr, saida: &mut Vec<(PathBuf, IntervaloFonte)>) {
        match instrucao {
            InstrucaoIr::Localizada { caminho, intervalo, instrucao } => {
                saida.push((caminho.clone(), *intervalo));
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

fn normalizar_caminho(caminho: &Path, principal: &Path) -> PathBuf {
    let caminho = if caminho.as_os_str().is_empty() { principal } else { caminho };
    std::fs::canonicalize(caminho).unwrap_or_else(|_| caminho.to_path_buf())
}

fn linha_coluna_dap(fonte: &ArquivoFonte, offset: usize) -> Option<(usize, usize)> {
    let conteudo = fonte.conteudo();
    if offset > conteudo.len() || !conteudo.is_char_boundary(offset) {
        return None;
    }
    let prefixo = &conteudo[..offset];
    let linha = prefixo.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let inicio_linha = prefixo.rfind('\n').map_or(0, |pos| pos + 1);
    let coluna = conteudo[inicio_linha..offset].encode_utf16().count() + 1;
    Some((linha, coluna))
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

fn faixa_escopos(evento: &EventoDepuracao, frame_id: usize) -> Option<(usize, usize)> {
    let quantidade_funcoes = evento.pilha_chamadas.len();
    let total = evento.escopos.len();
    if quantidade_funcoes == 0 {
        return (frame_id == 1).then_some((0, total));
    }
    if frame_id == 0 {
        return None;
    }
    if frame_id <= quantidade_funcoes {
        let indice_funcao = quantidade_funcoes - frame_id;
        let inicio = *evento.limites_escopos.get(indice_funcao)?;
        let fim = evento.limites_escopos.get(indice_funcao + 1).copied().unwrap_or(total);
        return Some((inicio.min(total), fim.min(total)));
    }
    if frame_id == quantidade_funcoes + 1 {
        return Some((0, evento.limites_escopos.first().copied().unwrap_or(total).min(total)));
    }
    None
}

fn criar_variaveis(estado: &mut Estado, itens: Vec<(String, ValorIr)>) -> Vec<Value> {
    itens.into_iter().map(|(nome, valor)| criar_variavel(estado, nome, valor)).collect()
}

fn criar_variavel(estado: &mut Estado, nome: String, valor: ValorIr) -> Value {
    let filhos = match &valor {
        ValorIr::Lista(itens) => Some(itens.iter().enumerate()
            .map(|(indice, valor)| (indice.to_string(), valor.clone())).collect::<Vec<_>>()),
        ValorIr::Estrutura { campos, .. } => Some(campos.clone()),
        _ => None,
    };
    let referencia = filhos.filter(|itens| !itens.is_empty()).map_or(0, |itens| {
        let referencia = estado.proxima_referencia;
        estado.proxima_referencia += 1;
        estado.expansoes.insert(referencia, itens);
        referencia
    });
    json!({
        "name": nome,
        "value": format!("{valor:?}"),
        "type": tipo_valor(&valor),
        "variablesReference": referencia
    })
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
                Ok(0) | Err(_) => {
                    let _ = saida.send(Entrada::Fechou);
                    return;
                },
                Ok(_) => {}
            }
            if linha == "\r\n" || linha == "\n" { break; }
            if let Some(valor) = linha.strip_prefix("Content-Length:") {
                tamanho = valor.trim().parse::<usize>().ok();
            }
        }
        let Some(tamanho) = tamanho else { continue; };
        let mut corpo = vec![0; tamanho];
        if leitor.read_exact(&mut corpo).is_err() {
            let _ = saida.send(Entrada::Fechou);
            return;
        }
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

#[cfg(test)]
mod testes {
    use super::{linha_coluna_dap, offset_linha};
    use nebula_fonte::ArquivoFonte;

    #[test]
    fn coluna_dap_conta_unidades_utf16() {
        let fonte = ArquivoFonte::novo("unicode.nbx", "a😀b");
        let offset = "a😀".len();
        assert_eq!(linha_coluna_dap(&fonte, offset), Some((1, 4)));
    }

    #[test]
    fn breakpoint_ignora_recuo_da_linha() {
        let fonte = ArquivoFonte::novo("recuo.nbx", "primeira\n    segunda");
        assert_eq!(offset_linha(&fonte, 2), Some("primeira\n    ".len()));
    }
}
