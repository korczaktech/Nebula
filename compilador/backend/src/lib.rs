//! Backend nativo da Nebula: geração direta de executáveis ELF x86-64.
#![forbid(unsafe_code)]

/// Classificação e planejamento da convenção ABI interna.
pub mod abi;

#[rustfmt::skip]
mod implementacao {
use nebula_biblioteca::dados::{analisar_json, serializar_json};
use nebula_ir::{CasoEscolhaIr, ExpressaoIr, FuncaoIr, InstrucaoIr, OperadorIr, ProgramaIr, ValorIr};
use nebula_semantica::Tipo;
use std::collections::HashMap;

const TAMANHO_CABECALHO: usize = 4096;
const TAMANHO_QUADRO: u32 = 65536;
const INICIO_AREA_AGREGADOS: u64 = (LIMITE_VARIAVEIS as u64) * 16 + 8;
const LIMITE_VARIAVEIS: usize = 512;
const NOME_INICIO_INTERNO: &str = "\0nebula_inicio";

fn instrucao_base(mut instrucao: &InstrucaoIr) -> &InstrucaoIr {
    while let InstrucaoIr::Localizada { instrucao: interna, .. } = instrucao {
        instrucao = interna;
    }
    instrucao
}

/// Compila a IR em um executável ELF64 estático para Linux x86-64.
///
/// O arquivo é construído diretamente em Rust. Não gera C/C++, assembly textual,
/// objeto intermediário nem invoca linker ou compilador externo.
pub fn gerar_executavel(programa: &ProgramaIr) -> Result<Vec<u8>, String> {
    let mut globais = HashMap::new();
    for instrucao in &programa.instrucoes {
        if let InstrucaoIr::Declarar { nome, .. } = instrucao_base(instrucao) {
            let deslocamento = globais.len().checked_mul(16)
                .ok_or("Quantidade de variáveis globais excedeu o limite do alvo.")?;
            if globais.insert(nome.clone(), deslocamento).is_some() {
                return Err(format!("Variável global '{nome}' declarada mais de uma vez."));
            }
        }
    }
    let mut emissor = Emissor::novo(globais);
    emissor.carregar_layouts(programa);
    for instrucao in &programa.instrucoes {
        if let InstrucaoIr::Declarar { tipo, .. } = instrucao_base(instrucao) {
            emissor.validar_tipo_abi(tipo)?;
        }
    }
    emissor.iniciar_programa(programa)?;
    let (codigo, dados) = emissor.finalizar()?;
    gerar_elf(&codigo, &dados)
}

struct LayoutAgregado {
    tamanho: u64,
    alinhamento: u64,
    campos: HashMap<String, (u64, u64)>,
}

struct Emissor {
    codigo: Vec<u8>,
    funcoes: HashMap<String, usize>,
    chamadas: Vec<(usize, String)>,
    referencias_funcoes: Vec<(usize, String)>,
    escopos: Vec<HashMap<String, i32>>,
    tipos_escopos: Vec<HashMap<String, Tipo>>,
    proximo_slot: usize,
    proximo_agregado: u64,
    pilha: usize,
    lacos: Vec<Laco>,
    globais: HashMap<String, usize>,
    tipos_globais: HashMap<String, Tipo>,
    retornos_funcoes: HashMap<String, Tipo>,
    retorno_oculto_atual: Option<(i32, u64)>,
    layouts_tipos: HashMap<String, LayoutAgregado>,
    referencias_globais: Vec<(usize, String, i32)>,
    dados_estaticos: Vec<u8>,
    referencias_texto: Vec<(usize, usize)>,
    funcao_atual: String,
    erro_salto: Option<String>,
}

#[derive(Clone, Copy)]
enum PosicaoArgumento {
    Registradores(usize),
    Pilha { deslocamento: u64, largura: usize },
}

fn planejar_posicoes_argumentos(
    larguras: &[usize],
    inicio_registrador: usize,
) -> Result<Vec<PosicaoArgumento>, String> {
    if inicio_registrador > 6 || larguras.iter().any(|largura| !matches!(largura, 1 | 2)) {
        return Err("Largura ou início de registradores inválido no ABI x86-64.".into());
    }
    let mut proximo_registrador = inicio_registrador;
    let mut topo_pilha = 0u64;
    let mut posicoes = Vec::with_capacity(larguras.len());
    for largura in larguras {
        if proximo_registrador + largura <= 6 {
            posicoes.push(PosicaoArgumento::Registradores(proximo_registrador));
            proximo_registrador += largura;
        } else {
            let alinhamento = if *largura == 2 { 16 } else { 8 };
            topo_pilha = topo_pilha.checked_add(alinhamento - 1)
                .map(|valor| valor & !(alinhamento - 1))
                .ok_or("Overflow ao alinhar argumentos na pilha.")?;
            posicoes.push(PosicaoArgumento::Pilha {
                deslocamento: topo_pilha,
                largura: *largura,
            });
            topo_pilha = topo_pilha.checked_add((*largura as u64) * 8)
                .ok_or("Área de argumentos excede o limite x86-64.")?;
        }
    }
    Ok(posicoes)
}

fn alinhar_pilha_16(valor: u64) -> Result<u64, String> {
    valor.checked_add(15)
        .map(|valor| valor & !15)
        .ok_or_else(|| "Overflow ao alinhar argumentos na pilha.".into())
}

struct Laco {
    saidas: Vec<usize>,
    continuacoes: Vec<usize>,
}

impl Emissor {
    fn novo(globais: HashMap<String, usize>) -> Self {
        Self {
            codigo: Vec::new(),
            funcoes: HashMap::new(),
            chamadas: Vec::new(),
            referencias_funcoes: Vec::new(),
            escopos: Vec::new(),
            tipos_escopos: Vec::new(),
            proximo_slot: 0,
            proximo_agregado: INICIO_AREA_AGREGADOS,
            pilha: 0,
            lacos: Vec::new(),
            globais,
            tipos_globais: HashMap::new(),
            retornos_funcoes: HashMap::new(),
            retorno_oculto_atual: None,
            layouts_tipos: HashMap::new(),
            referencias_globais: Vec::new(),
            dados_estaticos: Vec::new(),
            referencias_texto: Vec::new(),
            funcao_atual: String::new(),
            erro_salto: None,
        }
    }

    fn carregar_layouts(&mut self, programa: &ProgramaIr) {
        self.retornos_funcoes = programa.funcoes.iter()
            .map(|(nome, funcao)| (nome.clone(), funcao.retorno.clone()))
            .collect();
        self.tipos_globais = programa.instrucoes.iter().filter_map(|instrucao| {
            if let InstrucaoIr::Declarar { nome, tipo, .. } = instrucao_base(instrucao) {
                Some((nome.clone(), tipo.clone()))
            } else {
                None
            }
        }).collect();
        self.layouts_tipos = programa.layouts_tipos.iter().map(|(nome, layout)| {
            let campos = layout.campos.iter().map(|campo| {
                (campo.nome.clone(), (campo.deslocamento, campo.tamanho))
            }).collect();
            (nome.clone(), LayoutAgregado { tamanho: layout.tamanho, alinhamento: layout.alinhamento, campos })
        }).collect();
    }

    fn layout_agregado_grande(&self, tipo: &Tipo) -> Option<(u64, u64)> {
        match tipo {
            Tipo::Estrutura(nome) => {
                let layout = self.layouts_tipos.get(nome)?;
                (layout.tamanho > 16).then_some((layout.tamanho, layout.alinhamento))
            }
            Tipo::Tupla(tipos) => {
                let (tamanho, _) = self.layout_tupla(tipos)?;
                let alinhamento = tipos.iter()
                    .map(|tipo| self.tamanho_alinhamento_tipo(tipo).map(|(_, alinhamento)| alinhamento))
                    .collect::<Option<Vec<_>>>()?
                    .into_iter()
                    .max()
                    .unwrap_or(1);
                (tamanho > 16).then_some((tamanho, alinhamento))
            }
            _ => None,
        }
    }

    fn validar_tipo_abi(&self, tipo: &Tipo) -> Result<(), String> {
        match tipo {
            Tipo::Funcao(parametros, retorno) => {
                for parametro in parametros {
                    if *parametro == Tipo::Vazio {
                        return Err("Parâmetro de função não pode ter tipo vazio no ABI x86-64.".into());
                    }
                    self.validar_tipo_abi(parametro)?;
                }
                self.validar_tipo_abi(retorno)
            }
            Tipo::Estrutura(nome) => {
                let layout = self.layouts_tipos.get(nome)
                    .ok_or_else(|| format!("Layout da estrutura '{nome}' não foi fornecido à ABI nativa."))?;
                if layout.tamanho == 0 {
                    return Err(format!("A estrutura '{nome}' possui tamanho inválido."));
                }
                for (campo, (deslocamento, tamanho)) in &layout.campos {
                    let fim = deslocamento.checked_add(*tamanho)
                        .ok_or_else(|| format!("Campo '{campo}' excede o layout de '{nome}'."))?;
                    if *tamanho == 0 || fim > layout.tamanho {
                        return Err(format!("Campo '{campo}' excede os limites da estrutura '{nome}'."));
                    }
                    if layout.tamanho <= 16 {
                        let campo_em_dois_registros = *deslocamento == 0 && *tamanho == 16;
                        let campo_escalar = matches!(*deslocamento, 0 | 8) && (1..=8).contains(tamanho);
                        if !campo_em_dois_registros && !campo_escalar {
                            return Err(format!("Campo '{campo}' da estrutura '{nome}' não cabe nos dois registradores ABI."));
                        }
                    }
                }
                Ok(())
            }
            Tipo::Tupla(tipos) => {
                let (tamanho, campos) = self.layout_tupla(tipos)
                    .ok_or_else(|| "Layout de tupla não suportado pela ABI nativa.".to_owned())?;
                if tamanho == 0 {
                    return Err("Tupla possui tamanho inválido.".into());
                }
                for (deslocamento, tamanho_campo) in campos {
                    let fim = deslocamento.checked_add(tamanho_campo)
                        .ok_or("Elemento de tupla excede o layout.")?;
                    if tamanho_campo == 0 || fim > tamanho {
                        return Err("Elemento de tupla excede os limites do layout.".into());
                    }
                    if tamanho <= 16 {
                        let campo_em_dois_registros = deslocamento == 0 && tamanho_campo == 16;
                        let campo_escalar = matches!(deslocamento, 0 | 8) && (1..=8).contains(&tamanho_campo);
                        if !campo_em_dois_registros && !campo_escalar {
                            return Err("Elemento de tupla não cabe nos dois registradores ABI.".into());
                        }
                    }
                }
                Ok(())
            }
            Tipo::Enumeracao(nome) => {
                let layout = self.layouts_tipos.get(nome)
                    .ok_or_else(|| format!("Layout da enumeração '{nome}' não foi fornecido à ABI nativa."))?;
                if layout.tamanho == 0 || layout.tamanho > 16 {
                    return Err(format!("A enumeração '{nome}' excede o limite inicial de 16 bytes da ABI nativa."));
                }
                if layout.campos.get("payload").is_some_and(|(_, tamanho)| *tamanho > 8) {
                    return Err(format!("Payload da enumeração '{nome}' excede um registrador da ABI inicial."));
                }
                Ok(())
            }
            Tipo::Opcional(tipo) => {
                if self.largura_abi_tipo(tipo)? > 1 {
                    return Err("Payload de Opcional excede um registrador da ABI inicial.".into());
                }
                Ok(())
            }
            Tipo::Resultado(sucesso, falha) => {
                if self.largura_abi_tipo(sucesso)? > 1 || self.largura_abi_tipo(falha)? > 1 {
                    return Err("Payload de Resultado excede um registrador da ABI inicial.".into());
                }
                Ok(())
            }
            _ if validar_tipo(tipo).is_ok() => Ok(()),
            _ => Err(format!("Tipo '{}' ainda não possui ABI nativa implementada.", tipo.nome())),
        }
    }

    fn alinhar_layout(valor: u64, alinhamento: u64) -> Option<u64> {
        valor.checked_add(alinhamento.checked_sub(1)?)
            .map(|v| v & !(alinhamento - 1))
    }

    fn tamanho_alinhamento_tipo(&self, tipo: &Tipo) -> Option<(u64, u64)> {
        match tipo {
            Tipo::Vazio => Some((0, 1)),
            Tipo::Logico | Tipo::Byte | Tipo::Inteiro8 | Tipo::InteiroU8 => Some((1, 1)),
            Tipo::Inteiro16 | Tipo::InteiroU16 => Some((2, 2)),
            Tipo::Inteiro32 | Tipo::InteiroU32 | Tipo::Decimal32 => Some((4, 4)),
            Tipo::Inteiro | Tipo::Inteiro64 | Tipo::InteiroU64 | Tipo::Decimal | Tipo::Decimal64
            | Tipo::Funcao(_, _) | Tipo::Caractere => Some((8, 8)),
            Tipo::Texto => Some((16, 8)),
            Tipo::Inteiro128 | Tipo::InteiroU128 => Some((16, 16)),
            Tipo::Estrutura(nome) | Tipo::Enumeracao(nome) => self.layouts_tipos.get(nome)
                .map(|layout| (layout.tamanho, layout.alinhamento)),
            Tipo::Opcional(interno) => {
                let (tamanho, alinhamento) = self.tamanho_alinhamento_tipo(interno)?;
                let deslocamento = Self::alinhar_layout(1, alinhamento)?;
                Some((Self::alinhar_layout(deslocamento.checked_add(tamanho)?, alinhamento)?, alinhamento))
            }
            Tipo::Resultado(sucesso, falha) => {
                let (tamanho_ok, alinhamento_ok) = self.tamanho_alinhamento_tipo(sucesso)?;
                let (tamanho_erro, alinhamento_erro) = self.tamanho_alinhamento_tipo(falha)?;
                let alinhamento = alinhamento_ok.max(alinhamento_erro);
                let tamanho = tamanho_ok.max(tamanho_erro);
                let deslocamento = Self::alinhar_layout(1, alinhamento)?;
                Some((Self::alinhar_layout(deslocamento.checked_add(tamanho)?, alinhamento)?, alinhamento))
            }
            Tipo::Tupla(tipos) => {
                let (tamanho, _) = self.layout_tupla(tipos)?;
                let alinhamento = tipos.iter()
                    .map(|tipo| self.tamanho_alinhamento_tipo(tipo).map(|(_, alinhamento)| alinhamento))
                    .collect::<Option<Vec<_>>>()?
                    .into_iter()
                    .max()
                    .unwrap_or(1);
                Some((tamanho, alinhamento))
            }
            _ => None,
        }
    }

    fn layout_tupla(&self, tipos: &[Tipo]) -> Option<(u64, Vec<(u64, u64)>)> {
        let mut deslocamento = 0u64;
        let mut alinhamento_maximo = 1u64;
        let mut campos = Vec::with_capacity(tipos.len());
        for tipo in tipos {
            let (tamanho, alinhamento) = self.tamanho_alinhamento_tipo(tipo)?;
            deslocamento = Self::alinhar_layout(deslocamento, alinhamento)?;
            campos.push((deslocamento, tamanho));
            deslocamento = deslocamento.checked_add(tamanho)?;
            alinhamento_maximo = alinhamento_maximo.max(alinhamento);
        }
        Some((Self::alinhar_layout(deslocamento, alinhamento_maximo)?, campos))
    }

    fn largura_abi_tipo(&self, tipo: &Tipo) -> Result<usize, String> {
        self.validar_tipo_abi(tipo)?;
        if tipo_inteiro_128(tipo) {
            return Ok(2);
        }
        if let Tipo::Estrutura(nome) = tipo {
            let layout = self.layouts_tipos.get(nome)
                .ok_or_else(|| format!("Layout da estrutura '{nome}' ausente."))?;
            return Ok(if layout.tamanho > 16 { 1 } else if layout.tamanho > 8 { 2 } else { 1 });
        }
        if let Tipo::Tupla(tipos) = tipo {
            let (tamanho, _) = self.layout_tupla(tipos)
                .ok_or_else(|| "Layout de tupla não suportado pela ABI nativa.".to_owned())?;
            return Ok(if tamanho > 16 { 1 } else if tamanho > 8 { 2 } else { 1 });
        }
        if let Tipo::Enumeracao(nome) = tipo {
            let layout = self.layouts_tipos.get(nome)
                .ok_or_else(|| format!("Layout da enumeração '{nome}' ausente."))?;
            return Ok(if layout.campos.get("payload").is_some_and(|(_, tamanho)| *tamanho > 0) { 2 } else { 1 });
        }
        if matches!(tipo, Tipo::Opcional(_) | Tipo::Resultado(_, _)) {
            return Ok(2);
        }
        if tipo_decimal(tipo) {
            return Ok(1);
        }
        if *tipo == Tipo::Texto {
            return Ok(2);
        }
        Ok(1)
    }

    fn largura_abi_expressao(&self, expressao: &ExpressaoIr) -> Result<usize, String> {
        if let ExpressaoIr::Normalizar { tipo, .. } = expressao {
            self.largura_abi_tipo(tipo)
        } else if inteiro_128_expressao(expressao) {
            Ok(2)
        } else {
            Ok(1)
        }
    }

    fn iniciar_programa(&mut self, programa: &ProgramaIr) -> Result<(), String> {
        let principal = programa.funcoes.get("principal");
        if principal.is_none() && programa.instrucoes.is_empty() {
            return Err("Declare a função principal ou instruções globais.".into());
        }
        if principal.is_some_and(|f| !f.parametros.is_empty()) {
            return Err("A função principal não pode receber parâmetros no alvo inicial.".into());
        }
        if principal.is_some_and(|funcao| self.layout_agregado_grande(&funcao.retorno).is_some()) {
            return Err("A função principal não pode retornar agregado grande no ponto de entrada ELF.".into());
        }

        // O ponto de entrada chama as rotinas geradas e encerra pelo syscall Linux.
        let mut chamadas_inicio = Vec::new();
        if !programa.instrucoes.is_empty() {
            self.codigo.push(0xE8);
            chamadas_inicio.push((self.codigo.len(), NOME_INICIO_INTERNO.to_owned()));
            self.codigo.extend_from_slice(&[0, 0, 0, 0]);
        }
        if let Some(funcao) = principal {
            self.codigo.push(0xE8);
            chamadas_inicio.push((self.codigo.len(), funcao.nome.clone()));
            self.codigo.extend_from_slice(&[0, 0, 0, 0]);
        }
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7, 0xB8, 0x3C, 0, 0, 0, 0x0F, 0x05]);

        if !programa.instrucoes.is_empty() {
            let inicio = FuncaoIr {
                nome: NOME_INICIO_INTERNO.into(),
                parametros: Vec::new(),
                retorno: Tipo::Vazio,
                corpo: programa.instrucoes.clone(),
            };
            self.compilar_funcao(&inicio)?;
        }

        let mut nomes: Vec<_> = programa.funcoes.keys().cloned().collect();
        nomes.sort();
        for nome in nomes {
            self.compilar_funcao(&programa.funcoes[&nome])?;
        }
        self.chamadas.extend(chamadas_inicio);
        Ok(())
    }

    fn compilar_funcao(&mut self, funcao: &FuncaoIr) -> Result<(), String> {
        if self.funcoes.contains_key(&funcao.nome) {
            return Err(format!("Função duplicada no backend: '{}'.", funcao.nome));
        }
        self.validar_tipo_abi(&funcao.retorno)?;
        self.funcoes.insert(funcao.nome.clone(), self.codigo.len());
        self.funcao_atual = funcao.nome.clone();
        self.escopos.clear();
        self.escopos.push(HashMap::new());
        self.tipos_escopos.clear();
        self.tipos_escopos.push(HashMap::new());
        self.proximo_slot = 0;
        self.proximo_agregado = INICIO_AREA_AGREGADOS;
        self.retorno_oculto_atual = None;
        self.pilha = 0;
        self.lacos.clear();

        // Prólogo e quadro fixo: mantém alinhamento ABI e evita alocação dinâmica no compilador.
        self.codigo.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5, 0x48, 0x81, 0xEC]);
        self.codigo.extend_from_slice(&TAMANHO_QUADRO.to_le_bytes());

        let retorno_grande = self.layout_agregado_grande(&funcao.retorno);
        if let Some((tamanho, _)) = retorno_grande {
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xBD]);
            self.codigo.extend_from_slice(&(-(TAMANHO_QUADRO as i32)).to_le_bytes()); // salva RDI no fim do quadro
            self.retorno_oculto_atual = Some((-(TAMANHO_QUADRO as i32), tamanho));
        }
        let registradores = [7u8, 6, 2, 1, 8, 9]; // rdi, rsi, rdx, rcx, r8, r9
        let larguras = funcao.parametros.iter()
            .map(|(_, tipo)| self.largura_abi_tipo(tipo))
            .collect::<Result<Vec<_>, _>>()?;
        let posicoes = planejar_posicoes_argumentos(&larguras, usize::from(retorno_grande.is_some()))?;
        let mut slots_superiores = Vec::new();
        for ((nome, tipo), posicao) in funcao.parametros.iter().zip(posicoes) {
            self.validar_tipo_abi(tipo)?;
            let largura = self.largura_abi_tipo(tipo)?;
            let deslocamento = self.declarar_tipado(nome, tipo)?;
            match posicao {
                PosicaoArgumento::Registradores(indice) => {
                    self.mover_registrador_rax(registradores[indice]);
                    if largura == 2 {
                        self.armazenar_rax(deslocamento);
                        self.mover_registrador_rax(registradores[indice + 1]);
                        self.armazenar_rax(deslocamento - 8);
                        continue;
                    }
                }
                PosicaoArgumento::Pilha { deslocamento: pilha, largura: largura_pilha } => {
                    let origem = i32::try_from(16u64.checked_add(pilha)
                        .ok_or("Deslocamento de parâmetro excedeu o ABI x86-64.")?)
                        .map_err(|_| "Deslocamento de parâmetro excedeu o ABI x86-64.")?;
                    self.carregar_rax(origem);
                    if largura_pilha == 2 {
                        self.carregar_rdx(origem + 8);
                    }
                }
            }
            if !matches!(tipo, Tipo::Estrutura(_) | Tipo::Tupla(_) | Tipo::Enumeracao(_) | Tipo::Opcional(_) | Tipo::Resultado(_, _)) {
                self.normalizar_tipo(tipo)?;
            }
            self.armazenar_rax(deslocamento);
            if largura == 2 {
                self.armazenar_rdx(deslocamento - 8);
            } else {
                slots_superiores.push(deslocamento - 8);
            }
        }
        // Só zerar os slots auxiliares depois de capturar todos os argumentos:
        // RDX é também o terceiro registrador de entrada do ABI System V.
        for deslocamento in slots_superiores {
            self.mov_rdx(0);
            self.armazenar_rdx(deslocamento);
        }

        // Parâmetros e declarações do corpo compartilham o escopo da função.
        for instrucao in &funcao.corpo {
            self.instrucao(instrucao)?;
        }
        if let Some((tamanho, _)) = retorno_grande {
            let deslocamento = -(TAMANHO_QUADRO as i32);
            self.carregar_rax(deslocamento);
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7]); // rdi = destino
            self.codigo.extend_from_slice(&[0x48, 0xB9]);
            self.codigo.extend_from_slice(&tamanho.to_le_bytes());
            self.codigo.extend_from_slice(&[0x31, 0xC0, 0xF3, 0xAA]); // zera retorno implícito
            self.carregar_rax(deslocamento);
        } else {
            self.codigo.extend_from_slice(&[0x31, 0xC0]); // retorno implícito zero
        }
        self.codigo.extend_from_slice(&[0xC9, 0xC3]);
        self.retorno_oculto_atual = None;
        self.escopos.clear();
        self.tipos_escopos.clear();
        Ok(())
    }

    fn bloco(&mut self, instrucoes: &[InstrucaoIr]) -> Result<(), String> {
        self.escopos.push(HashMap::new());
        self.tipos_escopos.push(HashMap::new());
        for instrucao in instrucoes {
            self.instrucao(instrucao)?;
        }
        let _ = self.escopos.pop();
        let _ = self.tipos_escopos.pop();
        Ok(())
    }

    fn instrucao(&mut self, instrucao: &InstrucaoIr) -> Result<(), String> {
        match instrucao {
            InstrucaoIr::Localizada { instrucao, .. } => self.instrucao(instrucao)?,
            InstrucaoIr::Declarar { nome, tipo, valor, .. } => {
                self.validar_tipo_abi(tipo)?;
                let global = self.funcao_atual == NOME_INICIO_INTERNO
                    && self.escopos.len() == 1
                    && self.globais.contains_key(nome);
                if global {
                    if self.layout_agregado_grande(tipo).is_some() {
                        return Err(format!("Variável global '{nome}' de agregado grande ainda não possui armazenamento estático seguro."));
                    }
                    self.expressao(valor)?;
                    self.armazenar_global(nome);
                } else if let Some((tamanho, alinhamento)) = self.layout_agregado_grande(tipo) {
                    self.expressao(valor)?;
                    let origem = self.reservar_agregado(8, 8)?;
                    self.armazenar_rax(origem);
                    let destino = self.reservar_agregado(tamanho, alinhamento)?;
                    let deslocamento = self.declarar_tipado(nome, tipo)?;
                    self.emitir_endereco_rbp_rax(destino);
                    self.armazenar_rax(deslocamento);
                    self.carregar_rax(origem);
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = origem
                    self.emitir_endereco_rbp_rax(destino);
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7]); // rdi = destino
                    self.codigo.extend_from_slice(&[0x48, 0xB9]);
                    self.codigo.extend_from_slice(&tamanho.to_le_bytes());
                    self.codigo.extend_from_slice(&[0xF3, 0xA4]); // cópia por valor
                    self.mov_rdx(0);
                } else {
                    self.expressao(valor)?;
                    let deslocamento = self.declarar_tipado(nome, tipo)?;
                    self.armazenar_rax(deslocamento);
                    self.armazenar_rdx(deslocamento - 8);
                }
            }
            InstrucaoIr::Expressao(expressao) => self.expressao(expressao)?,
            InstrucaoIr::Bloco(instrucoes) => self.bloco(instrucoes)?,
            InstrucaoIr::Devolver(valor) => {
                if let Some((deslocamento, tamanho)) = self.retorno_oculto_atual {
                    let valor = valor.as_ref().ok_or("Retorno agregado grande exige um valor.")?;
                    self.expressao(valor)?;
                    self.push_rax(); // origem
                    self.carregar_rax(deslocamento);
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7]); // rdi = destino
                    self.pop_rax();
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = origem
                    self.codigo.extend_from_slice(&[0x48, 0xB9]);
                    self.codigo.extend_from_slice(&tamanho.to_le_bytes());
                    self.codigo.extend_from_slice(&[0xF3, 0xA4]); // rep movsb
                    self.carregar_rax(deslocamento); // ABI retorna o destino em RAX
                } else if let Some(valor) = valor {
                    self.expressao(valor)?;
                } else {
                    self.codigo.extend_from_slice(&[0x31, 0xC0]);
                }
                self.codigo.extend_from_slice(&[0xC9, 0xC3]);
            }
            InstrucaoIr::Escolha { valor, casos } => self.escolha(valor, casos)?,
            InstrucaoIr::Se { condicao, entao, senao } => {
                self.expressao(condicao)?;
                self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]);
                let desvio_senao = self.emitir_jcc(0x84);
                self.bloco(entao)?;
                if senao.is_empty() {
                    self.aplicar_salto(desvio_senao, self.codigo.len());
                } else {
                    let fim = self.emitir_jmp();
                    self.aplicar_salto(desvio_senao, self.codigo.len());
                    self.bloco(senao)?;
                    self.aplicar_salto(fim, self.codigo.len());
                }
            }
            InstrucaoIr::Enquanto { condicao, corpo } => {
                let inicio = self.codigo.len();
                self.expressao(condicao)?;
                self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]);
                let fim = self.emitir_jcc(0x84);
                self.lacos.push(Laco { saidas: Vec::new(), continuacoes: Vec::new() });
                self.bloco(corpo)?;
                let laco = self.lacos.pop().expect("laço ativo");
                self.emitir_salto_para(inicio);
                let destino_fim = self.codigo.len();
                self.aplicar_salto(fim, destino_fim);
                for salto in laco.saidas { self.aplicar_salto(salto, destino_fim); }
                for salto in laco.continuacoes { self.aplicar_salto(salto, inicio); }
            }
            InstrucaoIr::Cada { nome, iteravel, corpo } => {
                let tipo_iteravel = self.tipo_expressao_atual(iteravel)
                    .ok_or("O backend nativo não conseguiu determinar o tipo do iterável de 'cada'.")?;
                let Tipo::Intervalo(tipo_elemento) = tipo_iteravel else {
                    return Err("O backend ELF nativo aceita 'cada' sobre intervalos inteiros; listas e textos exigem runtime.".into());
                };
                let tipo_elemento = *tipo_elemento;
                let sem_sinal = matches!(
                    tipo_elemento,
                    Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32
                        | Tipo::InteiroU64 | Tipo::Byte
                );
                if !matches!(
                    tipo_elemento,
                    Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16
                        | Tipo::Inteiro32 | Tipo::Inteiro64
                        | Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32
                        | Tipo::InteiroU64 | Tipo::Byte
                ) {
                    return Err(format!(
                        "O backend ELF nativo não suporta 'cada' sobre intervalos do tipo '{}'.",
                        tipo_elemento.nome()
                    ));
                }

                self.escopos.push(HashMap::new());
                self.tipos_escopos.push(HashMap::new());
                let slot_inicio = self.declarar_tipado("\0nebula_cada_inicio", &tipo_elemento)?;
                let slot_fim = self.declarar_tipado("\0nebula_cada_fim", &tipo_elemento)?;
                let slot_cursor = self.declarar_tipado("\0nebula_cada_cursor", &tipo_elemento)?;
                let slot_descendente = self.declarar_tipado("\0nebula_cada_descendente", &Tipo::Logico)?;
                let slot_quantidade = self.declarar_tipado("\0nebula_cada_quantidade", &Tipo::InteiroU64)?;
                let slot_variavel = self.declarar_tipado(nome, &tipo_elemento)?;

                self.expressao(iteravel)?;
                self.armazenar_rax(slot_inicio);
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xD0]); // rax = limite final
                self.armazenar_rax(slot_fim);
                self.carregar_rax(slot_inicio);
                self.armazenar_rax(slot_cursor);

                self.carregar_rax(slot_inicio);
                self.push_rax();
                self.carregar_rax(slot_fim);
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = fim
                self.pop_rax();
                self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]); // cmp rax, rcx
                let salto_descendente = self.emitir_jcc(if sem_sinal { 0x87 } else { 0x8F });
                self.mov_rax(0);
                self.armazenar_rax(slot_descendente);
                let salto_inicio = self.emitir_jmp();
                let inicio_descendente = self.codigo.len();
                self.mov_rax(1);
                self.armazenar_rax(slot_descendente);
                let inicio_laco = self.codigo.len();
                self.aplicar_salto(salto_descendente, inicio_descendente);
                self.aplicar_salto(salto_inicio, inicio_laco);

                self.carregar_rax(slot_descendente);
                self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]);
                let verificar_descendente = self.emitir_jcc(0x85);

                self.carregar_rax(slot_cursor);
                self.push_rax();
                self.carregar_rax(slot_fim);
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]);
                self.pop_rax();
                self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]);
                let sair_ascendente = self.emitir_jcc(if sem_sinal { 0x87 } else { 0x8F });
                let saltar_corpo = self.emitir_jmp();

                let inicio_verificacao_descendente = self.codigo.len();
                self.carregar_rax(slot_cursor);
                self.push_rax();
                self.carregar_rax(slot_fim);
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]);
                self.pop_rax();
                self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]);
                let sair_descendente = self.emitir_jcc(if sem_sinal { 0x82 } else { 0x8C });
                let inicio_corpo = self.codigo.len();
                self.aplicar_salto(verificar_descendente, inicio_verificacao_descendente);
                self.aplicar_salto(saltar_corpo, inicio_corpo);

                self.mov_rax(0);
                self.armazenar_rax(slot_quantidade);
                self.mov_rdx(0);
                self.armazenar_rdx(slot_quantidade - 8);
                self.carregar_rax(slot_cursor);
                self.armazenar_rax(slot_variavel);
                self.mov_rdx(0);
                self.armazenar_rdx(slot_variavel - 8);
                self.lacos.push(Laco { saidas: Vec::new(), continuacoes: Vec::new() });
                self.bloco(corpo)?;
                let laco = self.lacos.pop().expect("laço ativo");
                let inicio_atualizacao = self.codigo.len();
                for salto in laco.continuacoes {
                    self.aplicar_salto(salto, inicio_atualizacao);
                }

                self.carregar_rax(slot_cursor);
                self.push_rax();
                self.carregar_rax(slot_fim);
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]);
                self.pop_rax();
                self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]);
                let chegou_ao_fim = self.emitir_jcc(0x84);

                self.carregar_rax(slot_descendente);
                self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]);
                let decrementar = self.emitir_jcc(0x85);
                self.carregar_rax(slot_cursor);
                self.codigo.extend_from_slice(&[0x48, 0xFF, 0xC0]); // inc rax
                self.armazenar_rax(slot_cursor);
                self.emitir_salto_para(inicio_laco);

                let inicio_decremento = self.codigo.len();
                self.carregar_rax(slot_cursor);
                self.codigo.extend_from_slice(&[0x48, 0xFF, 0xC8]); // dec rax
                self.armazenar_rax(slot_cursor);
                self.emitir_salto_para(inicio_laco);

                let fim_laco = self.codigo.len();
                self.aplicar_salto(decrementar, inicio_decremento);
                self.aplicar_salto(chegou_ao_fim, fim_laco);
                self.aplicar_salto(sair_ascendente, fim_laco);
                self.aplicar_salto(sair_descendente, fim_laco);
                for salto in laco.saidas {
                    self.aplicar_salto(salto, fim_laco);
                }
                let _ = self.escopos.pop();
                let _ = self.tipos_escopos.pop();
            }
            InstrucaoIr::Pulso { intervalo, corpo } => {
                let tipo_intervalo = self.tipo_expressao_atual(intervalo)
                    .ok_or("O backend nativo não conseguiu determinar o tipo do intervalo de 'pulso'.")?;
                let sem_sinal = matches!(
                    tipo_intervalo,
                    Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32
                        | Tipo::InteiroU64 | Tipo::Byte
                );
                if !matches!(
                    tipo_intervalo,
                    Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16
                        | Tipo::Inteiro32 | Tipo::Inteiro64
                        | Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32
                        | Tipo::InteiroU64 | Tipo::Byte
                ) {
                    return Err(format!(
                        "O backend ELF nativo não suporta o intervalo de 'pulso' do tipo '{}'.",
                        tipo_intervalo.nome()
                    ));
                }

                self.escopos.push(HashMap::new());
                self.tipos_escopos.push(HashMap::new());
                let slot_intervalo = self.declarar_tipado("\0nebula_pulso_intervalo", &tipo_intervalo)?;
                let timespec = self.reservar_agregado(16, 8)?;
                self.expressao(intervalo)?;
                if !sem_sinal {
                    self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
                    let nao_negativo = self.emitir_jcc(0x89); // jns
                    self.encerrar_com_erro(2);
                    self.aplicar_salto(nao_negativo, self.codigo.len());
                }
                self.armazenar_rax(slot_intervalo);

                let inicio_laco = self.codigo.len();
                self.lacos.push(Laco { saidas: Vec::new(), continuacoes: Vec::new() });
                self.bloco(corpo)?;
                let laco = self.lacos.pop().expect("laço ativo");
                let inicio_sono = self.codigo.len();
                for salto in laco.continuacoes {
                    self.aplicar_salto(salto, inicio_sono);
                }

                self.carregar_rax(slot_intervalo);
                self.codigo.extend_from_slice(&[
                    0x48, 0xB9, 0xE8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                ]); // mov rcx, 1000
                self.codigo.extend_from_slice(&[0x48, 0x31, 0xD2]); // xor rdx, rdx
                self.codigo.extend_from_slice(&[0x48, 0xF7, 0xF1]); // div rcx
                self.armazenar_rax(timespec); // tv_sec
                self.mov_rax(1_000_000);
                self.codigo.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xD0]); // imul rdx, rax
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xD0]); // rax = tv_nsec
                self.armazenar_rax(timespec + 8);
                self.emitir_endereco_rbp_rax(timespec);
                self.mover_rax_registrador(7); // rdi = timespec
                self.codigo.extend_from_slice(&[0x31, 0xF6]); // rsi = NULL
                self.mov_rax(35); // nanosleep
                self.codigo.extend_from_slice(&[0x0F, 0x05]); // syscall
                self.emitir_salto_para(inicio_laco);

                let fim_laco = self.codigo.len();
                for salto in laco.saidas {
                    self.aplicar_salto(salto, fim_laco);
                }
                let _ = self.escopos.pop();
                let _ = self.tipos_escopos.pop();
            }
            InstrucaoIr::Para { inicial, condicao, atualizacao, corpo } => {
                self.escopos.push(HashMap::new());
                self.tipos_escopos.push(HashMap::new());
                if let Some(inicial) = inicial { self.instrucao(inicial)?; }
                let inicio_condicao = self.codigo.len();
                let fim_condicao = if let Some(condicao) = condicao {
                    self.expressao(condicao)?;
                    self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]);
                    Some(self.emitir_jcc(0x84))
                } else { None };
                self.lacos.push(Laco { saidas: Vec::new(), continuacoes: Vec::new() });
                self.bloco(corpo)?;
                let inicio_atualizacao = self.codigo.len();
                if let Some(atualizacao) = atualizacao { self.expressao(atualizacao)?; }
                let laco = self.lacos.pop().expect("laço ativo");
                self.emitir_salto_para(inicio_condicao);
                for salto in laco.continuacoes { self.aplicar_salto(salto, inicio_atualizacao); }
                let destino_fim = self.codigo.len();
                if let Some(salto) = fim_condicao { self.aplicar_salto(salto, destino_fim); }
                for salto in laco.saidas { self.aplicar_salto(salto, destino_fim); }
                let _ = self.escopos.pop();
                let _ = self.tipos_escopos.pop();
            }
            InstrucaoIr::Parar => {
                let salto = self.emitir_jmp();
                self.lacos.last_mut().ok_or("parar fora de laço no backend.")?.saidas.push(salto);
            }
            InstrucaoIr::Seguir => {
                let salto = self.emitir_jmp();
                self.lacos.last_mut().ok_or("seguir fora de laço no backend.")?.continuacoes.push(salto);
            }
        }
        Ok(())
    }

    fn expressao(&mut self, expressao: &ExpressaoIr) -> Result<(), String> {
        match expressao {
            ExpressaoIr::Normalizar { tipo, valor } => {
                if let ExpressaoIr::Membro { alvo, campo } = valor.as_ref() {
                    if self.alvo_estrutura_grande(alvo) {
                        self.carregar_membro_agregado(alvo, campo, tipo)?;
                        if !matches!(tipo, Tipo::Estrutura(_) | Tipo::Tupla(_) | Tipo::Enumeracao(_) | Tipo::Opcional(_) | Tipo::Resultado(_, _)) {
                            self.normalizar_tipo(tipo)?;
                        }
                        return Ok(());
                    }
                }
                if matches!(tipo, Tipo::Estrutura(_) | Tipo::Enumeracao(_) | Tipo::Opcional(_) | Tipo::Resultado(_, _)) {
                    self.validar_tipo_abi(tipo)?;
                    self.expressao(valor)?;
                } else if let Tipo::Tupla(tipos) = tipo {
                    self.validar_tipo_abi(tipo)?;
                    if let ExpressaoIr::Tupla(valores) = valor.as_ref() {
                        self.construir_tupla(tipos, valores)?;
                    } else {
                        self.expressao(valor)?;
                    }
                } else if tipo_inteiro_128(tipo) {
                    match valor.as_ref() {
                        ExpressaoIr::Unaria { operador, valor: interno }
                            if *operador == OperadorIr::Menos =>
                        {
                            let interno = ExpressaoIr::Normalizar {
                                tipo: tipo.clone(),
                                valor: interno.clone(),
                            };
                            self.expressao(&ExpressaoIr::Unaria {
                                operador: *operador,
                                valor: Box::new(interno),
                            })?;
                        }
                        ExpressaoIr::Binaria { esquerda, operador, direita }
                            if matches!(operador, OperadorIr::Mais | OperadorIr::Menos
                                | OperadorIr::Vezes | OperadorIr::Divide | OperadorIr::Resto) =>
                        {
                            let esquerda = ExpressaoIr::Normalizar {
                                tipo: tipo.clone(),
                                valor: esquerda.clone(),
                            };
                            let direita = ExpressaoIr::Normalizar {
                                tipo: tipo.clone(),
                                valor: direita.clone(),
                            };
                            self.expressao(&ExpressaoIr::Binaria {
                                esquerda: Box::new(esquerda),
                                operador: *operador,
                                direita: Box::new(direita),
                            })?;
                        }
                        _ => self.expressao(valor)?,
                    }
                } else {
                    self.expressao(valor)?;
                }
                if !matches!(
                    tipo,
                    Tipo::Estrutura(_) | Tipo::Tupla(_) | Tipo::Enumeracao(_)
                        | Tipo::Opcional(_) | Tipo::Resultado(_, _) | Tipo::Intervalo(_)
                ) {
                    self.normalizar_tipo(tipo)?;
                }
            },
            ExpressaoIr::ReferenciaFuncao { nome, .. } => self.emitir_referencia_funcao(nome),
            ExpressaoIr::ReferenciaFuncaoQualificada { modulo, nome, .. } => {
                self.emitir_referencia_funcao(&format!("{modulo}::{nome}"));
            }
            ExpressaoIr::ChamadaIndireta { alvo, argumentos } => {
                let retorno_grande = tipo_expressao_ir(alvo).and_then(|tipo| {
                    if let Tipo::Funcao(_, retorno) = tipo {
                        self.layout_agregado_grande(retorno)
                    } else {
                        None
                    }
                });
                self.expressao(alvo)?;
                let funcao_slot = self.reservar_agregado(8, 8)?;
                self.armazenar_rax(funcao_slot);
                let (tamanho_pilha, preenchimento) = if let Some((tamanho, alinhamento)) = retorno_grande {
                    let destino = self.reservar_agregado(tamanho, alinhamento)?;
                    let destino_slot = self.reservar_agregado(8, 8)?;
                    self.emitir_endereco_rbp_rax(destino);
                    self.armazenar_rax(destino_slot);
                    let preparado = self.preparar_argumentos_desde(argumentos, 1)?;
                    self.carregar_rax(destino_slot);
                    self.mover_rax_registrador(7);
                    preparado
                } else {
                    self.preparar_argumentos(argumentos)?
                };
                self.carregar_rax(funcao_slot);
                self.codigo.extend_from_slice(&[0x49, 0x89, 0xC3]); // r11 = função
                self.codigo.extend_from_slice(&[0x41, 0xFF, 0xD3]); // call r11
                self.limpar_pilha_chamada(tamanho_pilha, preenchimento)?;
            }
            ExpressaoIr::Conversao { tipo, origem, valor } => {
                self.expressao(valor)?;
                self.converter_tipo_explicito(tipo, origem)?;
            },
            ExpressaoIr::Constante(ValorIr::Inteiro(valor)) => {
                self.mov_rax(*valor as u64 as i64);
                self.mov_rdx((*valor >> 64) as i64);
            }
            ExpressaoIr::Constante(ValorIr::InteiroSemSinal(valor)) => {
                self.mov_rax(*valor as u64 as i64);
                self.mov_rdx((*valor >> 64) as i64);
            }
            ExpressaoIr::Constante(ValorIr::Decimal(valor)) => self.mov_rax(valor.to_bits() as i64),
            ExpressaoIr::Constante(ValorIr::Texto(texto)) => {
                self.emitir_texto_estatico(texto)?;
            }
            ExpressaoIr::Constante(ValorIr::Logico(valor)) => self.mov_rax(i64::from(*valor)),
            ExpressaoIr::Constante(ValorIr::Caractere(valor)) => self.mov_rax(*valor as i64),
            ExpressaoIr::Constante(ValorIr::Lista(_)
                | ValorIr::Estrutura { .. }
                | ValorIr::Intervalo(_, _)
                | ValorIr::IntervaloSemSinal(_, _)
                | ValorIr::IntervaloDecimal(_, _)
                | ValorIr::Nulo
                | ValorIr::Funcao(_)
                | ValorIr::Vazio) => return Err("Literal ainda não suportado pelo backend nativo sem runtime.".into()),
            ExpressaoIr::Variavel(nome) => {
                if let Some(deslocamento) = self.buscar_local(nome) {
                    self.carregar_rax(deslocamento);
                    self.carregar_rdx(deslocamento - 8);
                } else if self.globais.contains_key(nome) {
                    self.carregar_global(nome);
                } else {
                    return Err(format!("Variável '{nome}' não encontrada no backend."));
                }
            }
            ExpressaoIr::Lista(_) => {
                return Err("Listas aguardam a implementação do suporte de memória/runtime.".into());
            }
            ExpressaoIr::Tupla(_) => {
                return Err("Tupla precisa de tipo contextual para a ABI nativa.".into());
            }
            ExpressaoIr::Indexacao { alvo, indice } => {
                let Some(Tipo::Tupla(tipos)) = tipo_expressao_ir(alvo) else {
                    return Err("Indexação nativa está disponível somente para tuplas tipadas.".into());
                };
                let posicao = indice_literal_ir(indice)
                    .ok_or("A indexação nativa de tupla exige índice literal.")?;
                let (_, campos) = self.layout_tupla(tipos)
                    .ok_or("Layout da tupla não suportado pela ABI nativa.")?;
                let (deslocamento, _) = campos.get(posicao)
                    .ok_or("Índice de tupla fora dos limites.")?;
                self.expressao(alvo)?;
                match deslocamento {
                    0 => {}
                    8 => self.codigo.extend_from_slice(&[0x48, 0x89, 0xD0]),
                    _ => return Err("Elemento da tupla não está em um registrador ABI.".into()),
                }
            }
            ExpressaoIr::Membro { alvo, campo } => {
                let Some(Tipo::Estrutura(nome)) = tipo_expressao_ir(alvo) else {
                    return Err("Acesso de campo nativo exige uma estrutura com tipo conhecido.".into());
                };
                let layout = self.layouts_tipos.get(nome)
                    .ok_or_else(|| format!("Layout da estrutura '{nome}' ausente no backend."))?;
                let (deslocamento, tamanho_campo, tamanho_agregado) = layout.campos.get(campo)
                    .map(|(deslocamento, tamanho)| (*deslocamento, *tamanho, layout.tamanho))
                    .ok_or_else(|| format!("Campo '{campo}' não encontrado no layout de '{nome}'."))?;
                if tamanho_agregado > 16 {
                    let tipo_campo = match tamanho_campo {
                        1 => Tipo::Byte,
                        2 => Tipo::InteiroU16,
                        4 => Tipo::InteiroU32,
                        8 => Tipo::Inteiro64,
                        9..=16 => Tipo::Inteiro128,
                        _ => Tipo::Estrutura(nome.clone()),
                    };
                    self.carregar_membro_agregado(alvo, campo, &tipo_campo)?;
                    return Ok(());
                }
                self.expressao(alvo)?;
                match deslocamento {
                    0 => {}
                    8 => self.codigo.extend_from_slice(&[0x48, 0x89, 0xD0]),
                    _ => return Err(format!("Campo '{campo}' não está em um dos dois registradores ABI.")),
                }
            }
            ExpressaoIr::ConstrucaoEstrutura { nome, campos } => {
                let layout = self.layouts_tipos.get(nome)
                    .ok_or_else(|| format!("Layout da estrutura '{nome}' ausente no backend."))?;
                if layout.tamanho > 16 {
                    let tamanho = layout.tamanho;
                    let alinhamento = layout.alinhamento;
                    let offsets = layout.campos.clone();
                    let armazenamento = self.reservar_agregado(tamanho, alinhamento)?;
                    self.emitir_endereco_rbp_rax(armazenamento);
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7]); // rdi = destino
                    self.codigo.extend_from_slice(&[0x48, 0xB9]);
                    self.codigo.extend_from_slice(&tamanho.to_le_bytes()); // rcx = tamanho
                    self.codigo.extend_from_slice(&[0x31, 0xC0, 0xF3, 0xAA]); // zero com rep stosb
                    for (campo, valor) in campos {
                        let (deslocamento, tamanho_campo) = offsets.get(campo)
                            .copied()
                            .ok_or_else(|| format!("Campo '{campo}' não existe em '{nome}'."))?;
                        self.expressao(valor)?;
                        self.armazenar_campo_agregado(armazenamento, deslocamento, tamanho_campo)?;
                    }
                    self.emitir_endereco_rbp_rax(armazenamento);
                    self.mov_rdx(0);
                    return Ok(());
                }
                let primeiro = campos.iter().find(|(campo, _)| {
                    layout.campos.get(campo).is_some_and(|(deslocamento, _)| *deslocamento == 0)
                }).ok_or_else(|| format!("Estrutura '{nome}' não possui campo inicializável no primeiro registrador."))?;
                let tamanho_primeiro = layout.campos.get(&primeiro.0).map(|(_, tamanho)| *tamanho).unwrap_or(0);
                if tamanho_primeiro == 16 {
                    return self.expressao(&primeiro.1);
                }
                let segundo = campos.iter().find(|(campo, _)| {
                    layout.campos.get(campo).is_some_and(|(deslocamento, _)| *deslocamento == 8)
                });
                self.expressao(&primeiro.1)?;
                self.push_rax();
                if let Some((_, valor)) = segundo {
                    self.expressao(valor)?;
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC2]);
                } else {
                    self.mov_rdx(0);
                }
                self.pop_rax();
            }
            ExpressaoIr::ConstrucaoEnumeracao { nome, variante, valores } => {
                if valores.len() > 1 {
                    return Err("A ABI inicial de enumerações aceita no máximo um valor de payload por variante.".into());
                }
                if let Some(valor) = valores.first() {
                    if self.largura_abi_expressao(valor)? > 1 {
                        return Err("Payload de enumeração não pode ocupar mais de um registrador na ABI inicial.".into());
                    }
                    self.expressao(valor)?;
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC2]); // mov rdx, rax
                } else {
                    self.mov_rdx(0);
                }
                self.mov_rax(tag_enumeracao(nome_enum_canonico(nome), variante));
            }
            ExpressaoIr::Unaria { operador: OperadorIr::Menos, valor }
                if matches!(valor.as_ref(), ExpressaoIr::Constante(ValorIr::Inteiro(v)) if *v == i64::MAX as i128 + 1) => {
                self.mov_rax(i64::MIN);
            }
            ExpressaoIr::Unaria { operador: OperadorIr::Menos, valor }
                if inteiro_128_expressao(valor) => {
                if inteiro_128_sem_sinal_expressao(valor) {
                    return Err("Negação de inteiro_u128 não é suportada como operação assinada.".into());
                }
                self.expressao(valor)?;
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = low original
                self.mov_rax(i64::MIN);
                self.codigo.extend_from_slice(&[0x48, 0x39, 0xC2]); // cmp rdx, rax
                let nao_minimo_alto = self.emitir_jcc(0x85);
                self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]); // test low original
                let nao_minimo_baixo = self.emitir_jcc(0x85);
                self.encerrar_com_erro(2);
                let continuar = self.codigo.len();
                self.aplicar_salto(nao_minimo_alto, continuar);
                self.aplicar_salto(nao_minimo_baixo, continuar);
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xC8]); // rax = low original
                self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]);
                let baixo_zero = self.emitir_jcc(0x84);
                self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD8]); // neg rax
                self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD2]); // not rdx
                let fim_negacao = self.emitir_jmp();
                let destino_baixo_zero = self.codigo.len();
                self.codigo.extend_from_slice(&[0x48, 0xF7, 0xDA]); // neg rdx
                self.codigo.extend_from_slice(&[0x48, 0x31, 0xC0]); // xor rax, rax
                let fim = self.codigo.len();
                self.aplicar_salto(baixo_zero, destino_baixo_zero);
                self.aplicar_salto(fim_negacao, fim);
            }
            ExpressaoIr::Unaria { operador, valor } => {
                self.expressao(valor)?;
                match operador {
                    OperadorIr::Menos => {
                        self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD8]);
                        self.verificar_overflow();
                    }
                    OperadorIr::Nao | OperadorIr::Negacao => {
                        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0, 0x0F, 0x94, 0xC0, 0x48, 0x0F, 0xB6, 0xC0]);
                    }
                    OperadorIr::Mais => {}
                    _ => return Err("Operador unário não suportado.".into()),
                }
            }
            ExpressaoIr::Binaria { esquerda, operador, direita } => {
                if *operador == OperadorIr::Mais
                    && self.tipo_expressao_atual(esquerda).as_ref() == Some(&Tipo::Texto)
                    && self.tipo_expressao_atual(direita).as_ref() == Some(&Tipo::Texto)
                {
                    self.concatenar_textos(esquerda, direita)?;
                    return Ok(());
                }
                if self.tipo_expressao_atual(esquerda).as_ref() == Some(&Tipo::Texto)
                    && matches!(operador, OperadorIr::Igual | OperadorIr::Diferente)
                {
                    self.emitir_igualdade_texto(
                        esquerda,
                        direita,
                        *operador == OperadorIr::Diferente,
                    )?;
                    return Ok(());
                }
                if inteiro_128_expressao(esquerda) {
                    self.expressao(esquerda)?;
                    self.codigo.push(0x52); // push rdx = lhs high
                    self.pilha += 1;
                    self.push_rax(); // push rax = lhs low
                    self.expressao(direita)?;
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = rhs low
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xD6]); // rsi = rhs high
                    self.pop_rax();
                    self.codigo.extend_from_slice(&[0x5A]); // pop rdx = lhs high
                    self.pilha = self.pilha.saturating_sub(1);
                    let sem_sinal = inteiro_128_sem_sinal_expressao(esquerda);
                    match operador {
                        OperadorIr::Mais => {
                            self.codigo.extend_from_slice(&[0x48, 0x01, 0xC8]); // add rax, rcx
                            self.codigo.extend_from_slice(&[0x48, 0x11, 0xF2]); // adc rdx, rsi
                            if sem_sinal { self.verificar_carry(); } else { self.verificar_overflow(); }
                        }
                        OperadorIr::Vezes => self.multiplicar_inteiro_128(sem_sinal)?,
                        OperadorIr::Divide | OperadorIr::Resto =>
                            self.dividir_inteiro_128(*operador == OperadorIr::Resto, sem_sinal)?,
                        OperadorIr::Menos => {
                            self.codigo.extend_from_slice(&[0x48, 0x29, 0xC8]); // sub rax, rcx
                            self.codigo.extend_from_slice(&[0x48, 0x19, 0xF2]); // sbb rdx, rsi
                            if sem_sinal { self.verificar_carry(); } else { self.verificar_overflow(); }
                        }
                        OperadorIr::Igual | OperadorIr::Diferente => {
                            self.codigo.extend_from_slice(&[0x48, 0x31, 0xC8]); // xor low
                            self.codigo.extend_from_slice(&[0x48, 0x31, 0xF2]); // xor high
                            self.codigo.extend_from_slice(&[0x48, 0x09, 0xD0]); // or halves
                            self.codigo.extend_from_slice(if *operador == OperadorIr::Igual {
                                &[0x0F, 0x94, 0xC0]
                            } else {
                                &[0x0F, 0x95, 0xC0]
                            });
                            self.codigo.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]);
                        }
                        OperadorIr::Menor | OperadorIr::MenorIgual
                        | OperadorIr::Maior | OperadorIr::MaiorIgual => {
                            self.comparar_inteiro_128(operador, sem_sinal);
                        }
                        _ => return Err("Operação de 128 bits ainda não suportada no backend nativo.".into()),
                    }
                    return Ok(());
                }
                if *operador == OperadorIr::Intervalo {
                    if inteiro_128_expressao(esquerda)
                        || inteiro_128_expressao(direita)
                        || tipo_decimal_expressao(esquerda).is_some()
                    {
                        return Err("O backend ELF nativo aceita intervalos inteiros de até 64 bits.".into());
                    }
                    self.expressao(esquerda)?;
                    self.push_rax();
                    self.expressao(direita)?;
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = limite final
                    self.pop_rax(); // rax = limite inicial
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xCA]); // rdx = limite final
                    return Ok(());
                }
                let operacao_sem_sinal = eh_inteiro_sem_sinal(esquerda);
                let tipo_decimal = tipo_decimal_expressao(esquerda);
                self.expressao(esquerda)?;
                if *operador == OperadorIr::E || *operador == OperadorIr::Ou {
                    self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]);
                    let salto_curto = self.emitir_jcc(if *operador == OperadorIr::E { 0x84 } else { 0x85 });
                    self.expressao(direita)?;
                    self.normalizar_logico();
                    let fim = self.emitir_jmp();
                    self.aplicar_salto(salto_curto, self.codigo.len());
                    self.mov_rax(i64::from(*operador == OperadorIr::Ou));
                    self.aplicar_salto(fim, self.codigo.len());
                    return Ok(());
                }
                self.push_rax(); // preserva o operando esquerdo
                self.expressao(direita)?;
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx=rhs
                self.pop_rax(); // rax=lhs
                if let Some(tipo) = tipo_decimal {
                    match operador {
                        OperadorIr::Mais | OperadorIr::Menos | OperadorIr::Vezes | OperadorIr::Divide =>
                            self.operar_decimal(&tipo, operador)?,
                        OperadorIr::Igual | OperadorIr::Diferente | OperadorIr::Menor
                        | OperadorIr::MenorIgual | OperadorIr::Maior | OperadorIr::MaiorIgual =>
                            self.comparar_decimal(&tipo, operador),
                        _ => return Err("Operação não suportada para tipos decimais no backend nativo.".into()),
                    }
                    return Ok(());
                }
                match operador {
                    OperadorIr::Mais => {
                        self.codigo.extend_from_slice(&[0x48, 0x01, 0xC8]);
                        if operacao_sem_sinal { self.verificar_carry(); } else { self.verificar_overflow(); }
                    }
                    OperadorIr::Menos => {
                        self.codigo.extend_from_slice(&[0x48, 0x29, 0xC8]);
                        if operacao_sem_sinal { self.verificar_carry(); } else { self.verificar_overflow(); }
                    }
                    OperadorIr::Vezes => {
                        if operacao_sem_sinal {
                            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xE1]); // mul rcx
                            self.codigo.extend_from_slice(&[0x48, 0x85, 0xD2]); // test rdx, rdx
                            let valido = self.emitir_jcc(0x84);
                            self.encerrar_com_erro(2);
                            self.aplicar_salto(valido, self.codigo.len());
                        } else {
                            self.codigo.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]);
                            self.verificar_overflow();
                        }
                    }
                    OperadorIr::Divide | OperadorIr::Resto => {
                        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]);
                        let divisor_valido = self.emitir_jcc(0x85);
                        self.encerrar_com_erro(2);
                        self.aplicar_salto(divisor_valido, self.codigo.len());
                        if operacao_sem_sinal {
                            self.codigo.extend_from_slice(&[0x48, 0x31, 0xD2, 0x48, 0xF7, 0xF1]); // xor rdx,rdx; div rcx
                        } else {
                            self.codigo.extend_from_slice(&[0x48, 0x83, 0xF9, 0xFF]);
                            let divisor_normal = self.emitir_jcc(0x85);
                            self.codigo.extend_from_slice(&[0x48, 0xBA]);
                            self.codigo.extend_from_slice(&i64::MIN.to_le_bytes());
                            self.codigo.extend_from_slice(&[0x48, 0x39, 0xD0]);
                            let dividendo_normal = self.emitir_jcc(0x85);
                            self.encerrar_com_erro(2);
                            let inicio_divisao = self.codigo.len();
                            self.aplicar_salto(divisor_normal, inicio_divisao);
                            self.aplicar_salto(dividendo_normal, inicio_divisao);
                            self.codigo.extend_from_slice(&[0x48, 0x99, 0x48, 0xF7, 0xF9]);
                        }
                        if *operador == OperadorIr::Resto { self.codigo.extend_from_slice(&[0x48, 0x89, 0xD0]); }
                    }
                    OperadorIr::Potencia => {
                        // Exponenciação inteira por quadratura: O(log expoente), sem runtime.
                        // Expoente negativo é rejeitado com código de saída 2.
                        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]);
                        let expoente_nao_negativo = self.emitir_jcc(0x89); // JNS
                        self.encerrar_com_erro(2);
                        self.aplicar_salto(expoente_nao_negativo, self.codigo.len());
                        // A IR de referência define expoentes inteiros no intervalo u32.
                        self.codigo.extend_from_slice(&[0x41, 0xB8, 0xFF, 0xFF, 0xFF, 0xFF, 0x4C, 0x39, 0xC1]); // r8d = u32::MAX; cmp rcx, r8
                        let expoente_no_intervalo = self.emitir_jcc(0x86); // JBE
                        self.encerrar_com_erro(2);
                        self.aplicar_salto(expoente_no_intervalo, self.codigo.len());
                        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC2]); // rdx = base
                        self.mov_rax(1); // acumulador
                        let inicio = self.codigo.len();
                        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]);
                        let fim = self.emitir_jcc(0x84);
                        self.codigo.extend_from_slice(&[0xF6, 0xC1, 0x01]);
                        let multiplicar = self.emitir_jcc(0x84);
                        self.codigo.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]);
                        self.verificar_overflow();
                        self.aplicar_salto(multiplicar, self.codigo.len());
                        self.codigo.extend_from_slice(&[0x48, 0xD1, 0xE9]); // expoente >>= 1
                        let terminar = self.emitir_jcc(0x84);
                        self.codigo.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xD2]); // base *= base
                        self.verificar_overflow();
                        self.emitir_salto_para(inicio);
                        let destino_fim = self.codigo.len();
                        self.aplicar_salto(fim, destino_fim);
                        self.aplicar_salto(terminar, destino_fim);
                    }
                    OperadorIr::Igual | OperadorIr::Diferente | OperadorIr::Menor | OperadorIr::MenorIgual | OperadorIr::Maior | OperadorIr::MaiorIgual => {
                        self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]);
                        let set = match operador {
                            OperadorIr::Igual => 0x94, OperadorIr::Diferente => 0x95,
                            OperadorIr::Menor if operacao_sem_sinal => 0x92,
                            OperadorIr::MenorIgual if operacao_sem_sinal => 0x96,
                            OperadorIr::Maior if operacao_sem_sinal => 0x97,
                            OperadorIr::MaiorIgual if operacao_sem_sinal => 0x93,
                            OperadorIr::Menor => 0x9C, OperadorIr::MenorIgual => 0x9E,
                            OperadorIr::Maior => 0x9F, OperadorIr::MaiorIgual => 0x9D,
                            _ => unreachable!(),
                        };
                        self.codigo.extend_from_slice(&[0x0F, set, 0xC0, 0x48, 0x0F, 0xB6, 0xC0]);
                    }
                    OperadorIr::E | OperadorIr::Ou => unreachable!(),
                    OperadorIr::Intervalo | OperadorIr::Negacao | OperadorIr::Nao => {
                        return Err("Operador ainda não implementado no backend nativo.".into());
                    }
                }
            }
            ExpressaoIr::AtribuicaoCampo { raiz, caminho, valor, tipo } => {
                if caminho.len() != 1 {
                    return Err("Atribuição nativa suporta somente campos diretos de estrutura.".into());
                }
                let tipo_raiz = self.buscar_tipo_local(raiz)
                    .or_else(|| self.tipos_globais.get(raiz))
                    .cloned()
                    .ok_or_else(|| format!("Tipo da variável '{raiz}' não foi encontrado para atribuição de campo."))?;
                let Tipo::Estrutura(nome) = &tipo_raiz else {
                    return Err("Atribuição de campo exige uma variável de estrutura.".into());
                };
                self.validar_tipo_abi(&tipo_raiz)?;
                let (deslocamento, tamanho, tamanho_agregado) = self.layouts_tipos.get(nome)
                    .and_then(|layout| layout.campos.get(&caminho[0]).map(|(offset, size)| (*offset, *size, layout.tamanho)))
                    .ok_or_else(|| format!("Campo '{}' não encontrado em '{nome}'.", caminho[0]))?;
                let largura = self.largura_abi_tipo(tipo)?;
                let largura_esperada = if tamanho > 16 { 1 } else if tamanho > 8 { 2 } else { 1 };
                if largura != largura_esperada {
                    return Err("Tipo do valor não corresponde à largura do campo no ABI.".into());
                }
                if tamanho_agregado > 16 {
                    let base = self.buscar_local(raiz)
                        .ok_or_else(|| "Agregado grande global não possui armazenamento estático seguro.".to_owned())?;
                    let temporario = self.reservar_agregado(16, 16)?;
                    self.expressao(valor)?;
                    self.armazenar_rax(temporario);
                    if largura == 2 {
                        self.armazenar_rdx(temporario + 8);
                    }
                    self.carregar_rax(base);
                    self.codigo.extend_from_slice(&[0x49, 0x89, 0xC3]); // r11 = agregado
                    self.carregar_rax(temporario);
                    if largura == 2 {
                        self.carregar_rdx(temporario + 8);
                    }
                    self.armazenar_campo_r11(deslocamento, tamanho)?;
                } else {
                    if tipo_decimal(tipo) {
                        return Err("Atribuição de campos decimais pequenos ainda não está disponível.".into());
                    }
                    self.expressao(valor)?;
                    if let Some(base) = self.buscar_local(raiz) {
                        match (deslocamento, tamanho) {
                            (0, 16) => {
                                self.armazenar_rax(base);
                                self.armazenar_rdx(base - 8);
                            }
                            (0, _) => self.armazenar_rax(base),
                            (8, _) => self.armazenar_rax(base - 8),
                            _ => return Err("Campo não está em um registrador ABI.".into()),
                        }
                    } else if self.globais.contains_key(raiz) {
                        match (deslocamento, tamanho) {
                            (0, 16) => self.armazenar_global(raiz),
                            (0, _) => self.armazenar_global_parte(raiz, 0),
                            (8, _) => self.armazenar_global_parte(raiz, 8),
                            _ => return Err("Campo não está em um registrador ABI.".into()),
                        }
                    }
                }
            }
            ExpressaoIr::Atribuicao { nome, valor, .. } => {
                let tipo_local = self.buscar_tipo_local(nome).cloned();
                if let Some(tipo) = tipo_local.as_ref() {
                    if let Some((tamanho, _)) = self.layout_agregado_grande(tipo) {
                        let destino = self.buscar_local(nome)
                            .ok_or_else(|| format!("Variável '{nome}' não encontrada no backend."))?;
                        let origem = self.reservar_agregado(8, 8)?;
                        self.expressao(valor)?;
                        self.armazenar_rax(origem);
                        self.carregar_rax(destino);
                        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7]); // rdi = destino
                        self.carregar_rax(origem);
                        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = origem
                        self.codigo.extend_from_slice(&[0x48, 0xB9]);
                        self.codigo.extend_from_slice(&tamanho.to_le_bytes());
                        self.codigo.extend_from_slice(&[0xF3, 0xA4]); // cópia por valor
                        self.carregar_rax(destino);
                        self.mov_rdx(0);
                        return Ok(());
                    }
                }
                self.expressao(valor)?;
                if let Some(deslocamento) = self.buscar_local(nome) {
                    self.armazenar_rax(deslocamento);
                    self.armazenar_rdx(deslocamento - 8);
                } else if self.globais.contains_key(nome) {
                    self.armazenar_global(nome);
                } else {
                    return Err(format!("Variável '{nome}' não encontrada no backend."));
                }
            }
            ExpressaoIr::Chamada { nome, argumentos } => {
                if nome == "exibir" {
                    if argumentos.iter().any(|argumento| {
                        self.tipo_expressao_atual(argumento).as_ref() != Some(&Tipo::Texto)
                    }) {
                        return Err("O backend nativo implementa 'exibir' somente para argumentos do tipo texto nesta etapa.".into());
                    }
                    self.emitir_exibir_textos(argumentos)?;
                    return Ok(());
                }
                if nome == "comprimento" {
                    let [argumento] = argumentos.as_slice() else {
                        return Err("'comprimento' exige exatamente um argumento no backend nativo.".into());
                    };
                    if self.tipo_expressao_atual(argumento).as_ref() != Some(&Tipo::Texto) {
                        return Err("O backend nativo implementa 'comprimento' somente para texto nesta etapa.".into());
                    }
                    self.emitir_comprimento_texto(argumento)?;
                    return Ok(());
                }
                if nome == "contem" {
                    if argumentos.len() != 2
                        || argumentos.iter().any(|argumento| {
                            self.tipo_expressao_atual(argumento).as_ref() != Some(&Tipo::Texto)
                        })
                    {
                        return Err("O backend nativo exige dois textos em 'contem'.".into());
                    }
                    self.emitir_contem_texto(&argumentos[0], &argumentos[1])?;
                    return Ok(());
                }
                if nome == "comeca_com" || nome == "termina_com" {
                    if argumentos.len() != 2
                        || argumentos.iter().any(|argumento| {
                            self.tipo_expressao_atual(argumento).as_ref() != Some(&Tipo::Texto)
                        })
                    {
                        return Err(format!("O backend nativo exige dois textos em '{nome}'."));
                    }
                    self.emitir_comparacao_prefixo_sufixo_texto(
                        &argumentos[0],
                        &argumentos[1],
                        nome == "termina_com",
                    )?;
                    return Ok(());
                }
                if nome == "dormir" || nome == "aguardar" {
                    let [argumento] = argumentos.as_slice() else {
                        return Err(format!("'{nome}' exige exatamente um argumento no backend nativo."));
                    };
                    if self.tipo_expressao_atual(argumento).as_ref() != Some(&Tipo::Inteiro) {
                        return Err(format!("O backend nativo exige inteiro não negativo em '{nome}'."));
                    }
                    self.emitir_dormir_milisegundos(argumento)?;
                    return Ok(());
                }
                if nome == "tempo" {
                    if !argumentos.is_empty() {
                        return Err("'tempo' não aceita argumentos no backend nativo.".into());
                    }
                    self.emitir_tempo_unix_milisegundos()?;
                    return Ok(());
                }
                if nome == "ler_texto" || nome == "receber" || nome == "perguntar" {
                    if argumentos.len() > 1 {
                        return Err(format!("'{nome}' aceita no máximo um texto de prompt no backend nativo."));
                    }
                    if let Some(prompt) = argumentos.first() {
                        if self.tipo_expressao_atual(prompt).as_ref() != Some(&Tipo::Texto) {
                            return Err(format!("O prompt de '{nome}' precisa ser texto."));
                        }
                    }
                    self.emitir_ler_texto(argumentos)?;
                    return Ok(());
                }
                if nome == "json_valido" || nome == "json_canonico" {
                    let [argumento] = argumentos.as_slice() else {
                        return Err(format!("'{nome}' exige exatamente um argumento de texto."));
                    };
                    if self.tipo_expressao_atual(argumento).as_ref() != Some(&Tipo::Texto) {
                        return Err(format!("'{nome}' exige um argumento do tipo texto."));
                    }
                    let Some(texto) = texto_literal_ir(argumento) else {
                        return Err(format!(
                            "'{nome}' no backend ELF exige texto literal nesta etapa; valores dinâmicos requerem runtime nativo."
                        ));
                    };
                    if nome == "json_valido" {
                        self.mov_rax(if analisar_json(texto).is_ok() { 1 } else { 0 });
                    } else {
                        let valor = analisar_json(texto)
                            .map_err(|erro| format!("JSON inválido em 'json_canonico': {erro}"))?;
                        let canonico = serializar_json(&valor)
                            .map_err(|erro| format!("Falha ao canonicalizar JSON: {erro}"))?;
                        self.emitir_texto_estatico(&canonico)?;
                    }
                    return Ok(());
                }
                let retorno_grande = self.retornos_funcoes.get(nome)
                    .and_then(|tipo| self.layout_agregado_grande(tipo));
                let (tamanho_pilha, preenchimento) = if let Some((tamanho, alinhamento)) = retorno_grande {
                    let destino = self.reservar_agregado(tamanho, alinhamento)?;
                    let destino_slot = self.reservar_agregado(8, 8)?;
                    self.emitir_endereco_rbp_rax(destino);
                    self.armazenar_rax(destino_slot);
                    let preparado = self.preparar_argumentos_desde(argumentos, 1)?;
                    self.carregar_rax(destino_slot);
                    self.mover_rax_registrador(7);
                    preparado
                } else {
                    self.preparar_argumentos(argumentos)?
                };
                self.codigo.push(0xE8);
                let posicao = self.codigo.len();
                self.codigo.extend_from_slice(&[0, 0, 0, 0]);
                self.chamadas.push((posicao, nome.clone()));
                self.limpar_pilha_chamada(tamanho_pilha, preenchimento)?;
            }
            ExpressaoIr::ChamadaQualificada { modulo, nome, argumentos } => {
                let nome_completo = format!("{modulo}::{nome}");
                let retorno_grande = self.retornos_funcoes.get(&nome_completo)
                    .and_then(|tipo| self.layout_agregado_grande(tipo));
                let (tamanho_pilha, preenchimento) = if let Some((tamanho, alinhamento)) = retorno_grande {
                    let destino = self.reservar_agregado(tamanho, alinhamento)?;
                    let destino_slot = self.reservar_agregado(8, 8)?;
                    self.emitir_endereco_rbp_rax(destino);
                    self.armazenar_rax(destino_slot);
                    let preparado = self.preparar_argumentos_desde(argumentos, 1)?;
                    self.carregar_rax(destino_slot);
                    self.mover_rax_registrador(7);
                    preparado
                } else {
                    self.preparar_argumentos(argumentos)?
                };
                self.codigo.push(0xE8);
                let posicao = self.codigo.len();
                self.codigo.extend_from_slice(&[0, 0, 0, 0]);
                self.chamadas.push((posicao, nome_completo));
                self.limpar_pilha_chamada(tamanho_pilha, preenchimento)?;
            }
        }
        Ok(())
    }

    fn emitir_texto_estatico(&mut self, texto: &str) -> Result<(), String> {
        let base_global = self.globais.len().checked_mul(16)
            .ok_or("Área de dados globais excedeu o limite do alvo.")?;
        let deslocamento = base_global.checked_add(self.dados_estaticos.len())
            .ok_or("Área de textos estáticos excedeu o limite do alvo.")?;
        self.dados_estaticos.extend_from_slice(texto.as_bytes());
        self.codigo.extend_from_slice(&[0x48, 0x8D, 0x05]);
        let posicao = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 4]);
        self.referencias_texto.push((posicao, deslocamento));
        self.mov_rdx(i64::try_from(texto.len())
            .map_err(|_| "Literal de texto excede o limite do alvo.")?);
        Ok(())
    }

    fn emitir_referencia_funcao(&mut self, nome: &str) {
        self.codigo.extend_from_slice(&[0x48, 0xB8]); // mov rax, imm64
        let posicao = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 8]);
        self.referencias_funcoes.push((posicao, nome.to_owned()));
    }

    fn normalizar_tipo(&mut self, tipo: &Tipo) -> Result<(), String> {
        let bits_assinados = match tipo {
            Tipo::Inteiro8 => Some(8),
            Tipo::Inteiro16 => Some(16),
            Tipo::Inteiro32 => Some(32),
            _ => None,
        };
        if let Some(bits) = bits_assinados {
            let deslocamento = 64 - bits;
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = valor original
            self.codigo.extend_from_slice(&[0x48, 0xC1, 0xE0, deslocamento]); // shl rax
            self.codigo.extend_from_slice(&[0x48, 0xC1, 0xF8, deslocamento]); // sar rax
            self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]); // cmp rax, rcx
            let valido = self.emitir_jcc(0x84);
            self.encerrar_com_erro(2);
            self.aplicar_salto(valido, self.codigo.len());
            return Ok(());
        }

        let bits_sem_sinal = match tipo {
            Tipo::InteiroU8 | Tipo::Byte => Some(8),
            Tipo::InteiroU16 => Some(16),
            Tipo::InteiroU32 => Some(32),
            _ => None,
        };
        if let Some(bits) = bits_sem_sinal {
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = valor original
            self.codigo.extend_from_slice(&[0x48, 0xC1, 0xE8, bits]); // shr rax
            self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
            let valido = self.emitir_jcc(0x84);
            self.encerrar_com_erro(2);
            self.aplicar_salto(valido, self.codigo.len());
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC8]); // rax = rcx
            return Ok(());
        }

        if *tipo == Tipo::Caractere {
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = escalar original
            self.mov_rax(0x10FFFF);
            self.codigo.extend_from_slice(&[0x48, 0x39, 0xC1]); // cmp rcx, rax
            let dentro_do_unicode = self.emitir_jcc(0x86); // JBE
            self.encerrar_com_erro(2);
            self.aplicar_salto(dentro_do_unicode, self.codigo.len());

            self.mov_rax(0xD800);
            self.codigo.extend_from_slice(&[0x48, 0x39, 0xC1]);
            let abaixo_dos_surrogates = self.emitir_jcc(0x82); // JB
            self.mov_rax(0xDFFF);
            self.codigo.extend_from_slice(&[0x48, 0x39, 0xC1]);
            let acima_dos_surrogates = self.emitir_jcc(0x87); // JA
            self.encerrar_com_erro(2);
            let fim = self.codigo.len();
            self.aplicar_salto(abaixo_dos_surrogates, fim);
            self.aplicar_salto(acima_dos_surrogates, fim);
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC8]); // rax = escalar original
            return Ok(());
        }

        match tipo {
            Tipo::Inteiro | Tipo::Inteiro64 | Tipo::InteiroU64 | Tipo::Inteiro128 | Tipo::InteiroU128 => Ok(()),
            Tipo::Logico => {
                self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0, 0x0F, 0x95, 0xC0, 0x48, 0x0F, 0xB6, 0xC0]);
                Ok(())
            }
            Tipo::Decimal32 => {
                self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x5A, 0xC0]); // cvtsd2ss xmm0, xmm0
                self.codigo.extend_from_slice(&[0xF3, 0x0F, 0x5A, 0xC0]); // cvtss2sd xmm0, xmm0
                self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                Ok(())
            }
            Tipo::Decimal | Tipo::Decimal64 => Ok(()),
            _ => Err(format!("Normalização de '{}' não é suportada pelo backend nativo.", tipo.nome())),
        }
    }

    fn converter_tipo_explicito(
        &mut self,
        destino: &Tipo,
        tipo_origem: &Tipo,
    ) -> Result<(), String> {
        let origem_inteira = tipo_inteiro_abi(tipo_origem);
        let destino_inteiro = tipo_inteiro_abi(destino);

        if let Some((128, destino_assinado)) = destino_inteiro {
            if let Some((bits_origem, origem_assinada)) = origem_inteira {
                if bits_origem == 128 {
                    if destino_assinado != origem_assinada {
                        self.codigo.extend_from_slice(&[0x48, 0x85, 0xD2]); // test rdx, rdx
                        let valido = self.emitir_jcc(0x89); // JNS
                        self.encerrar_com_erro(2);
                        self.aplicar_salto(valido, self.codigo.len());
                    }
                    return Ok(());
                }
                if origem_assinada && !destino_assinado {
                    self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
                    let valido = self.emitir_jcc(0x89); // JNS
                    self.encerrar_com_erro(2);
                    self.aplicar_salto(valido, self.codigo.len());
                }
                if origem_assinada && destino_assinado {
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC2]); // mov rdx, rax
                    self.codigo.extend_from_slice(&[0x48, 0xC1, 0xFA, 0x3F]); // sar rdx, 63
                } else {
                    self.mov_rdx(0);
                }
                return Ok(());
            }
        }

        if let Some((128, _)) = origem_inteira {
            if let Some((_, destino_assinado)) = destino_inteiro {
                if destino_assinado {
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = low
                    self.codigo.extend_from_slice(&[0x48, 0xC1, 0xF9, 0x3F]); // sar rcx, 63
                    self.codigo.extend_from_slice(&[0x48, 0x39, 0xCA]); // cmp rdx, rcx
                } else {
                    self.codigo.extend_from_slice(&[0x48, 0x85, 0xD2]); // test rdx, rdx
                }
                let valido = self.emitir_jcc(0x84);
                self.encerrar_com_erro(2);
                self.aplicar_salto(valido, self.codigo.len());
                return self.normalizar_tipo(destino);
            }
            if matches!(destino, Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64) {
                return self.converter_inteiro128_para_decimal(
                    origem_inteira.is_some_and(|(_, assinado)| assinado),
                    destino,
                );
            }
        }

        if let Some((64, destino_assinado)) = destino_inteiro {
            if let Some((_, origem_assinada)) = origem_inteira {
                if destino_assinado != origem_assinada {
                    self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]);
                    let valido = self.emitir_jcc(0x89);
                    self.encerrar_com_erro(2);
                    self.aplicar_salto(valido, self.codigo.len());
                }
                return Ok(());
            }
        }
        if destino_inteiro.is_some() {
            if origem_inteira.is_some() {
                return self.normalizar_tipo(destino);
            }
            if matches!(tipo_origem, Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64) {
                return self.converter_decimal_para_inteiro(destino);
            }
        }

        if matches!(destino, Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64) {
            if matches!(tipo_origem, Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64) {
                return self.normalizar_tipo(destino);
            }
            if let Some((_, assinado)) = origem_inteira {
                if assinado {
                    self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                    self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xC0]); // cvtsi2sd xmm0, rax
                } else {
                    // Conversão u64 -> f64 sem reinterpretar valores acima de i64::MAX.
                    self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1, 0x48, 0xD1, 0xE8, 0x83, 0xE1, 0x01]);
                    self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xC0]); // cvtsi2sd xmm0, rax
                    self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xC0]); // addsd xmm0, xmm0
                    self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xC9]); // cvtsi2sd xmm1, rcx
                    self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xC1]); // addsd xmm0, xmm1
                }
                self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                return self.normalizar_tipo(destino);
            }
        }
        Err(format!(
            "Conversão explícita de '{}' para '{}' não é suportada pelo backend nativo.",
            tipo_origem.nome(),
            destino.nome()
        ))
    }

    fn converter_inteiro128_para_decimal(
        &mut self,
        origem_assinada: bool,
        destino: &Tipo,
    ) -> Result<(), String> {
        if origem_assinada {
            self.codigo.extend_from_slice(&[0x41, 0xBB, 0, 0, 0, 0]); // r11d = sinal
            self.codigo.extend_from_slice(&[0x48, 0x85, 0xD2]); // test rdx, rdx
            let positivo = self.emitir_jcc(0x89); // jns
            self.codigo.extend_from_slice(&[0x41, 0xBB, 1, 0, 0, 0]);
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD8]); // neg rax
            self.codigo.extend_from_slice(&[0xF5]); // cmc
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD2]); // not rdx
            self.codigo.extend_from_slice(&[0x48, 0x83, 0xD2, 0x00]); // adc rdx, 0
            self.aplicar_salto(positivo, self.codigo.len());
        }

        // Converte cada metade unsigned64 sem interpretar o bit 63 como sinal.
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1, 0x48, 0xD1, 0xE8, 0x83, 0xE1, 0x01]);
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xC0]); // cvtsi2sd xmm0, rax
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xC0]); // addsd xmm0, xmm0
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
        self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xC9]); // cvtsi2sd xmm1, rcx
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xC1]); // addsd xmm0, xmm1

        self.codigo.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1, 0x48, 0xD1, 0xE8, 0x83, 0xE1, 0x01]);
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xD0]); // movq xmm2, rax
        self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xD0]); // cvtsi2sd xmm2, rax
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xD2]); // addsd xmm2, xmm2
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xD9]); // movq xmm3, rcx
        self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xD9]); // cvtsi2sd xmm3, rcx
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xD3]); // addsd xmm2, xmm3
        self.mov_rax(0x43F0_0000_0000_0000u64 as i64); // 2^64
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xD8]); // movq xmm3, rax
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x59, 0xD3]); // mulsd xmm2, xmm3
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xC2]); // addsd xmm0, xmm2

        if origem_assinada {
            self.codigo.extend_from_slice(&[0x45, 0x85, 0xDB]); // test r11d, r11d
            let positivo = self.emitir_jcc(0x84);
            self.mov_rax(i64::MIN);
            self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]); // movq xmm1, rax
            self.codigo.extend_from_slice(&[0x66, 0x0F, 0x57, 0xC1]); // xorpd xmm0, xmm1
            self.aplicar_salto(positivo, self.codigo.len());
        }

        if *destino == Tipo::Decimal32 {
            self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x5A, 0xC0]); // cvtsd2ss
            self.codigo.extend_from_slice(&[0xF3, 0x0F, 0x5A, 0xC0]); // cvtss2sd
        }
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
        Ok(())
    }

    fn converter_decimal_para_inteiro(&mut self, destino: &Tipo) -> Result<(), String> {
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.codigo.extend_from_slice(&[0x66, 0x0F, 0x2E, 0xC0]); // ucomisd xmm0, xmm0
        let finito = self.emitir_jcc(0x8B); // JNP: rejeita NaN
        self.encerrar_com_erro(2);
        self.aplicar_salto(finito, self.codigo.len());

        let Some((largura, assinado)) = tipo_inteiro_abi(destino) else {
            return Err(format!("Tipo '{}' não possui conversão inteira nativa.", destino.nome()));
        };
        if largura == 128 {
            return self.converter_decimal_para_inteiro128(destino);
        }
        if assinado {
            // -2^63 <= x < 2^63; o limite inferior é inclusivo e o superior exclusivo.
            self.mov_rax(0xC3E0_0000_0000_0000u64 as i64);
            self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]); // movq xmm1, rax
            self.codigo.extend_from_slice(&[0x66, 0x0F, 0x2E, 0xC1]); // ucomisd xmm0, xmm1
            let acima_minimo = self.emitir_jcc(0x83); // JAE
            self.encerrar_com_erro(2);
            self.aplicar_salto(acima_minimo, self.codigo.len());

            self.mov_rax(0x43E0_0000_0000_0000u64 as i64);
            self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]);
            self.codigo.extend_from_slice(&[0x66, 0x0F, 0x2E, 0xC1]);
            let abaixo_maximo = self.emitir_jcc(0x82); // JB
            self.encerrar_com_erro(2);
            self.aplicar_salto(abaixo_maximo, self.codigo.len());
            self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
        } else {
            // 0 <= x < 2^64.
            self.mov_rax(0);
            self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]);
            self.codigo.extend_from_slice(&[0x66, 0x0F, 0x2E, 0xC1]);
            let nao_negativo = self.emitir_jcc(0x83); // JAE
            self.encerrar_com_erro(2);
            self.aplicar_salto(nao_negativo, self.codigo.len());

            self.mov_rax(0x43F0_0000_0000_0000u64 as i64);
            self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]);
            self.codigo.extend_from_slice(&[0x66, 0x0F, 0x2E, 0xC1]);
            let abaixo_limite = self.emitir_jcc(0x82); // JB
            self.encerrar_com_erro(2);
            self.aplicar_salto(abaixo_limite, self.codigo.len());

            self.mov_rax(0x43E0_0000_0000_0000u64 as i64);
            self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]);
            self.codigo.extend_from_slice(&[0x66, 0x0F, 0x2E, 0xC1]);
            let abaixo_metade = self.emitir_jcc(0x82);
            self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x5C, 0xC1]); // subsd xmm0, xmm1
            self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
            self.codigo.extend_from_slice(&[0x48, 0x0F, 0xBA, 0xE8, 0x3F]); // bts rax, 63
            let convertido = self.emitir_jmp();
            self.aplicar_salto(abaixo_metade, self.codigo.len());
            self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
            self.aplicar_salto(convertido, self.codigo.len());
        }
        self.normalizar_tipo(destino)
    }

    fn converter_decimal_para_inteiro128(&mut self, destino: &Tipo) -> Result<(), String> {
        let assinado = matches!(destino, Tipo::Inteiro128);
        self.codigo.extend_from_slice(&[0x41, 0xBB, 0, 0, 0, 0]); // r11d = sinal
        if assinado {
            self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax (sinal do double)
            let positivo = self.emitir_jcc(0x89);
            self.codigo.extend_from_slice(&[0x41, 0xBB, 1, 0, 0, 0]);
            self.aplicar_salto(positivo, self.codigo.len());
        } else {
            self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]);
            let nao_negativo = self.emitir_jcc(0x89);
            self.encerrar_com_erro(2);
            self.aplicar_salto(nao_negativo, self.codigo.len());
        }

        // Trabalha com a magnitude e preserva a entrada original em xmm2.
        self.mov_rax(i64::MAX);
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]); // movq xmm1, rax
        self.codigo.extend_from_slice(&[0x66, 0x0F, 0x54, 0xC1]); // andpd xmm0, xmm1
        self.codigo.extend_from_slice(&[0x66, 0x0F, 0x28, 0xD0]); // movapd xmm2, xmm0

        // high = trunc(magnitude / 2^64), com verificação unsigned64.
        self.mov_rax(0x43F0_0000_0000_0000u64 as i64);
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]); // movq xmm1, rax
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x5E, 0xC1]); // divsd xmm0, xmm1
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
        self.converter_decimal_para_inteiro(&Tipo::InteiroU64)?;
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC2]); // high -> rdx

        // Converte high para double unsigned64 e subtrai high * 2^64.
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1, 0x48, 0xD1, 0xE8, 0x83, 0xE1, 0x01]);
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xD8]); // movq xmm3, rax
        self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xD8]); // cvtsi2sd xmm3, rax
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xDB]); // addsd xmm3, xmm3
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xE1]); // movq xmm4, rcx
        self.codigo.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xE1]); // cvtsi2sd xmm4, rcx
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xDC]); // addsd xmm3, xmm4
        self.mov_rax(0x43F0_0000_0000_0000u64 as i64);
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]); // movq xmm1, rax
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x59, 0xD9]); // mulsd xmm3, xmm1
        self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x5C, 0xD3]); // subsd xmm2, xmm3
        self.codigo.extend_from_slice(&[0x66, 0x0F, 0x28, 0xC2]); // movapd xmm0, xmm2
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
        self.converter_decimal_para_inteiro(&Tipo::InteiroU64)?;

        if assinado {
            self.codigo.extend_from_slice(&[0x41, 0xF6, 0xC3, 0x01]); // test r11b, 1
            let positivo = self.emitir_jcc(0x84);

            // Negativo: magnitude <= 2^127; igualdade exige low == 0.
            self.codigo.extend_from_slice(&[0x49, 0x89, 0xC1]); // save low in r9
            self.mov_rax(i64::MIN);
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = 2^63
            self.codigo.extend_from_slice(&[0x4C, 0x89, 0xC8]); // restore low in rax
            self.codigo.extend_from_slice(&[0x48, 0x39, 0xCA]); // cmp high, 2^63
            let acima_limite_negativo = self.emitir_jcc(0x87); // ja
            let abaixo_limite_negativo = self.emitir_jcc(0x82); // jb
            self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test low, low
            let baixo_nao_zero = self.emitir_jcc(0x85); // jnz
            let limite_negativo_valido = self.emitir_jmp();

            let negativo_valido = self.codigo.len();
            self.aplicar_salto(abaixo_limite_negativo, negativo_valido);
            self.aplicar_salto(limite_negativo_valido, negativo_valido);
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD8, 0xF5, 0x48, 0xF7, 0xD2, 0x48, 0x83, 0xD2, 0x00]);
            let fim_negativo = self.emitir_jmp();

            let positivo_inicio = self.codigo.len();
            self.aplicar_salto(positivo, positivo_inicio);
            self.codigo.extend_from_slice(&[0x49, 0x89, 0xC1]); // save low in r9
            self.mov_rax(i64::MIN);
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = 2^63
            self.codigo.extend_from_slice(&[0x4C, 0x89, 0xC8]); // restore low in rax
            self.codigo.extend_from_slice(&[0x48, 0x39, 0xCA]); // cmp high, 2^63
            let positivo_fora_faixa = self.emitir_jcc(0x83); // jae
            let fim_positivo = self.emitir_jmp();

            let erro = self.codigo.len();
            self.aplicar_salto(acima_limite_negativo, erro);
            self.aplicar_salto(baixo_nao_zero, erro);
            self.aplicar_salto(positivo_fora_faixa, erro);
            self.encerrar_com_erro(2);
            let fim = self.codigo.len();
            self.aplicar_salto(fim_negativo, fim);
            self.aplicar_salto(fim_positivo, fim);
        } else {
            self.normalizar_tipo(destino)?;
            return Ok(());
        }
        self.normalizar_tipo(destino)
    }

    fn carregar_decimais_sse(&mut self, tipo: &Tipo) {
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
        if *tipo == Tipo::Decimal32 {
            self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x5A, 0xC0]); // cvtsd2ss xmm0, xmm0
            self.codigo.extend_from_slice(&[0xF2, 0x0F, 0x5A, 0xC9]); // cvtsd2ss xmm1, xmm1
        }
    }

    fn operar_decimal(&mut self, tipo: &Tipo, operador: &OperadorIr) -> Result<(), String> {
        self.carregar_decimais_sse(tipo);
        let (prefixo, opcode) = match (tipo, operador) {
            (Tipo::Decimal32, OperadorIr::Mais) => (0xF3, 0x58),
            (Tipo::Decimal32, OperadorIr::Menos) => (0xF3, 0x5C),
            (Tipo::Decimal32, OperadorIr::Vezes) => (0xF3, 0x59),
            (Tipo::Decimal32, OperadorIr::Divide) => (0xF3, 0x5E),
            (_, OperadorIr::Mais) => (0xF2, 0x58),
            (_, OperadorIr::Menos) => (0xF2, 0x5C),
            (_, OperadorIr::Vezes) => (0xF2, 0x59),
            (_, OperadorIr::Divide) => (0xF2, 0x5E),
            _ => return Err("Operador decimal não suportado.".into()),
        };
        self.codigo.extend_from_slice(&[prefixo, 0x0F, opcode, 0xC1]);
        if *tipo == Tipo::Decimal32 {
            self.codigo.extend_from_slice(&[0xF3, 0x0F, 0x5A, 0xC0]); // cvtss2sd xmm0, xmm0
        }
        self.codigo.extend_from_slice(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
        Ok(())
    }

    fn comparar_decimal(&mut self, tipo: &Tipo, operador: &OperadorIr) {
        self.carregar_decimais_sse(tipo);
        if *tipo == Tipo::Decimal32 {
            self.codigo.extend_from_slice(&[0x0F, 0x2E, 0xC1]); // ucomiss xmm0, xmm1
        } else {
            self.codigo.extend_from_slice(&[0x66, 0x0F, 0x2E, 0xC1]); // ucomisd xmm0, xmm1
        }
        match operador {
            OperadorIr::Igual => {
                self.codigo.extend_from_slice(&[0x0F, 0x94, 0xC0, 0x0F, 0x9B, 0xC1, 0x20, 0xC8]);
            }
            OperadorIr::Diferente => {
                self.codigo.extend_from_slice(&[0x0F, 0x95, 0xC0, 0x0F, 0x9A, 0xC1, 0x08, 0xC8]);
            }
            OperadorIr::Menor => {
                self.codigo.extend_from_slice(&[0x0F, 0x92, 0xC0, 0x0F, 0x9B, 0xC1, 0x20, 0xC8]);
            }
            OperadorIr::MenorIgual => {
                self.codigo.extend_from_slice(&[0x0F, 0x96, 0xC0, 0x0F, 0x9B, 0xC1, 0x20, 0xC8]);
            }
            OperadorIr::Maior => self.codigo.extend_from_slice(&[0x0F, 0x97, 0xC0]),
            OperadorIr::MaiorIgual => self.codigo.extend_from_slice(&[0x0F, 0x93, 0xC0]),
            _ => unreachable!(),
        }
        self.codigo.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
    }

    fn declarar(&mut self, nome: &str) -> Result<i32, String> {
        self.declarar_tipado(nome, &Tipo::Vazio)
    }

    fn declarar_tipado(&mut self, nome: &str, tipo: &Tipo) -> Result<i32, String> {
        if self.escopos.last().is_some_and(|escopo| escopo.contains_key(nome)) {
            return Err(format!("Identificador '{nome}' declarado mais de uma vez no mesmo escopo."));
        }
        if self.proximo_slot >= LIMITE_VARIAVEIS {
            return Err("A função excedeu o limite inicial de 512 posições locais.".into());
        }
        self.proximo_slot += 1;
        let deslocamento = -((self.proximo_slot * 16 - 8) as i32);
        self.escopos.last_mut().expect("escopo ativo").insert(nome.to_owned(), deslocamento);
        self.tipos_escopos.last_mut().expect("escopo tipado ativo").insert(nome.to_owned(), tipo.clone());
        Ok(deslocamento)
    }

    fn alvo_estrutura_grande(&self, alvo: &ExpressaoIr) -> bool {
        let Some(Tipo::Estrutura(nome)) = tipo_expressao_ir(alvo) else {
            return false;
        };
        self.layouts_tipos.get(nome).is_some_and(|layout| layout.tamanho > 16)
    }

    fn carregar_membro_agregado(
        &mut self,
        alvo: &ExpressaoIr,
        campo: &str,
        tipo: &Tipo,
    ) -> Result<(), String> {
        let Some(Tipo::Estrutura(nome)) = tipo_expressao_ir(alvo) else {
            return Err("Acesso a agregado grande exige estrutura tipada.".into());
        };
        let (deslocamento, tamanho) = self.layouts_tipos.get(nome)
            .and_then(|layout| layout.campos.get(campo).copied())
            .ok_or_else(|| format!("Campo '{campo}' não encontrado em '{nome}'."))?;
        let deslocamento = i32::try_from(deslocamento)
            .map_err(|_| "Campo agregado excede o deslocamento x86-64.")?;
        self.expressao(alvo)?;
        self.codigo.extend_from_slice(&[0x49, 0x89, 0xC3]); // r11 = endereço do agregado

        if tamanho > 16 {
            self.codigo.extend_from_slice(&[0x49, 0x8D, 0x83]);
            self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
            self.mov_rdx(0);
            return Ok(());
        }
        if tamanho == 16 {
            self.codigo.extend_from_slice(&[0x49, 0x8B, 0x83]);
            self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
            self.codigo.extend_from_slice(&[0x49, 0x8B, 0x93]);
            self.codigo.extend_from_slice(&(deslocamento + 8).to_le_bytes());
            return Ok(());
        }
        if (9..16).contains(&tamanho) {
            self.codigo.extend_from_slice(&[0x49, 0x8B, 0x83]);
            self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
            self.codigo.extend_from_slice(&[0x49, 0x8B, 0x93]);
            self.codigo.extend_from_slice(&(deslocamento + 8).to_le_bytes());
            let bytes_superiores = tamanho - 8;
            let mascara = (1u64 << (bytes_superiores * 8)) - 1;
            self.mov_rax(mascara as i64);
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1, 0x48, 0x21, 0xCA]); // rdx &= mask
            self.codigo.extend_from_slice(&[0x49, 0x8B, 0x83]);
            self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
            return Ok(());
        }

        let bits = tipo_inteiro_abi(tipo);
        match bits {
            Some((8, true)) => self.codigo.extend_from_slice(&[0x49, 0x0F, 0xBE, 0x83]),
            Some((8, false)) | None if tamanho == 1 => {
                self.codigo.extend_from_slice(&[0x49, 0x0F, 0xB6, 0x83])
            }
            Some((16, true)) => self.codigo.extend_from_slice(&[0x49, 0x0F, 0xBF, 0x83]),
            Some((16, false)) | None if tamanho == 2 => {
                self.codigo.extend_from_slice(&[0x49, 0x0F, 0xB7, 0x83])
            }
            Some((32, true)) => self.codigo.extend_from_slice(&[0x49, 0x63, 0x83]),
            Some((32, false)) | None if tamanho == 4 => {
                self.codigo.extend_from_slice(&[0x41, 0x8B, 0x83])
            }
            Some((64, _)) | None if tamanho == 8 => {
                self.codigo.extend_from_slice(&[0x49, 0x8B, 0x83])
            }
            Some((128, _)) => {
                self.codigo.extend_from_slice(&[0x49, 0x8B, 0x83]);
                self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
                self.codigo.extend_from_slice(&[0x49, 0x8B, 0x93]);
                self.codigo.extend_from_slice(&(deslocamento + 8).to_le_bytes());
                return Ok(());
            }
            _ => return Err(format!("Campo '{campo}' possui layout incompatível com o tipo '{}'.", tipo.nome())),
        }
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
        if *tipo == Tipo::Decimal32 {
            self.codigo.extend_from_slice(&[0x66, 0x0F, 0x6E, 0xC0, 0xF3, 0x0F, 0x5A, 0xC0, 0x66, 0x48, 0x0F, 0x7E, 0xC0]);
        }
        self.mov_rdx(0);
        Ok(())
    }

    fn emitir_endereco_rbp_rax(&mut self, deslocamento: i32) {
        self.codigo.extend_from_slice(&[0x48, 0x8D, 0x85]);
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
    }

    fn armazenar_registro_r11(&mut self, deslocamento: i32, registro: u8, largura: u8) {
        if largura == 2 {
            self.codigo.push(0x66);
        }
        self.codigo.push(if largura == 8 { 0x49 } else { 0x41 });
        self.codigo.push(if largura == 1 { 0x88 } else { 0x89 });
        self.codigo.push(0x80 | ((registro & 7) << 3) | 3);
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
    }

    fn armazenar_fragmentos_r11(&mut self, mut deslocamento: i32, mut tamanho: u64, registro: u8) {
        if tamanho >= 8 {
            self.armazenar_registro_r11(deslocamento, registro, 8);
            return;
        }
        if tamanho >= 4 {
            self.armazenar_registro_r11(deslocamento, registro, 4);
            deslocamento += 4;
            tamanho -= 4;
            self.codigo.extend_from_slice(if registro == 0 {
                &[0x48, 0xC1, 0xE8, 32]
            } else {
                &[0x48, 0xC1, 0xEA, 32]
            });
        }
        if tamanho >= 2 {
            self.armazenar_registro_r11(deslocamento, registro, 2);
            deslocamento += 2;
            tamanho -= 2;
            self.codigo.extend_from_slice(if registro == 0 {
                &[0x48, 0xC1, 0xE8, 16]
            } else {
                &[0x48, 0xC1, 0xEA, 16]
            });
        }
        if tamanho == 1 {
            self.armazenar_registro_r11(deslocamento, registro, 1);
        }
    }

    fn armazenar_campo_agregado(
        &mut self,
        base: i32,
        deslocamento: u64,
        tamanho: u64,
    ) -> Result<(), String> {
        self.codigo.extend_from_slice(&[0x4C, 0x8D, 0x9D]);
        self.codigo.extend_from_slice(&base.to_le_bytes()); // r11 = início do agregado
        self.armazenar_campo_r11(deslocamento, tamanho)
    }

    fn armazenar_campo_r11(&mut self, deslocamento: u64, tamanho: u64) -> Result<(), String> {
        let deslocamento = i32::try_from(deslocamento)
            .map_err(|_| "Campo agregado excede o deslocamento x86-64.")?;
        if tamanho > 16 {
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = origem
            self.codigo.extend_from_slice(&[0x49, 0x8D, 0xBB]);
            self.codigo.extend_from_slice(&deslocamento.to_le_bytes()); // rdi = destino
            self.codigo.extend_from_slice(&[0x48, 0xB9]);
            self.codigo.extend_from_slice(&tamanho.to_le_bytes());
            self.codigo.extend_from_slice(&[0xF3, 0xA4]); // rep movsb
        } else if tamanho <= 8 {
            self.armazenar_fragmentos_r11(deslocamento, tamanho, 0);
        } else {
            self.armazenar_registro_r11(deslocamento, 0, 8);
            let restante = tamanho - 8;
            self.armazenar_fragmentos_r11(deslocamento + 8, restante, 2);
        }
        Ok(())
    }

    fn reservar_agregado(&mut self, tamanho: u64, alinhamento: u64) -> Result<i32, String> {
        if tamanho == 0 || alinhamento == 0 || !alinhamento.is_power_of_two() || alinhamento > 16 {
            return Err("Layout de agregado incompatível com o quadro nativo.".into());
        }
        let topo = self.proximo_agregado.checked_add(tamanho)
            .ok_or("Área de agregados excedeu o limite do quadro.")?;
        let fim = Self::alinhar_layout(topo, alinhamento)
            .ok_or("Alinhamento da área de agregados excedeu o limite.")?;
        if fim > u64::from(TAMANHO_QUADRO - 16) {
            return Err("A função excedeu a área reservada para agregados no quadro nativo.".into());
        }
        let deslocamento = i32::try_from(fim)
            .ok()
            .and_then(i32::checked_neg)
            .ok_or("Deslocamento do agregado excedeu o endereçamento x86-64.")?;
        self.proximo_agregado = fim;
        Ok(deslocamento)
    }

    fn tipo_expressao_atual(&self, expressao: &ExpressaoIr) -> Option<Tipo> {
        match expressao {
            ExpressaoIr::Normalizar { tipo, .. }
            | ExpressaoIr::Conversao { tipo, .. }
            | ExpressaoIr::ReferenciaFuncao { tipo, .. }
            | ExpressaoIr::ReferenciaFuncaoQualificada { tipo, .. }
            | ExpressaoIr::Atribuicao { tipo, .. }
            | ExpressaoIr::AtribuicaoCampo { tipo, .. } => Some(tipo.clone()),
            ExpressaoIr::Constante(ValorIr::Inteiro(_)) => Some(Tipo::Inteiro),
            ExpressaoIr::Constante(ValorIr::InteiroSemSinal(_)) => Some(Tipo::InteiroU128),
            ExpressaoIr::Constante(ValorIr::Decimal(_)) => Some(Tipo::Decimal64),
            ExpressaoIr::Constante(ValorIr::Logico(_)) => Some(Tipo::Logico),
            ExpressaoIr::Constante(ValorIr::Texto(_)) => Some(Tipo::Texto),
            ExpressaoIr::Constante(ValorIr::Caractere(_)) => Some(Tipo::Caractere),
            ExpressaoIr::Constante(ValorIr::Estrutura { nome, .. }) => {
                Some(Tipo::Estrutura(nome.clone()))
            }
            ExpressaoIr::Constante(ValorIr::Intervalo(_, _)) => {
                Some(Tipo::Intervalo(Box::new(Tipo::Inteiro)))
            }
            ExpressaoIr::Constante(ValorIr::IntervaloSemSinal(_, _)) => {
                Some(Tipo::Intervalo(Box::new(Tipo::InteiroU128)))
            }
            ExpressaoIr::Constante(ValorIr::IntervaloDecimal(_, _)) => {
                Some(Tipo::Intervalo(Box::new(Tipo::Decimal64)))
            }
            ExpressaoIr::Constante(ValorIr::Vazio) => Some(Tipo::Vazio),
            ExpressaoIr::Variavel(nome) => self
                .buscar_tipo_local(nome)
                .cloned()
                .or_else(|| self.tipos_globais.get(nome).cloned()),
            ExpressaoIr::Chamada { nome, .. } if nome == "json_canonico" => {
                Some(Tipo::Texto)
            }
            ExpressaoIr::Chamada { nome, .. } if nome == "json_valido" => {
                Some(Tipo::Logico)
            }
            ExpressaoIr::Chamada { nome, .. } => self.retornos_funcoes.get(nome).cloned(),
            ExpressaoIr::ChamadaQualificada { modulo, nome, .. } => {
                self.retornos_funcoes.get(&format!("{modulo}::{nome}")).cloned()
            }
            ExpressaoIr::ChamadaIndireta { alvo, .. } => {
                match self.tipo_expressao_atual(alvo)? {
                    Tipo::Funcao(_, retorno) => Some(*retorno),
                    _ => None,
                }
            }
            ExpressaoIr::Unaria {
                operador: OperadorIr::Nao | OperadorIr::Negacao,
                ..
            } => Some(Tipo::Logico),
            ExpressaoIr::Unaria { valor, .. } => self.tipo_expressao_atual(valor),
            ExpressaoIr::Binaria {
                operador:
                    OperadorIr::Igual
                    | OperadorIr::Diferente
                    | OperadorIr::Menor
                    | OperadorIr::MenorIgual
                    | OperadorIr::Maior
                    | OperadorIr::MaiorIgual
                    | OperadorIr::E
                    | OperadorIr::Ou,
                ..
            } => Some(Tipo::Logico),
            ExpressaoIr::Binaria {
                esquerda,
                operador: OperadorIr::Mais,
                direita,
            } if self.tipo_expressao_atual(esquerda).as_ref() == Some(&Tipo::Texto)
                && self.tipo_expressao_atual(direita).as_ref() == Some(&Tipo::Texto) =>
            {
                Some(Tipo::Texto)
            }
            ExpressaoIr::Binaria {
                esquerda,
                operador:
                    OperadorIr::Mais
                    | OperadorIr::Menos
                    | OperadorIr::Vezes
                    | OperadorIr::Divide
                    | OperadorIr::Resto
                    | OperadorIr::Potencia,
                ..
            } => self.tipo_expressao_atual(esquerda),
            ExpressaoIr::Binaria {
                esquerda,
                operador: OperadorIr::Intervalo,
                ..
            } => self
                .tipo_expressao_atual(esquerda)
                .map(|tipo| Tipo::Intervalo(Box::new(tipo))),
            ExpressaoIr::Lista(itens) => itens
                .first()
                .and_then(|item| self.tipo_expressao_atual(item))
                .map(|tipo| Tipo::Lista(Box::new(tipo))),
            ExpressaoIr::Tupla(itens) => itens
                .iter()
                .map(|item| self.tipo_expressao_atual(item))
                .collect::<Option<Vec<_>>>()
                .map(Tipo::Tupla),
            ExpressaoIr::ConstrucaoEstrutura { nome, .. } => {
                Some(Tipo::Estrutura(nome.clone()))
            }
            ExpressaoIr::ConstrucaoEnumeracao { nome, .. } => {
                Some(Tipo::Enumeracao(nome.clone()))
            }
            ExpressaoIr::Indexacao { alvo, .. } => match self.tipo_expressao_atual(alvo)? {
                Tipo::Lista(tipo) => Some(*tipo),
                Tipo::Texto => Some(Tipo::Caractere),
                _ => None,
            },
            _ => None,
        }
    }

    fn buscar_tipo_local(&self, nome: &str) -> Option<&Tipo> {
        self.tipos_escopos.iter().rev().find_map(|escopo| escopo.get(nome))
    }

    fn buscar_local(&self, nome: &str) -> Option<i32> {
        self.escopos.iter().rev().find_map(|escopo| escopo.get(nome).copied())
    }

    fn carregar_global(&mut self, nome: &str) {
        self.codigo.extend_from_slice(&[0x48, 0x8B, 0x05]); // mov rax, [rip + disp32]
        let posicao = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 4]);
        self.referencias_globais.push((posicao, nome.to_owned(), 0));
        self.codigo.extend_from_slice(&[0x48, 0x8B, 0x15]); // mov rdx, [rip + disp32]
        let posicao_alto = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 4]);
        self.referencias_globais.push((posicao_alto, nome.to_owned(), 8));
    }

    fn armazenar_global_parte(&mut self, nome: &str, parte: i32) {
        self.codigo.extend_from_slice(&[0x48, 0x89, 0x05]); // mov [rip + disp32], rax
        let posicao = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 4]);
        self.referencias_globais.push((posicao, nome.to_owned(), parte));
    }

    fn armazenar_global(&mut self, nome: &str) {
        self.codigo.extend_from_slice(&[0x48, 0x89, 0x05]); // mov [rip + disp32], rax
        let posicao = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 4]);
        self.referencias_globais.push((posicao, nome.to_owned(), 0));
        self.codigo.extend_from_slice(&[0x48, 0x89, 0x15]); // mov [rip + disp32], rdx
        let posicao_alto = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 4]);
        self.referencias_globais.push((posicao_alto, nome.to_owned(), 8));
    }

    fn verificar_carry(&mut self) {
        let sem_carry = self.emitir_jcc(0x83); // JNC
        self.encerrar_com_erro(2);
        self.aplicar_salto(sem_carry, self.codigo.len());
    }

    fn verificar_overflow(&mut self) {
        // A semântica inteira é checked: overflow encerra o processo com código 2.
        let sem_overflow = self.emitir_jcc(0x81); // JNO
        self.encerrar_com_erro(2);
        self.aplicar_salto(sem_overflow, self.codigo.len());
    }

    fn encerrar_com_erro(&mut self, codigo: u8) {
        // Linux x86-64: exit(codigo), syscall 60.
        self.codigo.extend_from_slice(&[0xBF, codigo, 0, 0, 0, 0xB8, 0x3C, 0, 0, 0, 0x0F, 0x05]);
    }

    fn normalizar_logico(&mut self) {
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0, 0x0F, 0x95, 0xC0, 0x48, 0x0F, 0xB6, 0xC0]);
    }

    fn escolha(&mut self, valor: &ExpressaoIr, casos: &[CasoEscolhaIr]) -> Result<(), String> {
        let tipo = tipo_expressao_ir(valor)
            .ok_or("Correspondência de padrões exige tipo algébrico conhecido.")?;
        self.validar_tipo_abi(tipo)?;
        self.expressao(valor)?;
        let nome_tag = format!("\\0nebula_tag_{}", self.proximo_slot);
        let slot_tag = self.declarar(&nome_tag)?;
        self.armazenar_rax(slot_tag);
        let nome_payload = format!("\\0nebula_payload_{}", self.proximo_slot);
        let slot_payload = self.declarar(&nome_payload)?;
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx
        self.armazenar_rax(slot_payload);

        let mut saltos_fim = Vec::new();
        let mut possui_curinga = false;
        for caso in casos {
            let proximo_caso = if let (Some(enumeracao), Some(variante)) =
                (&caso.enumeracao, &caso.variante)
            {
                let enumeracao = nome_enum_canonico(enumeracao);
                self.mov_rax(tag_enumeracao(enumeracao, variante));
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // mov rcx, tag esperada
                self.carregar_rax(slot_tag);
                self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]); // cmp rax, rcx
                Some(self.emitir_jcc(0x85))
            } else {
                possui_curinga = true;
                None
            };

            if caso.vinculacoes.len() > 1 {
                return Err("A ABI inicial de enumerações suporta um único vínculo de payload por caso.".into());
            }
            self.escopos.push(HashMap::new());
            self.tipos_escopos.push(HashMap::new());
            if let Some(vinculo) = caso.vinculacoes.first() {
                let deslocamento = self.declarar(vinculo)?;
                self.carregar_rax(slot_payload);
                self.armazenar_rax(deslocamento);
                self.mov_rdx(0);
                self.armazenar_rdx(deslocamento - 8);
            }
            self.bloco(&caso.corpo)?;
            let _ = self.escopos.pop();
            let _ = self.tipos_escopos.pop();
            saltos_fim.push(self.emitir_jmp());
            if let Some(salto) = proximo_caso {
                self.aplicar_salto(salto, self.codigo.len());
            }
        }
        if !possui_curinga {
            self.encerrar_com_erro(2);
        }
        let fim = self.codigo.len();
        for salto in saltos_fim {
            self.aplicar_salto(salto, fim);
        }
        Ok(())
    }

    fn construir_tupla(&mut self, tipos: &[Tipo], valores: &[ExpressaoIr]) -> Result<(), String> {
        if tipos.len() != valores.len() {
            return Err("Quantidade de elementos da tupla não corresponde ao tipo declarado.".into());
        }
        let (tamanho, campos) = self.layout_tupla(tipos)
            .ok_or("Layout de tupla não suportado pela ABI nativa.")?;
        if tamanho > 16 {
            let alinhamento = tipos.iter()
                .map(|tipo| self.tamanho_alinhamento_tipo(tipo).map(|(_, alinhamento)| alinhamento))
                .collect::<Option<Vec<_>>>()
                .ok_or("Alinhamento de elemento da tupla não suportado.")?
                .into_iter()
                .max()
                .unwrap_or(1);
            let armazenamento = self.reservar_agregado(tamanho, alinhamento)?;
            self.emitir_endereco_rbp_rax(armazenamento);
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7]);
            self.codigo.extend_from_slice(&[0x48, 0xB9]);
            self.codigo.extend_from_slice(&tamanho.to_le_bytes());
            self.codigo.extend_from_slice(&[0x31, 0xC0, 0xF3, 0xAA]);
            for (indice, (deslocamento, tamanho_campo)) in campos.iter().enumerate() {
                self.expressao(&valores[indice])?;
                self.armazenar_campo_agregado(armazenamento, *deslocamento, *tamanho_campo)?;
            }
            self.emitir_endereco_rbp_rax(armazenamento);
            self.mov_rdx(0);
            return Ok(());
        }        let primeiro = campos.iter().position(|(deslocamento, _)| *deslocamento == 0)
            .ok_or("Tupla não possui elemento no primeiro registrador.")?;
        let segundo = campos.iter().position(|(deslocamento, _)| *deslocamento == 8);
        self.expressao(&valores[primeiro])?;
        self.push_rax();
        if let Some(indice) = segundo {
            self.expressao(&valores[indice])?;
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC2]);
        } else {
            self.mov_rdx(0);
        }
        self.pop_rax();
        Ok(())
    }
    fn multiplicar_inteiro_128(&mut self, sem_sinal: bool) -> Result<(), String> {
        let mut falhas = Vec::new();

        // Para inteiros assinados, multiplica magnitudes e restaura o sinal.
        self.codigo.extend_from_slice(&[0x41, 0xBB, 0, 0, 0, 0]); // xor r11d, r11d
        if !sem_sinal {
            self.codigo.extend_from_slice(&[0x48, 0x85, 0xD2]); // test rdx, rdx
            let lhs_positivo = self.emitir_jcc(0x89); // jns
            self.codigo.extend_from_slice(&[0x41, 0x83, 0xF3, 0x01]); // xor r11d, 1
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD8]); // neg rax
            self.codigo.extend_from_slice(&[0xF5]); // cmc
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD2]); // not rdx
            self.codigo.extend_from_slice(&[0x48, 0x83, 0xD2, 0x00]); // adc rdx, 0
            self.aplicar_salto(lhs_positivo, self.codigo.len());

            self.codigo.extend_from_slice(&[0x48, 0x85, 0xF6]); // test rsi, rsi
            let rhs_positivo = self.emitir_jcc(0x89); // jns
            self.codigo.extend_from_slice(&[0x41, 0x83, 0xF3, 0x01]); // xor r11d, 1
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD9]); // neg rcx
            self.codigo.extend_from_slice(&[0xF5]); // cmc
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD6]); // not rsi
            self.codigo.extend_from_slice(&[0x48, 0x83, 0xD6, 0x00]); // adc rsi, 0
            self.aplicar_salto(rhs_positivo, self.codigo.len());
        }

        // Multiplicação binária de 128 iterações. Mantém os 128 bits baixos
        // e rejeita qualquer bit significativo que ultrapasse o resultado.
        self.codigo.extend_from_slice(&[0x49, 0x89, 0xD0]); // mov r8, rdx
        self.codigo.extend_from_slice(&[0x49, 0x89, 0xC1]); // mov r9, rax
        self.codigo.extend_from_slice(&[0x48, 0x31, 0xD2, 0x48, 0x31, 0xC0]); // resultado = 0
        self.codigo.extend_from_slice(&[0x41, 0xBA, 128, 0, 0, 0]); // r10d = 128
        let inicio = self.codigo.len();

        self.codigo.extend_from_slice(&[0xF6, 0xC1, 0x01]); // test cl, 1
        let sem_somar = self.emitir_jcc(0x84); // jz
        self.codigo.extend_from_slice(&[0x4C, 0x01, 0xC8]); // add rax, r9
        self.codigo.extend_from_slice(&[0x4C, 0x11, 0xC2]); // adc rdx, r8
        falhas.push(self.emitir_jcc(0x82)); // jc: overflow no resultado
        self.aplicar_salto(sem_somar, self.codigo.len());

        self.codigo.extend_from_slice(&[0x48, 0xD1, 0xEE]); // shr rsi, 1
        self.codigo.extend_from_slice(&[0x48, 0xD1, 0xD9]); // rcr rcx, 1
        self.codigo.extend_from_slice(&[0x49, 0xD1, 0xE1]); // shl r9, 1
        self.codigo.extend_from_slice(&[0x49, 0xD1, 0xD0]); // rcl r8, 1
        let sem_overflow_deslocamento = self.emitir_jcc(0x83); // jnc
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xF6]); // test rsi, rsi
        falhas.push(self.emitir_jcc(0x85)); // jnz
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]); // test rcx, rcx
        falhas.push(self.emitir_jcc(0x85)); // jnz
        self.aplicar_salto(sem_overflow_deslocamento, self.codigo.len());

        self.codigo.extend_from_slice(&[0x41, 0xFF, 0xCA]); // dec r10d
        let repetir = self.emitir_jcc(0x85); // jnz
        self.aplicar_salto(repetir, inicio);

        if !sem_sinal {
            self.codigo.extend_from_slice(&[0x45, 0x85, 0xDB]); // test r11d, r11d
            let resultado_positivo = self.emitir_jcc(0x84); // jz
            // A magnitude negativa pode alcançar exatamente 2^127, mas não ultrapassá-la.
            self.codigo.extend_from_slice(&[0x49, 0x89, 0xC1]); // mov r9, rax
            self.mov_rax(i64::MIN);
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // mov rsi, rax
            self.codigo.extend_from_slice(&[0x4C, 0x89, 0xC8]); // mov rax, r9
            self.codigo.extend_from_slice(&[0x48, 0x39, 0xF2]); // cmp rdx, rsi
            falhas.push(self.emitir_jcc(0x87)); // ja
            let magnitude_menor = self.emitir_jcc(0x85); // jne
            self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
            falhas.push(self.emitir_jcc(0x85)); // jnz
            self.aplicar_salto(magnitude_menor, self.codigo.len());
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD8]); // neg rax
            self.codigo.extend_from_slice(&[0xF5]); // cmc
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD2]); // not rdx
            self.codigo.extend_from_slice(&[0x48, 0x83, 0xD2, 0x00]); // adc rdx, 0
            let fim_negativo = self.emitir_jmp();

            let verificar_positivo = self.codigo.len();
            self.aplicar_salto(resultado_positivo, verificar_positivo);
            self.codigo.extend_from_slice(&[0x48, 0x85, 0xD2]); // test rdx, rdx
            falhas.push(self.emitir_jcc(0x88)); // js
            self.aplicar_salto(fim_negativo, self.codigo.len());
        }

        let fim = self.emitir_jmp();
        let erro = self.codigo.len();
        for salto in falhas {
            self.aplicar_salto(salto, erro);
        }
        self.encerrar_com_erro(2);
        self.aplicar_salto(fim, self.codigo.len());
        Ok(())
    }

    fn dividir_inteiro_128(&mut self, resto: bool, sem_sinal: bool) -> Result<(), String> {
        let mut falhas = Vec::new();

        // Divisão restauradora binária: o dividendo deslocado também se torna o quociente.
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xF6]); // test rsi, rsi
        let divisor_alto_zero = self.emitir_jcc(0x84);
        let divisor_valido = self.emitir_jmp();
        self.aplicar_salto(divisor_alto_zero, self.codigo.len());
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]); // test rcx, rcx
        falhas.push(self.emitir_jcc(0x84)); // divisor zero
        self.aplicar_salto(divisor_valido, self.codigo.len());

        self.codigo.extend_from_slice(&[0x41, 0xBB, 0, 0, 0, 0]); // r11d = flags de sinal
        if !sem_sinal {
            self.codigo.extend_from_slice(&[0x48, 0x85, 0xD2]); // test rdx, rdx
            let lhs_positivo = self.emitir_jcc(0x89);
            self.codigo.extend_from_slice(&[0x41, 0x83, 0xF3, 0x03]); // lhs negativo: quociente e resto negativos
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD8, 0xF5, 0x48, 0xF7, 0xD2, 0x48, 0x83, 0xD2, 0x00]);
            self.aplicar_salto(lhs_positivo, self.codigo.len());

            self.codigo.extend_from_slice(&[0x48, 0x85, 0xF6]); // test rsi, rsi
            let rhs_positivo = self.emitir_jcc(0x89);
            self.codigo.extend_from_slice(&[0x41, 0x83, 0xF3, 0x01]); // divisor negativo inverte sinal do quociente
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD9, 0xF5, 0x48, 0xF7, 0xD6, 0x48, 0x83, 0xD6, 0x00]);
            self.aplicar_salto(rhs_positivo, self.codigo.len());
        }

        // r8:r9 recebe o dividendo e será transformado no quociente.
        self.codigo.extend_from_slice(&[0x49, 0x89, 0xD0]); // mov r8, rdx
        self.codigo.extend_from_slice(&[0x49, 0x89, 0xC1]); // mov r9, rax
        self.codigo.extend_from_slice(&[0x48, 0x31, 0xD2, 0x48, 0x31, 0xC0]); // resto = 0
        self.codigo.extend_from_slice(&[0x41, 0xBA, 128, 0, 0, 0]); // r10d = 128
        let inicio = self.codigo.len();

        self.codigo.extend_from_slice(&[0x49, 0xD1, 0xE1]); // shl r9, 1
        self.codigo.extend_from_slice(&[0x49, 0xD1, 0xD0]); // rcl r8, 1
        self.codigo.extend_from_slice(&[0x48, 0xD1, 0xD0]); // rcl rax, 1
        self.codigo.extend_from_slice(&[0x48, 0xD1, 0xD2]); // rcl rdx, 1
        let subtrair_carry = self.emitir_jcc(0x82); // carry do bit 129 do resto
        self.codigo.extend_from_slice(&[0x48, 0x39, 0xF2]); // cmp rdx, rsi
        let subtrair_alto = self.emitir_jcc(0x87); // ja
        let manter_alto = self.emitir_jcc(0x82); // jb
        self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]); // cmp rax, rcx
        let subtrair_baixo = self.emitir_jcc(0x83); // jae
        let manter_baixo = self.emitir_jmp();

        let subtracao = self.codigo.len();
        self.aplicar_salto(subtrair_carry, subtracao);
        self.aplicar_salto(subtrair_alto, subtracao);
        self.aplicar_salto(subtrair_baixo, subtracao);
        self.codigo.extend_from_slice(&[0x48, 0x29, 0xC8]); // sub rax, rcx
        self.codigo.extend_from_slice(&[0x48, 0x19, 0xF2]); // sbb rdx, rsi
        self.codigo.extend_from_slice(&[0x49, 0x83, 0xC9, 0x01]); // or r9, 1
        let proxima = self.emitir_jmp();

        let sem_subtracao = self.codigo.len();
        self.aplicar_salto(manter_alto, sem_subtracao);
        self.aplicar_salto(manter_baixo, sem_subtracao);
        self.aplicar_salto(proxima, sem_subtracao);
        self.codigo.extend_from_slice(&[0x41, 0xFF, 0xCA]); // dec r10d
        let repetir = self.emitir_jcc(0x85);
        self.aplicar_salto(repetir, inicio);

        if !resto {
            self.codigo.extend_from_slice(&[0x4C, 0x89, 0xC2]); // mov rdx, r8
            self.codigo.extend_from_slice(&[0x4C, 0x89, 0xC8]); // mov rax, r9
            if !sem_sinal {
                self.codigo.extend_from_slice(&[0x41, 0xF6, 0xC3, 0x01]); // test r11d, 1
                let quociente_positivo = self.emitir_jcc(0x84);
                self.codigo.extend_from_slice(&[0x49, 0x89, 0xC1]); // save low in r9
                self.mov_rax(i64::MIN);
                self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = 2^127
                self.codigo.extend_from_slice(&[0x4C, 0x89, 0xC8]); // restore low
                self.codigo.extend_from_slice(&[0x48, 0x39, 0xF2]); // cmp high, 2^127
                falhas.push(self.emitir_jcc(0x87)); // ja
                let magnitude_menor = self.emitir_jcc(0x85); // jne
                self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test low, low
                falhas.push(self.emitir_jcc(0x85)); // jnz
                self.aplicar_salto(magnitude_menor, self.codigo.len());
                self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD8, 0xF5, 0x48, 0xF7, 0xD2, 0x48, 0x83, 0xD2, 0x00]);
                let fim_negativo = self.emitir_jmp();
                let positivo = self.codigo.len();
                self.aplicar_salto(quociente_positivo, positivo);
                self.codigo.extend_from_slice(&[0x48, 0x85, 0xD2]); // test high
                falhas.push(self.emitir_jcc(0x88)); // js
                self.aplicar_salto(fim_negativo, self.codigo.len());
            }
        } else if !sem_sinal {
            self.codigo.extend_from_slice(&[0x41, 0xF6, 0xC3, 0x02]); // test r11d, 2
            let resto_positivo = self.emitir_jcc(0x84);
            self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD8, 0xF5, 0x48, 0xF7, 0xD2, 0x48, 0x83, 0xD2, 0x00]);
            let fim_negativo = self.emitir_jmp();
            self.aplicar_salto(resto_positivo, self.codigo.len());
            self.aplicar_salto(fim_negativo, self.codigo.len());
        }

        let fim = self.emitir_jmp();
        let erro = self.codigo.len();
        for salto in falhas {
            self.aplicar_salto(salto, erro);
        }
        self.encerrar_com_erro(2);
        self.aplicar_salto(fim, self.codigo.len());
        Ok(())
    }

    fn comparar_inteiro_128(&mut self, operador: &OperadorIr, sem_sinal: bool) {
        self.codigo.extend_from_slice(&[0x48, 0x39, 0xF2]); // cmp rdx, rsi
        let (maior, menor, comparacao_baixa) = match operador {
            OperadorIr::Menor => (if sem_sinal { 0x82 } else { 0x8C }, if sem_sinal { 0x87 } else { 0x8F }, 0x92),
            OperadorIr::MenorIgual => (if sem_sinal { 0x82 } else { 0x8C }, if sem_sinal { 0x87 } else { 0x8F }, 0x96),
            OperadorIr::Maior => (if sem_sinal { 0x87 } else { 0x8F }, if sem_sinal { 0x82 } else { 0x8C }, 0x97),
            OperadorIr::MaiorIgual => (if sem_sinal { 0x87 } else { 0x8F }, if sem_sinal { 0x82 } else { 0x8C }, 0x93),
            _ => unreachable!(),
        };
        let ramo_verdadeiro = self.emitir_jcc(maior);
        let ramo_falso = self.emitir_jcc(menor);
        self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]); // cmp rax, rcx
        self.codigo.extend_from_slice(&[0x0F, comparacao_baixa, 0xC0, 0x48, 0x0F, 0xB6, 0xC0]);
        let fim_comparacao = self.emitir_jmp();
        let destino_verdadeiro = self.codigo.len();
        self.mov_rax(1);
        let fim_verdadeiro = self.emitir_jmp();
        let destino_falso = self.codigo.len();
        self.mov_rax(0);
        let fim = self.codigo.len();
        self.aplicar_salto(ramo_verdadeiro, destino_verdadeiro);
        self.aplicar_salto(ramo_falso, destino_falso);
        self.aplicar_salto(fim_comparacao, fim);
        self.aplicar_salto(fim_verdadeiro, fim);
    }

    fn preparar_argumentos(
        &mut self,
        argumentos: &[ExpressaoIr],
    ) -> Result<(u64, u64), String> {
        self.preparar_argumentos_desde(argumentos, 0)
    }

    fn preparar_argumentos_desde(
        &mut self,
        argumentos: &[ExpressaoIr],
        inicio_registrador: usize,
    ) -> Result<(u64, u64), String> {
        let registradores = [7u8, 6, 2, 1, 8, 9];
        let larguras = argumentos.iter()
            .map(|argumento| self.largura_abi_expressao(argumento))
            .collect::<Result<Vec<_>, _>>()?;
        let posicoes = planejar_posicoes_argumentos(&larguras, inicio_registrador)?;
        let mut temporarios = Vec::with_capacity(argumentos.len());

        for (argumento, largura) in argumentos.iter().zip(&larguras) {
            let temporario = self.reservar_agregado(16, 16)?;
            self.expressao(argumento)?;
            self.armazenar_rax(temporario);
            if *largura == 2 {
                self.armazenar_rdx(temporario + 8);
            }
            temporarios.push(temporario);
        }

        let tamanho_pilha = posicoes.iter().filter_map(|posicao| {
            if let PosicaoArgumento::Pilha { deslocamento, largura } = posicao {
                Some(*deslocamento + (*largura as u64) * 8)
            } else {
                None
            }
        }).max().unwrap_or(0);
        let tamanho_pilha = alinhar_pilha_16(tamanho_pilha)?;
        let preenchimento = if self.pilha % 2 == 1 { 8u64 } else { 0 };
        if preenchimento > 0 {
            self.codigo.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]);
            self.pilha += 1;
        }
        if tamanho_pilha > 0 {
            self.codigo.extend_from_slice(&[0x48, 0x81, 0xEC]);
            self.codigo.extend_from_slice(&u32::try_from(tamanho_pilha)
                .map_err(|_| "Área de argumentos excede o limite x86-64.")?.to_le_bytes());
            self.pilha += usize::try_from(tamanho_pilha / 8)
                .map_err(|_| "Área de argumentos excede o limite x86-64.")?;
        }

        for (indice, posicao) in posicoes.iter().enumerate() {
            let temporario = temporarios[indice];
            match posicao {
                PosicaoArgumento::Registradores(indice_registrador) => {
                    self.carregar_rax(temporario);
                    self.mover_rax_registrador(registradores[*indice_registrador]);
                    if larguras[indice] == 2 {
                        self.carregar_rax(temporario + 8);
                        self.mover_rax_registrador(registradores[*indice_registrador + 1]);
                    }
                }
                PosicaoArgumento::Pilha { deslocamento, largura } => {
                    self.carregar_rax(temporario);
                    self.armazenar_rax_rsp(*deslocamento)?;
                    if *largura == 2 {
                        self.carregar_rax(temporario + 8);
                        self.armazenar_rax_rsp(deslocamento + 8)?;
                    }
                }
            }
        }
        Ok((tamanho_pilha, preenchimento))
    }

    fn armazenar_rax_rsp(&mut self, deslocamento: u64) -> Result<(), String> {
        let deslocamento = i32::try_from(deslocamento)
            .map_err(|_| "Deslocamento de argumento excede o limite x86-64.")?;
        self.codigo.extend_from_slice(&[0x48, 0x89, 0x84, 0x24]);
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
        Ok(())
    }

    fn limpar_pilha_chamada(&mut self, tamanho_pilha: u64, preenchimento: u64) -> Result<(), String> {
        if tamanho_pilha > 0 {
            self.codigo.extend_from_slice(&[0x48, 0x81, 0xC4]);
            self.codigo.extend_from_slice(&u32::try_from(tamanho_pilha)
                .map_err(|_| "Área de argumentos excede o limite x86-64.")?.to_le_bytes());
            self.pilha = self.pilha.checked_sub(usize::try_from(tamanho_pilha / 8)
                .map_err(|_| "Área de argumentos excede o limite x86-64.")?)
                .ok_or("Contabilidade da pilha inválida após chamada.")?;
        }
        if preenchimento > 0 {
            self.codigo.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]);
            self.pilha = self.pilha.checked_sub(1)
                .ok_or("Contabilidade do alinhamento da pilha inválida.")?;
        }
        Ok(())
    }

    fn emitir_tempo_unix_milisegundos(&mut self) -> Result<(), String> {
        let timespec = self.reservar_agregado(16, 8)?;
        self.codigo.extend_from_slice(&[0x31, 0xFF]); // xor edi, edi: CLOCK_REALTIME
        self.emitir_endereco_rbp_rax(timespec);
        self.mover_rax_registrador(6); // rsi = timespec
        self.mov_rax(228); // SYS_clock_gettime
        self.codigo.extend_from_slice(&[0x0F, 0x05]);
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
        let sucesso = self.emitir_jcc(0x89); // jns
        self.encerrar_com_erro(1);
        self.aplicar_salto(sucesso, self.codigo.len());

        self.carregar_rax(timespec + 8); // tv_nsec
        self.codigo.extend_from_slice(&[0x48, 0x31, 0xD2]);
        self.codigo.extend_from_slice(&[0x48, 0xB9]);
        self.codigo.extend_from_slice(&1_000_000i64.to_le_bytes()); // rcx = 1_000_000
        self.codigo.extend_from_slice(&[0x48, 0xF7, 0xF1]); // tv_nsec / 1_000_000
        self.codigo.extend_from_slice(&[0x49, 0x89, 0xC0]); // r8 = milissegundos fracionários

        self.carregar_rax(timespec); // tv_sec
        self.codigo.extend_from_slice(&[0x48, 0x69, 0xC0, 0xE8, 0x03, 0x00, 0x00]); // * 1000
        self.codigo.extend_from_slice(&[0x4C, 0x01, 0xC0]); // + r8
        self.mov_rdx(0);
        Ok(())
    }

    fn emitir_dormir_milisegundos(&mut self, argumento: &ExpressaoIr) -> Result<(), String> {
        let timespec = self.reservar_agregado(16, 8)?;
        self.expressao(argumento)?;
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
        let nao_negativo = self.emitir_jcc(0x89); // jns
        self.encerrar_com_erro(2);
        self.aplicar_salto(nao_negativo, self.codigo.len());

        self.codigo.extend_from_slice(&[0x48, 0xB9, 0xE8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // rcx = 1000
        self.codigo.extend_from_slice(&[0x48, 0x31, 0xD2]); // xor rdx, rdx
        self.codigo.extend_from_slice(&[0x48, 0xF7, 0xF1]); // div rcx
        self.armazenar_rax(timespec); // tv_sec
        self.mov_rax(1_000_000);
        self.codigo.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xD0]); // imul rdx, rax
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xD0]); // rax = tv_nsec
        self.armazenar_rax(timespec + 8);
        self.emitir_endereco_rbp_rax(timespec);
        self.mover_rax_registrador(7); // rdi = timespec
        self.codigo.extend_from_slice(&[0x31, 0xF6]); // rsi = NULL
        self.mov_rax(35); // SYS_nanosleep
        self.codigo.extend_from_slice(&[0x0F, 0x05]);
        self.mov_rax(0);
        self.mov_rdx(0);
        Ok(())
    }

    /// Concatena dois textos usando mmap como arena de vida útil do processo.
    ///
    /// O valor texto é representado por (ponteiro em RAX, comprimento em RDX).
    /// A alocação permanece válida até o encerramento do processo, evitando
    /// liberar memória enquanto cópias do texto ainda podem estar em uso.
    fn concatenar_textos(
        &mut self,
        esquerda: &ExpressaoIr,
        direita: &ExpressaoIr,
    ) -> Result<(), String> {
        let temporario = self.reservar_agregado(48, 8)?;

        self.expressao(esquerda)?;
        self.armazenar_rax(temporario);
        self.armazenar_rdx(temporario - 8);

        self.expressao(direita)?;
        self.armazenar_rax(temporario - 16);
        self.armazenar_rdx(temporario - 24);

        // Soma os comprimentos com detecção de overflow.
        self.carregar_rax(temporario - 8);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = comprimento esquerdo
        self.carregar_rax(temporario - 24);
        self.codigo.extend_from_slice(&[0x48, 0x01, 0xC8]); // rax = direito + esquerdo
        let sem_overflow = self.emitir_jcc(0x83); // jnc
        self.encerrar_com_erro(2);
        self.aplicar_salto(sem_overflow, self.codigo.len());
        self.armazenar_rax(temporario - 32);

        // mmap(NULL, max(total, 1), PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS, -1, 0)
        self.carregar_rax(temporario - 32);
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
        let tamanho_nao_zero = self.emitir_jcc(0x85); // jne
        self.mov_rax(1);
        let tamanho_pronto = self.codigo.len();
        self.aplicar_salto(tamanho_nao_zero, tamanho_pronto);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = tamanho
        self.codigo.extend_from_slice(&[0x48, 0x31, 0xFF]); // rdi = NULL
        self.codigo.extend_from_slice(&[0xBA, 0x03, 0x00, 0x00, 0x00]); // rdx = PROT_READ|PROT_WRITE
        self.codigo.extend_from_slice(&[0x49, 0xC7, 0xC2, 0x22, 0x00, 0x00, 0x00]); // r10 = MAP_PRIVATE|MAP_ANONYMOUS
        self.codigo.extend_from_slice(&[0x49, 0xC7, 0xC0, 0xFF, 0xFF, 0xFF, 0xFF]); // r8 = -1
        self.codigo.extend_from_slice(&[0x45, 0x31, 0xC9]); // r9 = 0
        self.codigo.extend_from_slice(&[0xB8, 0x09, 0x00, 0x00, 0x00]); // SYS_mmap
        self.codigo.extend_from_slice(&[0x0F, 0x05]); // syscall
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
        let alocacao_ok = self.emitir_jcc(0x89); // jns
        self.encerrar_com_erro(1);
        self.aplicar_salto(alocacao_ok, self.codigo.len());
        self.armazenar_rax(temporario - 40);

        // Copia o primeiro texto para o início da região.
        self.carregar_rax(temporario - 40);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7]); // rdi = destino
        self.carregar_rax(temporario);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = origem
        self.carregar_rax(temporario - 8);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = comprimento
        self.codigo.extend_from_slice(&[0xFC, 0xF3, 0xA4]); // cld; rep movsb

        // REP MOVSB deixou RDI logo após o primeiro texto.
        self.carregar_rax(temporario - 16);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = segundo texto
        self.carregar_rax(temporario - 24);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = segundo comprimento
        self.codigo.extend_from_slice(&[0xFC, 0xF3, 0xA4]); // cld; rep movsb

        self.carregar_rax(temporario - 40);
        self.carregar_rax(temporario - 32);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC2]); // rdx = comprimento total
        self.carregar_rax(temporario - 40); // rax = ponteiro resultante
        Ok(())
    }

    fn emitir_igualdade_texto(
        &mut self,
        esquerda: &ExpressaoIr,
        direita: &ExpressaoIr,
        diferente: bool,
    ) -> Result<(), String> {
        if self.tipo_expressao_atual(direita).as_ref() != Some(&Tipo::Texto) {
            return Err("Comparação de texto exige dois operandos textuais.".into());
        }
        let temporario = self.reservar_agregado(32, 8)?;
        self.expressao(esquerda)?;
        self.armazenar_rax(temporario);
        self.armazenar_rdx(temporario - 8);
        self.expressao(direita)?;
        self.armazenar_rax(temporario - 16);
        self.armazenar_rdx(temporario - 24);

        self.carregar_rax(temporario - 8);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = bytes esquerda
        self.carregar_rax(temporario - 24);
        self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]); // cmp bytes direita, esquerda
        let tamanhos_diferentes = self.emitir_jcc(0x85); // jne
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]); // test rcx, rcx
        let vazio = self.emitir_jcc(0x84); // je

        self.carregar_rax(temporario);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = esquerda
        self.carregar_rax(temporario - 16);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7]); // rdi = direita
        self.carregar_rax(temporario - 24);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = bytes
        self.codigo.extend_from_slice(&[0xFC, 0xF3, 0xA6]); // cld; repe cmpsb
        self.codigo.extend_from_slice(&[0x0F, 0x94, 0xC0]); // sete al
        self.codigo.extend_from_slice(&[0x0F, 0xB6, 0xC0]); // movzx eax, al
        let apos_comparacao = self.emitir_jmp();

        let falso = self.codigo.len();
        self.mov_rax(0);
        let apos_falso = self.emitir_jmp();
        let verdadeiro = self.codigo.len();
        self.mov_rax(1);
        let fim = self.codigo.len();
        self.aplicar_salto(tamanhos_diferentes, falso);
        self.aplicar_salto(vazio, verdadeiro);
        self.aplicar_salto(apos_comparacao, fim);
        self.aplicar_salto(apos_falso, fim);
        if diferente {
            self.codigo.extend_from_slice(&[0x83, 0xF0, 0x01]); // xor eax, 1
        }
        self.mov_rdx(0);
        Ok(())
    }

    fn emitir_contem_texto(
        &mut self,
        texto: &ExpressaoIr,
        trecho: &ExpressaoIr,
    ) -> Result<(), String> {
        let temporario = self.reservar_agregado(48, 8)?;
        self.expressao(texto)?;
        self.armazenar_rax(temporario);
        self.armazenar_rdx(temporario - 8);
        self.expressao(trecho)?;
        self.armazenar_rax(temporario - 16);
        self.armazenar_rdx(temporario - 24);

        self.carregar_rax(temporario - 24);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = bytes do trecho
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]);
        let trecho_vazio = self.emitir_jcc(0x84);
        self.carregar_rax(temporario - 8); // rax = bytes do texto
        self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]); // cmp rax, rcx
        let trecho_maior = self.emitir_jcc(0x82);
        self.codigo.extend_from_slice(&[0x48, 0x29, 0xC8]); // rax = texto - trecho
        self.codigo.extend_from_slice(&[0x48, 0xFF, 0xC0]); // + 1 candidato
        self.armazenar_rax(temporario - 40);
        self.carregar_rax(temporario);
        self.armazenar_rax(temporario - 32);

        let inicio = self.codigo.len();
        self.carregar_rax(temporario - 32);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = candidato
        self.carregar_rax(temporario - 16);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7]); // rdi = trecho
        self.carregar_rax(temporario - 24);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = bytes do trecho
        self.codigo.extend_from_slice(&[0xFC, 0xF3, 0xA6]); // cld; repe cmpsb
        let encontrado = self.emitir_jcc(0x84); // je

        self.carregar_rax(temporario - 32);
        self.codigo.extend_from_slice(&[0x48, 0xFF, 0xC0]); // próximo byte candidato
        self.armazenar_rax(temporario - 32);
        self.carregar_rax(temporario - 40);
        self.codigo.extend_from_slice(&[0x48, 0xFF, 0xC8]); // dec candidatos restantes
        self.armazenar_rax(temporario - 40);
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]);
        let continuar = self.emitir_jcc(0x85); // jne

        let falso = self.codigo.len();
        self.mov_rax(0);
        let apos_falso = self.emitir_jmp();
        let verdadeiro = self.codigo.len();
        self.mov_rax(1);
        let fim = self.codigo.len();
        self.aplicar_salto(trecho_vazio, verdadeiro);
        self.aplicar_salto(trecho_maior, falso);
        self.aplicar_salto(encontrado, verdadeiro);
        self.aplicar_salto(continuar, inicio);
        self.aplicar_salto(apos_falso, fim);
        self.mov_rdx(0);
        Ok(())
    }

    fn emitir_comparacao_prefixo_sufixo_texto(
        &mut self,
        texto: &ExpressaoIr,
        trecho: &ExpressaoIr,
        sufixo: bool,
    ) -> Result<(), String> {
        let temporario = self.reservar_agregado(32, 8)?;
        self.expressao(texto)?;
        self.armazenar_rax(temporario);
        self.armazenar_rdx(temporario - 8);
        self.expressao(trecho)?;
        self.armazenar_rax(temporario - 16);
        self.armazenar_rdx(temporario - 24);

        self.carregar_rax(temporario - 8); // rax = bytes do texto
        self.codigo.extend_from_slice(&[0x49, 0x89, 0xC0]); // r8 = bytes do texto
        self.carregar_rax(temporario - 24); // rax = bytes do trecho
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = bytes do trecho
        self.codigo.extend_from_slice(&[0x4C, 0x89, 0xC0]); // rax = bytes do texto
        self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]); // cmp rax, rcx
        let trecho_maior = self.emitir_jcc(0x82); // jb
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]); // test rcx, rcx
        let trecho_vazio = self.emitir_jcc(0x84); // je

        if sufixo {
            self.codigo.extend_from_slice(&[0x48, 0x29, 0xC8]); // rax = bytes texto - bytes trecho
            self.codigo.extend_from_slice(&[0x49, 0x89, 0xC0]); // r8 = deslocamento
        }
        self.carregar_rax(temporario);
        if sufixo {
            self.codigo.extend_from_slice(&[0x4C, 0x01, 0xC0]); // rax += deslocamento
        }
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = texto
        self.carregar_rax(temporario - 16);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC7]); // rdi = trecho
        self.carregar_rax(temporario - 24);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC1]); // rcx = bytes do trecho
        self.codigo.extend_from_slice(&[0xFC, 0xF3, 0xA6]); // cld; repe cmpsb
        self.codigo.extend_from_slice(&[0x0F, 0x94, 0xC0]); // sete al
        self.codigo.extend_from_slice(&[0x0F, 0xB6, 0xC0]); // movzx eax, al
        let apos_comparacao = self.emitir_jmp();

        let falso = self.codigo.len();
        self.mov_rax(0);
        let apos_falso = self.emitir_jmp();
        let verdadeiro = self.codigo.len();
        self.mov_rax(1);
        let fim = self.codigo.len();
        self.aplicar_salto(trecho_maior, falso);
        self.aplicar_salto(trecho_vazio, verdadeiro);
        self.aplicar_salto(apos_comparacao, fim);
        self.aplicar_salto(apos_falso, fim);
        self.mov_rdx(0);
        Ok(())
    }

    fn emitir_comprimento_texto(&mut self, argumento: &ExpressaoIr) -> Result<(), String> {
        self.expressao(argumento)?;
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = início UTF-8
        self.codigo.extend_from_slice(&[0x48, 0x8D, 0x0C, 0x10]); // rcx = início + bytes
        self.codigo.extend_from_slice(&[0x45, 0x31, 0xC0]); // r8d = quantidade de escalares
        let inicio = self.codigo.len();
        self.codigo.extend_from_slice(&[0x48, 0x39, 0xCE]); // cmp rsi, rcx
        let fim = self.emitir_jcc(0x84);
        self.codigo.extend_from_slice(&[0x0F, 0xB6, 0x06]); // movzx eax, byte [rsi]
        self.codigo.extend_from_slice(&[0x3C, 0x80]); // cmp al, 0x80
        let ascii = self.emitir_jcc(0x82);
        self.codigo.extend_from_slice(&[0x3C, 0xE0]); // cmp al, 0xE0
        let dois_bytes = self.emitir_jcc(0x82);
        self.codigo.extend_from_slice(&[0x3C, 0xF0]); // cmp al, 0xF0
        let tres_bytes = self.emitir_jcc(0x82);

        self.codigo.extend_from_slice(&[0x48, 0x83, 0xC6, 0x04]); // UTF-8 de quatro bytes
        let contar_quatro = self.emitir_jmp();
        let inicio_dois = self.codigo.len();
        self.codigo.extend_from_slice(&[0x48, 0x83, 0xC6, 0x02]);
        let contar_dois = self.emitir_jmp();
        let inicio_tres = self.codigo.len();
        self.codigo.extend_from_slice(&[0x48, 0x83, 0xC6, 0x03]);
        let contar_tres = self.emitir_jmp();
        let inicio_ascii = self.codigo.len();
        self.codigo.extend_from_slice(&[0x48, 0xFF, 0xC6]); // UTF-8 de um byte
        let contar = self.codigo.len();
        self.codigo.extend_from_slice(&[0x49, 0xFF, 0xC0]); // inc r8
        self.emitir_salto_para(inicio);

        let finalizado = self.codigo.len();
        self.aplicar_salto(fim, finalizado);
        self.aplicar_salto(ascii, inicio_ascii);
        self.aplicar_salto(dois_bytes, inicio_dois);
        self.aplicar_salto(tres_bytes, inicio_tres);
        self.aplicar_salto(contar_quatro, contar);
        self.aplicar_salto(contar_dois, contar);
        self.aplicar_salto(contar_tres, contar);
        self.codigo.extend_from_slice(&[0x4C, 0x89, 0xC0]); // rax = r8
        self.mov_rdx(0);
        Ok(())
    }

    /// Lê uma linha de stdin para um buffer local do quadro da função.
    /// O limite explícito evita sobrescrever a pilha: entradas maiores falham
    /// com código estável em vez de corromper memória.
    fn emitir_ler_texto(&mut self, argumentos: &[ExpressaoIr]) -> Result<(), String> {
        const LIMITE_ENTRADA: i64 = 4096;
        let buffer = self.reservar_agregado((LIMITE_ENTRADA as u64) + 1, 8)?;
        let slot_cursor = self.reservar_agregado(8, 8)?;
        let slot_comprimento = self.reservar_agregado(8, 8)?;
        let slot_buffer = self.reservar_agregado(8, 8)?;

        if let Some(prompt) = argumentos.first() {
            self.expressao(prompt)?;
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = texto
            self.mov_rax(1); // SYS_write
            self.codigo.extend_from_slice(&[0xBF, 1, 0, 0, 0]); // stdout
            self.codigo.extend_from_slice(&[0x0F, 0x05]);
        }

        self.emitir_endereco_rbp_rax(buffer);
        self.armazenar_rax(slot_buffer);
        self.armazenar_rax(slot_cursor);
        self.mov_rax(0);
        self.armazenar_rax(slot_comprimento);

        let inicio_leitura = self.codigo.len();
        self.carregar_rax(slot_cursor);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = cursor
        self.codigo.extend_from_slice(&[0x31, 0xC0]); // eax = SYS_read
        self.codigo.extend_from_slice(&[0x31, 0xFF]); // stdin
        self.codigo.extend_from_slice(&[0xBA, 1, 0, 0, 0]); // um byte
        self.codigo.extend_from_slice(&[0x0F, 0x05]); // syscall
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
        let fim_entrada = self.emitir_jcc(0x84); // EOF
        let leitura_valida = self.emitir_jcc(0x89); // jns
        self.encerrar_com_erro(1);
        let apos_erro = self.codigo.len();
        self.aplicar_salto(leitura_valida, apos_erro);

        self.carregar_rax(slot_cursor);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = cursor
        self.codigo.extend_from_slice(&[0x80, 0x3E, 10]); // cmp byte [rsi], LF
        let encontrou_linha = self.emitir_jcc(0x84);
        self.codigo.extend_from_slice(&[0x48, 0xFF, 0xC6]); // próximo byte
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xB5]);
        self.codigo.extend_from_slice(&slot_cursor.to_le_bytes()); // salva cursor

        self.carregar_rax(slot_comprimento);
        self.codigo.extend_from_slice(&[0x48, 0xFF, 0xC0]); // comprimento++
        self.armazenar_rax(slot_comprimento);
        self.codigo.extend_from_slice(&[0x48, 0x3D]);
        self.codigo.extend_from_slice(&(LIMITE_ENTRADA as u32).to_le_bytes());
        let limite = self.emitir_jcc(0x86); // jbe: tamanho ainda dentro do limite
        self.encerrar_com_erro(1);
        let apos_limite = self.codigo.len();
        self.aplicar_salto(limite, apos_limite);
        self.emitir_salto_para(inicio_leitura);

        let fim = self.codigo.len();
        self.aplicar_salto(fim_entrada, fim);
        self.aplicar_salto(encontrou_linha, fim);

        // Remove CR imediatamente anterior ao LF ou ao EOF.
        self.carregar_rax(slot_comprimento);
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
        let vazio = self.emitir_jcc(0x84);
        self.carregar_rax(slot_cursor);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = cursor
        self.codigo.extend_from_slice(&[0x80, 0x7E, 0xFF, 13]); // cmp byte [rsi-1], CR
        let sem_cr = self.emitir_jcc(0x85);
        self.codigo.extend_from_slice(&[0x48, 0xFF, 0xCE]); // rsi--
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xB5]);
        self.codigo.extend_from_slice(&slot_cursor.to_le_bytes());
        self.carregar_rax(slot_comprimento);
        self.codigo.extend_from_slice(&[0x48, 0xFF, 0xC8]); // comprimento--
        self.armazenar_rax(slot_comprimento);

        let finalizado = self.codigo.len();
        self.aplicar_salto(vazio, finalizado);
        self.aplicar_salto(sem_cr, finalizado);

        self.carregar_rax(slot_buffer);
        self.carregar_rdx(slot_comprimento);
        Ok(())
    }

    fn emitir_exibir_textos(&mut self, argumentos: &[ExpressaoIr]) -> Result<(), String> {
        if argumentos.is_empty() {
            let nova_linha = self.reservar_agregado(8, 1)?;
            self.codigo.extend_from_slice(&[0xC6, 0x85]);
            self.codigo.extend_from_slice(&nova_linha.to_le_bytes());
            self.codigo.push(b'\n');
            self.emitir_endereco_rbp_rax(nova_linha);
            self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]);
            self.mov_rdx(1);
            self.mov_rax(1);
            self.codigo.extend_from_slice(&[0xBF, 1, 0, 0, 0]);
            self.codigo.extend_from_slice(&[0x0F, 0x05]);
            self.mov_rax(0);
            self.mov_rdx(0);
            return Ok(());
        }
        let quantidade = argumentos.len();
        for (indice, argumento) in argumentos.iter().enumerate() {
            self.emitir_exibir_texto(argumento, indice + 1 == quantidade)?;
        }
        Ok(())
    }

    fn emitir_exibir_texto(&mut self, argumento: &ExpressaoIr, ultima: bool) -> Result<(), String> {
        let temporario = self.reservar_agregado(16, 8)?;
        let nova_linha = self.reservar_agregado(8, 1)?;
        self.expressao(argumento)?;
        self.armazenar_rax(temporario);
        self.armazenar_rdx(temporario - 8);

        self.carregar_rax(temporario);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]); // rsi = ponteiro
        self.carregar_rdx(temporario - 8);
        self.mov_rax(1); // SYS_write
        self.codigo.extend_from_slice(&[0xBF, 1, 0, 0, 0]); // stdout
        self.codigo.extend_from_slice(&[0x0F, 0x05]);

        self.codigo.extend_from_slice(&[0xC6, 0x85]);
        self.codigo.extend_from_slice(&nova_linha.to_le_bytes());
        self.codigo.push(if ultima { b'\n' } else { b' ' });
        self.emitir_endereco_rbp_rax(nova_linha);
        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC6]);
        self.mov_rdx(1);
        self.mov_rax(1);
        self.codigo.extend_from_slice(&[0xBF, 1, 0, 0, 0]);
        self.codigo.extend_from_slice(&[0x0F, 0x05]);
        self.mov_rax(0);
        self.mov_rdx(0);
        Ok(())
    }

    fn push_rax(&mut self) {
        self.codigo.push(0x50);
        self.pilha += 1;
    }

    fn pop_rax(&mut self) {
        self.codigo.push(0x58);
        self.pilha = self.pilha.saturating_sub(1);
    }

    fn mov_rax(&mut self, valor: i64) {
        self.codigo.extend_from_slice(&[0x48, 0xB8]);
        self.codigo.extend_from_slice(&valor.to_le_bytes());
    }

    fn armazenar_rax(&mut self, deslocamento: i32) {
        self.codigo.extend_from_slice(&[0x48, 0x89, 0x85]);
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
    }

    fn armazenar_rdx(&mut self, deslocamento: i32) {
        self.codigo.extend_from_slice(&[0x48, 0x89, 0x95]);
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
    }

    fn carregar_rdx(&mut self, deslocamento: i32) {
        self.codigo.extend_from_slice(&[0x48, 0x8B, 0x95]);
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
    }

    fn carregar_rax(&mut self, deslocamento: i32) {
        self.codigo.extend_from_slice(&[0x48, 0x8B, 0x85]);
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
    }

    #[cfg(test)]
    fn armazenar_registrador(&mut self, registrador: u8, deslocamento: i32) {
        self.codigo.push(if registrador >= 8 { 0x4C } else { 0x48 });
        self.codigo.extend_from_slice(&[0x89, 0x85 | ((registrador & 7) << 3)]);
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
    }

    fn mover_registrador_rax(&mut self, registrador: u8) {
        let rex = if registrador >= 8 { 0x4C } else { 0x48 };
        self.codigo.extend_from_slice(&[rex, 0x89, 0xC0 | ((registrador & 7) << 3)]);
    }

    fn mov_rdx(&mut self, valor: i64) {
        self.codigo.extend_from_slice(&[0x48, 0xBA]);
        self.codigo.extend_from_slice(&valor.to_le_bytes());
    }

    fn mover_rax_registrador(&mut self, registrador: u8) {
        let rex = if registrador >= 8 { 0x49 } else { 0x48 };
        self.codigo.extend_from_slice(&[rex, 0x89, 0xC0 | (registrador & 7)]);
    }

    fn emitir_jcc(&mut self, condicao: u8) -> usize {
        self.codigo.extend_from_slice(&[0x0F, condicao]);
        let posicao = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 4]);
        posicao
    }

    fn emitir_jmp(&mut self) -> usize {
        self.codigo.push(0xE9);
        let posicao = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 4]);
        posicao
    }

    fn emitir_salto_para(&mut self, destino: usize) {
        let salto = self.emitir_jmp();
        self.aplicar_salto(salto, destino);
    }

    fn aplicar_salto(&mut self, posicao: usize, destino: usize) {
        let Some(fim_operando) = posicao.checked_add(4) else {
            self.erro_salto = Some("Posição de relocação de salto excedeu o limite do alvo.".into());
            return;
        };
        if fim_operando > self.codigo.len() {
            self.erro_salto = Some("Relocação de salto aponta para fora do código gerado.".into());
            return;
        }
        if destino > self.codigo.len() {
            self.erro_salto = Some("Destino de salto aponta para fora do código gerado.".into());
            return;
        }
        let origem = match i128::try_from(fim_operando) {
            Ok(valor) => valor,
            Err(_) => { self.erro_salto = Some("Origem de salto excedeu o limite do alvo.".into()); return; }
        };
        let destino = match i128::try_from(destino) {
            Ok(valor) => valor,
            Err(_) => { self.erro_salto = Some("Destino de salto excedeu o limite do alvo.".into()); return; }
        };
        let relativo = match calcular_relativo32(origem, destino) {
            Ok(valor) => valor,
            Err(erro) => { self.erro_salto = Some(erro); return; }
        };
        self.codigo[posicao..fim_operando].copy_from_slice(&relativo.to_le_bytes());
    }

    fn finalizar(mut self) -> Result<(Vec<u8>, Vec<u8>), String> {
        if let Some(erro) = self.erro_salto.take() { return Err(erro); }
        for (posicao, nome) in self.chamadas {
            let destino = self.funcoes.get(&nome)
                .ok_or_else(|| format!("Função '{}' não foi encontrada durante a geração nativa.", nome))?;
            let fim_referencia = posicao.checked_add(4)
                .ok_or("Posição de chamada excedeu o limite do alvo.")?;
            let relativo = calcular_relativo32(
                i128::try_from(fim_referencia).map_err(|_| "Origem de chamada excedeu o limite do alvo.")?,
                i128::try_from(*destino).map_err(|_| "Destino de chamada excedeu o limite do alvo.")?,
            )?;
            self.codigo[posicao..posicao + 4].copy_from_slice(&relativo.to_le_bytes());
        }

        for (posicao, nome) in self.referencias_funcoes {
            let deslocamento = self.funcoes.get(&nome)
                .ok_or_else(|| format!("Função referenciada '{nome}' não foi encontrada durante a geração nativa."))?;
            let destino = 0x401000u64.checked_add(u64::try_from(*deslocamento)
                .map_err(|_| "Endereço da função excedeu o limite ELF64.")?)
                .ok_or("Endereço da função excedeu o limite ELF64.")?;
            let fim = posicao.checked_add(8).ok_or("Referência de função excedeu o limite do alvo.")?;
            self.codigo.get_mut(posicao..fim)
                .ok_or("Referência de função fora do segmento de código.")?
                .copy_from_slice(&destino.to_le_bytes());
        }

        let fim_texto = TAMANHO_CABECALHO.checked_add(self.codigo.len())
            .ok_or("Tamanho do segmento de código excedeu o limite do alvo.")?;
        let deslocamento_dados = alinhar_pagina(fim_texto)?;
        let endereco_dados = 0x400000u64.checked_add(u64::try_from(deslocamento_dados)
            .map_err(|_| "Deslocamento dos dados excedeu o limite ELF64.")?)
            .ok_or("Endereço dos dados excedeu o limite ELF64.")?;
        for (posicao, deslocamento) in self.referencias_texto {
            let destino = endereco_dados.checked_add(u64::try_from(deslocamento)
                .map_err(|_| "Deslocamento de texto excedeu o limite ELF64.")?)
                .ok_or("Endereço de texto excedeu o limite ELF64.")?;
            let fim_referencia = posicao.checked_add(4)
                .ok_or("Posição de referência de texto excedeu o limite do alvo.")?;
            let origem = 0x401000u64.checked_add(u64::try_from(fim_referencia)
                .map_err(|_| "Posição de referência de texto excedeu o limite ELF64.")?)
                .ok_or("Endereço da instrução excedeu o limite ELF64.")?;
            let relativo = calcular_relativo32(i128::from(origem), i128::from(destino))
                .map_err(|_| "Texto estático fora do alcance RIP-relative x86-64.")?;
            self.codigo[posicao..fim_referencia].copy_from_slice(&relativo.to_le_bytes());
        }
        for (posicao, nome, parte) in self.referencias_globais {
            let deslocamento = self.globais.get(&nome)
                .and_then(|base| base.checked_add(usize::try_from(parte).ok()?))
                .ok_or_else(|| format!("Variável global '{nome}' não foi alocada."))?;
            let destino = endereco_dados.checked_add(u64::try_from(deslocamento)
                .map_err(|_| "Deslocamento da variável global excedeu o limite ELF64.")?)
                .ok_or("Endereço da variável global excedeu o limite ELF64.")?;
            let fim_referencia = posicao.checked_add(4)
                .ok_or("Posição de referência global excedeu o limite ELF64.")?;
            let origem = 0x401000u64.checked_add(u64::try_from(fim_referencia)
                .map_err(|_| "Posição de referência global excedeu o limite ELF64.")?)
                .ok_or("Endereço da instrução excedeu o limite ELF64.")?;
            let relativo = calcular_relativo32(i128::from(origem), i128::from(destino))
                .map_err(|_| "Variável global fora do alcance RIP-relative x86-64.")?;
            self.codigo[posicao..posicao + 4].copy_from_slice(&relativo.to_le_bytes());
        }
        let mut dados = vec![0; self.globais.len().checked_mul(16)
            .ok_or("Tamanho dos dados globais excedeu o limite do alvo.")?];
        dados.extend_from_slice(&self.dados_estaticos);
        Ok((self.codigo, dados))
    }
}

fn calcular_relativo32(origem_apos_campo: i128, destino: i128) -> Result<i32, String> {
    let relativo = destino.checked_sub(origem_apos_campo)
        .ok_or("Deslocamento relativo excedeu o limite aritmético do alvo.")?;
    i32::try_from(relativo)
        .map_err(|_| "Deslocamento fora do alcance relativo de 32 bits do x86-64.".into())
}

fn eh_inteiro_sem_sinal(expressao: &ExpressaoIr) -> bool {
    match expressao {
        ExpressaoIr::Normalizar { tipo, .. } | ExpressaoIr::Conversao { tipo, .. } =>
            matches!(tipo, Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32 | Tipo::InteiroU64 | Tipo::Byte),
        ExpressaoIr::Unaria { valor, .. } => eh_inteiro_sem_sinal(valor),
        _ => false,
    }
}

fn tipo_decimal(tipo: &Tipo) -> bool {
    matches!(tipo, Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64)
}

fn nome_enum_canonico(nome: &str) -> &str {
    match nome {
        "opcional" => "Opcional",
        "resultado" => "Resultado",
        _ => nome,
    }
}

fn tag_enumeracao(nome: &str, variante: &str) -> i64 {
    let identificador = format!("{}::{variante}", nome_enum_canonico(nome));
    let mut hash = 0xCBF2_9CE4_8422_2325u64;
    for byte in identificador.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash as i64
}

fn texto_literal_ir(expressao: &ExpressaoIr) -> Option<&str> {
    match expressao {
        ExpressaoIr::Constante(ValorIr::Texto(texto)) => Some(texto),
        ExpressaoIr::Normalizar { valor, .. } | ExpressaoIr::Conversao { valor, .. } => {
            texto_literal_ir(valor)
        }
        _ => None,
    }
}

fn indice_literal_ir(expressao: &ExpressaoIr) -> Option<usize> {
    match expressao {
        ExpressaoIr::Constante(ValorIr::Inteiro(valor)) => usize::try_from(*valor).ok(),
        ExpressaoIr::Constante(ValorIr::InteiroSemSinal(valor)) => usize::try_from(*valor).ok(),
        ExpressaoIr::Normalizar { valor, .. } => indice_literal_ir(valor),
        _ => None,
    }
}

fn tipo_expressao_ir(expressao: &ExpressaoIr) -> Option<&Tipo> {
    match expressao {
        ExpressaoIr::Normalizar { tipo, .. }
        | ExpressaoIr::Conversao { tipo, .. }
        | ExpressaoIr::ReferenciaFuncao { tipo, .. }
        | ExpressaoIr::ReferenciaFuncaoQualificada { tipo, .. } => Some(tipo),
        ExpressaoIr::Unaria { valor, .. } => tipo_expressao_ir(valor),
        _ => None,
    }
}

fn tipo_inteiro_128(tipo: &Tipo) -> bool {
    matches!(tipo, Tipo::Inteiro128 | Tipo::InteiroU128)
}

fn inteiro_128_expressao(expressao: &ExpressaoIr) -> bool {
    match expressao {
        ExpressaoIr::Normalizar { tipo, .. } | ExpressaoIr::Conversao { tipo, .. } => tipo_inteiro_128(tipo),
        ExpressaoIr::Unaria { valor, .. } => inteiro_128_expressao(valor),
        _ => false,
    }
}

fn inteiro_128_sem_sinal_expressao(expressao: &ExpressaoIr) -> bool {
    match expressao {
        ExpressaoIr::Normalizar { tipo, .. } | ExpressaoIr::Conversao { tipo, .. } => *tipo == Tipo::InteiroU128,
        ExpressaoIr::Unaria { valor, .. } => inteiro_128_sem_sinal_expressao(valor),
        _ => false,
    }
}

fn tipo_inteiro_abi(tipo: &Tipo) -> Option<(u8, bool)> {
    match tipo {
        Tipo::Inteiro | Tipo::Inteiro64 => Some((64, true)),
        Tipo::Inteiro8 => Some((8, true)),
        Tipo::Inteiro16 => Some((16, true)),
        Tipo::Inteiro32 => Some((32, true)),
        Tipo::InteiroU8 | Tipo::Byte => Some((8, false)),
        Tipo::InteiroU16 => Some((16, false)),
        Tipo::InteiroU32 => Some((32, false)),
        Tipo::InteiroU64 => Some((64, false)),
        Tipo::Inteiro128 => Some((128, true)),
        Tipo::InteiroU128 => Some((128, false)),
        _ => None,
    }
}

fn tipo_decimal_expressao(expressao: &ExpressaoIr) -> Option<Tipo> {
    match expressao {
        ExpressaoIr::Normalizar { tipo, .. } | ExpressaoIr::Conversao { tipo, .. }
            if matches!(tipo, Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64) => Some(tipo.clone()),
        ExpressaoIr::Unaria { valor, .. } => tipo_decimal_expressao(valor),
        ExpressaoIr::Constante(ValorIr::Decimal(_)) => Some(Tipo::Decimal64),
        _ => None,
    }
}

fn validar_tipo(tipo: &Tipo) -> Result<(), String> {
    match tipo {
        Tipo::Vazio | Tipo::Logico | Tipo::Inteiro | Tipo::Inteiro64
        | Tipo::Inteiro8 | Tipo::Inteiro16 | Tipo::Inteiro32 | Tipo::Inteiro128 | Tipo::InteiroU128
        | Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32 | Tipo::InteiroU64 | Tipo::Byte
        | Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64 | Tipo::Caractere | Tipo::Texto => Ok(()),
        Tipo::Funcao(parametros, retorno) => {
            for parametro in parametros {
                if *parametro == Tipo::Vazio {
                    return Err("Parâmetro de função não pode ter tipo vazio no ABI x86-64.".into());
                }
                validar_tipo(parametro)?;
            }
            validar_tipo(retorno)
        }
        _ => Err(format!("Tipo '{}' ainda não é suportado pelo backend sem runtime.", tipo.nome())),
    }
}

fn alinhar_pagina(valor: usize) -> Result<usize, String> {
    valor.checked_add(0xFFF)
        .map(|valor| valor & !0xFFF)
        .ok_or("Tamanho do executável excedeu o limite do alvo ELF64.")
        .map_err(String::from)
}

fn gerar_elf(codigo: &[u8], dados_conteudo: &[u8]) -> Result<Vec<u8>, String> {
    let tamanho_dados = dados_conteudo.len();
    let fim_texto = TAMANHO_CABECALHO.checked_add(codigo.len())
        .ok_or("Tamanho do segmento de código excedeu o limite do alvo.")?;
    let deslocamento_dados = alinhar_pagina(fim_texto)?;
    let tamanho_total = if tamanho_dados == 0 {
        fim_texto
    } else {
        deslocamento_dados.checked_add(tamanho_dados)
            .ok_or("Tamanho do segmento de dados excedeu o limite do alvo.")?
    };
    let mut elf = vec![0u8; tamanho_total];
    elf[0..4].copy_from_slice(b"\x7FELF");
    elf[4] = 2; // ELFCLASS64
    elf[5] = 1; // little endian
    elf[6] = 1; // ELF version
    elf[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    elf[18..20].copy_from_slice(&62u16.to_le_bytes()); // EM_X86_64
    elf[20..24].copy_from_slice(&1u32.to_le_bytes());
    elf[24..32].copy_from_slice(&0x401000u64.to_le_bytes());
    elf[32..40].copy_from_slice(&64u64.to_le_bytes()); // e_phoff
    elf[52..54].copy_from_slice(&64u16.to_le_bytes());
    elf[54..56].copy_from_slice(&56u16.to_le_bytes());
    elf[56..58].copy_from_slice(&(if tamanho_dados == 0 { 2u16 } else { 3u16 }).to_le_bytes());

    // Primeiro PT_LOAD: cabeçalhos e código, somente leitura e execução.
    let ph = 64;
    elf[ph..ph + 4].copy_from_slice(&1u32.to_le_bytes());
    elf[ph + 4..ph + 8].copy_from_slice(&5u32.to_le_bytes()); // PF_R | PF_X
    elf[ph + 8..ph + 16].copy_from_slice(&0u64.to_le_bytes());
    elf[ph + 16..ph + 24].copy_from_slice(&0x400000u64.to_le_bytes());
    elf[ph + 24..ph + 32].copy_from_slice(&0x400000u64.to_le_bytes());
    elf[ph + 32..ph + 40].copy_from_slice(&(fim_texto as u64).to_le_bytes());
    elf[ph + 40..ph + 48].copy_from_slice(&(fim_texto as u64).to_le_bytes());
    elf[ph + 48..ph + 56].copy_from_slice(&0x1000u64.to_le_bytes());

    if tamanho_dados > 0 {
        // Segundo PT_LOAD: dados globais graváveis, sem tornar o código gravável.
        let dados = 120;
        let endereco_dados = 0x400000u64 + deslocamento_dados as u64;
        elf[dados..dados + 4].copy_from_slice(&1u32.to_le_bytes());
        elf[dados + 4..dados + 8].copy_from_slice(&6u32.to_le_bytes()); // PF_R | PF_W
        elf[dados + 8..dados + 16].copy_from_slice(&(deslocamento_dados as u64).to_le_bytes());
        elf[dados + 16..dados + 24].copy_from_slice(&endereco_dados.to_le_bytes());
        elf[dados + 24..dados + 32].copy_from_slice(&endereco_dados.to_le_bytes());
        elf[dados + 32..dados + 40].copy_from_slice(&(tamanho_dados as u64).to_le_bytes());
        elf[dados + 40..dados + 48].copy_from_slice(&(tamanho_dados as u64).to_le_bytes());
        elf[dados + 48..dados + 56].copy_from_slice(&0x1000u64.to_le_bytes());
    }

    // PT_GNU_STACK: a pilha de execução não precisa ser executável.
    let pilha = if tamanho_dados > 0 { 176 } else { 120 };
    elf[pilha..pilha + 4].copy_from_slice(&0x6474_E551u32.to_le_bytes());
    elf[pilha + 4..pilha + 8].copy_from_slice(&6u32.to_le_bytes()); // PF_R | PF_W, sem PF_X
    elf[pilha + 48..pilha + 56].copy_from_slice(&16u64.to_le_bytes());

    elf[TAMANHO_CABECALHO..fim_texto].copy_from_slice(codigo);
    if tamanho_dados > 0 {
        elf[deslocamento_dados..deslocamento_dados + tamanho_dados]
            .copy_from_slice(dados_conteudo);
    }
    Ok(elf)
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashMap;

    fn programa_com_retorno(valor: i128) -> ProgramaIr {
        ProgramaIr {
            instrucoes: Vec::new(),
            funcoes: HashMap::from([("principal".into(), FuncaoIr {
                nome: "principal".into(), parametros: Vec::new(), retorno: Tipo::Inteiro,
                corpo: vec![InstrucaoIr::Devolver(Some(ExpressaoIr::Constante(ValorIr::Inteiro(valor))))],
            })]),
            modulos: HashMap::new(),
            layouts_tipos: HashMap::new(),
        }
    }

    #[cfg(target_os = "linux")]
    fn executar_elf_de_teste(programa: &ProgramaIr) -> i32 {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Mutex;
        static CONTADOR: AtomicUsize = AtomicUsize::new(0);
        static EXECUCAO_ELF: Mutex<()> = Mutex::new(());

        let _guarda = EXECUCAO_ELF.lock().unwrap();
        let caminho = std::env::temp_dir().join(format!(
            "nebula-backend-{}-{}.elf",
            std::process::id(),
            CONTADOR.fetch_add(1, Ordering::Relaxed),
        ));
        let binario = gerar_executavel(programa).expect("geração ELF de teste");
        std::fs::write(&caminho, binario).expect("gravação ELF de teste");
        let mut permissoes = std::fs::metadata(&caminho).unwrap().permissions();
        permissoes.set_mode(0o700);
        std::fs::set_permissions(&caminho, permissoes).unwrap();
        let status = std::process::Command::new(&caminho)
            .status()
            .expect("execução ELF de teste");
        std::fs::remove_file(&caminho).unwrap();
        status.code().expect("código de saída ELF")
    }

    #[cfg(target_os = "linux")]
    fn programa_com_operacao_128(
        tipo: Tipo,
        operador: OperadorIr,
        esquerda: ValorIr,
        direita: ValorIr,
    ) -> ProgramaIr {
        let mut programa = programa_com_retorno(0);
        let principal = programa.funcoes.get_mut("principal").unwrap();
        principal.retorno = tipo.clone();
        principal.corpo = vec![InstrucaoIr::Devolver(Some(ExpressaoIr::Binaria {
            esquerda: Box::new(ExpressaoIr::Normalizar {
                tipo: tipo.clone(),
                valor: Box::new(ExpressaoIr::Constante(esquerda)),
            }),
            operador,
            direita: Box::new(ExpressaoIr::Normalizar {
                tipo,
                valor: Box::new(ExpressaoIr::Constante(direita)),
            }),
        }))];
        programa
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn executa_aritmetica_nativa_de_inteiros_128_bits() {
        let casos = [
            (Tipo::Inteiro128, OperadorIr::Vezes, ValorIr::Inteiro(7), ValorIr::Inteiro(9), 63),
            (Tipo::Inteiro128, OperadorIr::Divide, ValorIr::Inteiro(27), ValorIr::Inteiro(5), 5),
            (Tipo::Inteiro128, OperadorIr::Resto, ValorIr::Inteiro(27), ValorIr::Inteiro(5), 2),
            (Tipo::Inteiro128, OperadorIr::Divide, ValorIr::Inteiro(-27), ValorIr::Inteiro(5), 251),
            (Tipo::Inteiro128, OperadorIr::Resto, ValorIr::Inteiro(-27), ValorIr::Inteiro(5), 254),
            (Tipo::Inteiro128, OperadorIr::Divide, ValorIr::Inteiro(27), ValorIr::Inteiro(-5), 251),
            (Tipo::Inteiro128, OperadorIr::Resto, ValorIr::Inteiro(27), ValorIr::Inteiro(-5), 2),
            (Tipo::Inteiro128, OperadorIr::Divide, ValorIr::Inteiro(-27), ValorIr::Inteiro(-5), 5),
            (Tipo::Inteiro128, OperadorIr::Resto, ValorIr::Inteiro(-27), ValorIr::Inteiro(-5), 254),
            (Tipo::InteiroU128, OperadorIr::Vezes, ValorIr::InteiroSemSinal((1u128 << 64) + 3), ValorIr::InteiroSemSinal(3), 9),
            (Tipo::InteiroU128, OperadorIr::Divide, ValorIr::InteiroSemSinal((1u128 << 64) + 5), ValorIr::InteiroSemSinal(3), 87),
            (Tipo::InteiroU128, OperadorIr::Resto, ValorIr::InteiroSemSinal((1u128 << 64) + 5), ValorIr::InteiroSemSinal(5), 1),
        ];
        for (tipo, operador, esquerda, direita, esperado) in casos {
            let programa = programa_com_operacao_128(tipo, operador, esquerda, direita);
            assert_eq!(executar_elf_de_teste(&programa), esperado, "{operador:?}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn executa_conversoes_nativas_decimal_inteiro128() {
        let mut programa = programa_com_retorno(0);
        let principal = programa.funcoes.get_mut("principal").unwrap();
        principal.retorno = Tipo::Inteiro128;
        principal.corpo = vec![InstrucaoIr::Devolver(Some(ExpressaoIr::Conversao {
            tipo: Tipo::Inteiro128,
            origem: Tipo::Decimal64,
            valor: Box::new(ExpressaoIr::Constante(ValorIr::Decimal(42.9))),
        }))];
        assert_eq!(executar_elf_de_teste(&programa), 42);

        let mut programa = programa_com_retorno(0);
        let principal = programa.funcoes.get_mut("principal").unwrap();
        principal.retorno = Tipo::Inteiro128;
        principal.corpo = vec![InstrucaoIr::Devolver(Some(ExpressaoIr::Conversao {
            tipo: Tipo::Inteiro128,
            origem: Tipo::Decimal64,
            valor: Box::new(ExpressaoIr::Constante(ValorIr::Decimal(-42.9))),
        }))];
        assert_eq!(executar_elf_de_teste(&programa), 214);

        let mut programa = programa_com_retorno(0);
        let principal = programa.funcoes.get_mut("principal").unwrap();
        principal.retorno = Tipo::Inteiro64;
        principal.corpo = vec![InstrucaoIr::Devolver(Some(ExpressaoIr::Conversao {
            tipo: Tipo::Inteiro64,
            origem: Tipo::Decimal64,
            valor: Box::new(ExpressaoIr::Conversao {
                tipo: Tipo::Decimal64,
                origem: Tipo::Inteiro128,
                valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(-42))),
            }),
        }))];
        assert_eq!(executar_elf_de_teste(&programa), 214);

        let grande = ExpressaoIr::Conversao {
            tipo: Tipo::Inteiro128,
            origem: Tipo::Decimal64,
            valor: Box::new(ExpressaoIr::Constante(ValorIr::Decimal(18_446_744_073_709_555_712.0))),
        };
        let divisao = ExpressaoIr::Binaria {
            esquerda: Box::new(grande.clone()),
            operador: OperadorIr::Divide,
            direita: Box::new(ExpressaoIr::Normalizar {
                tipo: Tipo::Inteiro128,
                valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(1i128 << 64))),
            }),
        };
        let mut programa = programa_com_retorno(0);
        let principal = programa.funcoes.get_mut("principal").unwrap();
        principal.retorno = Tipo::Inteiro64;
        principal.corpo = vec![InstrucaoIr::Devolver(Some(ExpressaoIr::Conversao {
            tipo: Tipo::Inteiro64,
            origem: Tipo::Inteiro128,
            valor: Box::new(divisao),
        }))];
        assert_eq!(executar_elf_de_teste(&programa), 1);

        let subtracao = ExpressaoIr::Binaria {
            esquerda: Box::new(grande),
            operador: OperadorIr::Menos,
            direita: Box::new(ExpressaoIr::Normalizar {
                tipo: Tipo::Inteiro128,
                valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(1i128 << 64))),
            }),
        };
        let comparacao = ExpressaoIr::Binaria {
            esquerda: Box::new(ExpressaoIr::Normalizar {
                tipo: Tipo::Inteiro128,
                valor: Box::new(subtracao),
            }),
            operador: OperadorIr::Igual,
            direita: Box::new(ExpressaoIr::Normalizar {
                tipo: Tipo::Inteiro128,
                valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(4096))),
            }),
        };
        let mut programa = programa_com_retorno(0);
        let principal = programa.funcoes.get_mut("principal").unwrap();
        principal.retorno = Tipo::Inteiro64;
        principal.corpo = vec![InstrucaoIr::Devolver(Some(comparacao))];
        // O processo só expõe os oito bits inferiores do código de saída;
        // compare o valor completo dentro do programa gerado.
        assert_eq!(executar_elf_de_teste(&programa), 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejeita_overflow_e_divisao_por_zero_em_inteiros_128_bits() {
        let casos = [
            (Tipo::Inteiro128, OperadorIr::Vezes, ValorIr::Inteiro(i128::MAX), ValorIr::Inteiro(2)),
            (Tipo::Inteiro128, OperadorIr::Divide, ValorIr::Inteiro(i128::MIN), ValorIr::Inteiro(-1)),
            (Tipo::InteiroU128, OperadorIr::Vezes, ValorIr::InteiroSemSinal(u128::MAX), ValorIr::InteiroSemSinal(2)),
            (Tipo::InteiroU128, OperadorIr::Divide, ValorIr::InteiroSemSinal(5), ValorIr::InteiroSemSinal(0)),
        ];
        for (tipo, operador, esquerda, direita) in casos {
            let programa = programa_com_operacao_128(tipo, operador, esquerda, direita);
            assert_eq!(executar_elf_de_teste(&programa), 2, "{operador:?}");
        }
    }

    #[test]
    fn produz_cabecalho_elf64_linux_x86_64() {
        let binario = gerar_executavel(&programa_com_retorno(7)).unwrap();
        assert_eq!(&binario[..4], b"\x7FELF");
        assert_eq!(binario[4], 2);
        assert_eq!(binario[5], 1);
        assert_eq!(u16::from_le_bytes([binario[18], binario[19]]), 62);
        assert_eq!(u64::from_le_bytes(binario[24..32].try_into().unwrap()), 0x401000);
    }

    #[test]
    fn cabecalho_elf_tem_segmento_carregado_coerente() {
        let binario = gerar_executavel(&programa_com_retorno(0)).unwrap();
        assert_eq!(u64::from_le_bytes(binario[32..40].try_into().unwrap()), 64);
        assert_eq!(u16::from_le_bytes([binario[52], binario[53]]), 64);
        assert_eq!(u16::from_le_bytes([binario[54], binario[55]]), 56);
        assert_eq!(u16::from_le_bytes([binario[56], binario[57]]), 2);
        assert_eq!(u32::from_le_bytes(binario[64..68].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(binario[68..72].try_into().unwrap()), 5);
        assert_eq!(u64::from_le_bytes(binario[72..80].try_into().unwrap()), 0);
        assert_eq!(u64::from_le_bytes(binario[80..88].try_into().unwrap()), 0x400000);
        assert_eq!(u64::from_le_bytes(binario[96..104].try_into().unwrap()), binario.len() as u64);
        assert_eq!(u64::from_le_bytes(binario[104..112].try_into().unwrap()), binario.len() as u64);
        assert_eq!(u64::from_le_bytes(binario[112..120].try_into().unwrap()), 0x1000);
        assert_eq!(u32::from_le_bytes(binario[120..124].try_into().unwrap()), 0x6474_E551);
        assert_eq!(u32::from_le_bytes(binario[124..128].try_into().unwrap()), 6);
    }

    #[test]
    fn gera_segmento_de_dados_rw_para_variaveis_globais() {
        let mut programa = programa_com_retorno(0);
        programa.instrucoes.push(InstrucaoIr::Declarar {
            nome: "global".into(),
            tipo: Tipo::Inteiro,
            valor: ExpressaoIr::Constante(ValorIr::Inteiro(42)),
            constante: false,
        });
        programa.funcoes.get_mut("principal").unwrap().corpo = vec![
            InstrucaoIr::Devolver(Some(ExpressaoIr::Variavel("global".into())))
        ];
        let binario = gerar_executavel(&programa).unwrap();
        assert_eq!(u16::from_le_bytes([binario[56], binario[57]]), 3);
        assert_eq!(u32::from_le_bytes(binario[120..124].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(binario[124..128].try_into().unwrap()), 6);
        let deslocamento = u64::from_le_bytes(binario[128..136].try_into().unwrap());
        let endereco = u64::from_le_bytes(binario[136..144].try_into().unwrap());
        assert_eq!(deslocamento % 0x1000, endereco % 0x1000);
        assert_eq!(u64::from_le_bytes(binario[152..160].try_into().unwrap()), 16);
        assert_eq!(u64::from_le_bytes(binario[160..168].try_into().unwrap()), 16);
        assert_eq!(u32::from_le_bytes(binario[176..180].try_into().unwrap()), 0x6474_E551);
        assert_eq!(u32::from_le_bytes(binario[180..184].try_into().unwrap()), 6);
    }

    #[test]
    fn gera_codigo_de_retorno_sem_compilador_externo() {
        let binario = gerar_executavel(&programa_com_retorno(42)).unwrap();
        assert!(binario.len() > TAMANHO_CABECALHO);
        assert!(binario[TAMANHO_CABECALHO..].windows(2).any(|janela| janela == [0x0F, 0x05]));
    }

    #[test]
    fn executavel_nao_declara_interpretador_ou_bibliotecas_dinamicas() {
        let binario = gerar_executavel(&programa_com_retorno(0)).unwrap();
        assert_eq!(u16::from_le_bytes([binario[56], binario[57]]), 2);
        assert_eq!(u32::from_le_bytes(binario[64..68].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(binario[120..124].try_into().unwrap()), 0x6474_E551);
        assert_eq!(u32::from_le_bytes(binario[124..128].try_into().unwrap()), 6);
    }

    #[test]
    fn aceita_potencia_inteira_sem_runtime() {
        let mut programa = programa_com_retorno(0);
        programa.funcoes.get_mut("principal").unwrap().corpo = vec![
            InstrucaoIr::Devolver(Some(ExpressaoIr::Binaria {
                esquerda: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(2))),
                operador: OperadorIr::Potencia,
                direita: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(10))),
            }))
        ];
        let binario = gerar_executavel(&programa).unwrap();
        assert!(binario.len() > TAMANHO_CABECALHO);
    }

    #[test]
    fn aceita_funcao_com_argumentos_na_pilha() {
        let mut programa = programa_com_retorno(0);
        programa.funcoes.insert("excesso".into(), FuncaoIr {
            nome: "excesso".into(),
            parametros: (0..7).map(|indice| (format!("p{indice}"), Tipo::Inteiro)).collect(),
            retorno: Tipo::Inteiro,
            corpo: Vec::new(),
        });
        assert!(gerar_executavel(&programa).is_ok(), "argumentos excedentes devem ser recebidos pela pilha");
    }

    #[test]
    fn rejeita_quadro_local_maior_que_o_limite() {
        let mut programa = programa_com_retorno(0);
        let corpo = (0..=LIMITE_VARIAVEIS).map(|indice| InstrucaoIr::Declarar {
            nome: format!("v{indice}"),
            tipo: Tipo::Inteiro,
            valor: ExpressaoIr::Constante(ValorIr::Inteiro(0)),
            constante: false,
        }).collect();
        programa.funcoes.get_mut("principal").unwrap().corpo = corpo;
        assert!(gerar_executavel(&programa).unwrap_err().contains("512 posições"));
    }

    #[test]
    fn aceita_exatamente_512_posicoes_locais() {
        let mut programa = programa_com_retorno(0);
        programa.funcoes.get_mut("principal").unwrap().corpo = (0..LIMITE_VARIAVEIS)
            .map(|indice| InstrucaoIr::Declarar {
                nome: format!("v{indice}"),
                tipo: Tipo::Inteiro,
                valor: ExpressaoIr::Constante(ValorIr::Inteiro(0)),
                constante: false,
            })
            .collect();
        assert!(gerar_executavel(&programa).is_ok());
    }

    #[test]
    fn parametros_e_corpo_compartilham_escopo_da_funcao() {
        let mut programa = programa_com_retorno(0);
        programa.funcoes.insert(
            "auxiliar".into(),
            FuncaoIr {
                nome: "auxiliar".into(),
                parametros: vec![("valor".into(), Tipo::Inteiro)],
                retorno: Tipo::Inteiro,
                corpo: vec![InstrucaoIr::Declarar {
                    nome: "valor".into(),
                    tipo: Tipo::Inteiro,
                    valor: ExpressaoIr::Constante(ValorIr::Inteiro(1)),
                    constante: false,
                }],
            },
        );
        assert!(
            gerar_executavel(&programa)
                .unwrap_err()
                .contains("mesmo escopo")
        );
    }

    #[test]
    fn rejeita_declaracao_duplicada_no_mesmo_escopo() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.escopos.push(HashMap::new());
        emissor.tipos_escopos.push(HashMap::new());
        assert!(emissor.declarar("valor").is_ok());
        assert!(emissor.declarar("valor").is_err());
    }

    #[test]
    fn rejeita_exibir_de_tipo_nao_textual_no_backend_nativo() {
        let mut programa = programa_com_retorno(0);
        programa.funcoes.get_mut("principal").unwrap().corpo = vec![
            InstrucaoIr::Expressao(ExpressaoIr::Chamada { nome: "exibir".into(), argumentos: vec![ExpressaoIr::Constante(ValorIr::Inteiro(1))] })
        ];
        assert!(gerar_executavel(&programa).unwrap_err().contains("tipo texto"));
    }

    #[test]
    fn compila_builtins_json_com_textos_literais() {
        let mut valido = programa_com_retorno(0);
        valido.funcoes.get_mut("principal").unwrap().corpo = vec![
            InstrucaoIr::Expressao(ExpressaoIr::Chamada {
                nome: "json_valido".into(),
                argumentos: vec![ExpressaoIr::Constante(ValorIr::Texto("{\"x\":1}".into()))],
            }),
        ];
        assert!(gerar_executavel(&valido).is_ok());

        let mut canonico = programa_com_retorno(0);
        canonico.funcoes.get_mut("principal").unwrap().corpo = vec![
            InstrucaoIr::Expressao(ExpressaoIr::Chamada {
                nome: "json_canonico".into(),
                argumentos: vec![ExpressaoIr::Constante(ValorIr::Texto("{\"b\":2,\"a\":1}".into()))],
            }),
        ];
        assert!(gerar_executavel(&canonico).is_ok());
    }

    #[test]
    fn distribui_argumentos_excedentes_na_pilha_sem_dividir_agregados() {
        let posicoes = planejar_posicoes_argumentos(&[1, 1, 1, 1, 1, 2, 1], 0).unwrap();
        assert!(matches!(posicoes[0], PosicaoArgumento::Registradores(0)));
        assert!(matches!(posicoes[4], PosicaoArgumento::Registradores(4)));
        assert!(matches!(posicoes[5], PosicaoArgumento::Pilha { deslocamento: 0, largura: 2 }));
        assert!(matches!(posicoes[6], PosicaoArgumento::Registradores(5)));
    }

    #[test]
    fn prepara_chamada_com_setimo_argumento_na_pilha() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.expressao(&ExpressaoIr::Chamada {
            nome: "funcao_com_sete_argumentos".into(),
            argumentos: (0..7)
                .map(|valor| ExpressaoIr::Constante(ValorIr::Inteiro(valor)))
                .collect(),
        }).unwrap();
        assert!(emissor.codigo.windows(7).any(|bytes| bytes == [0x48, 0x81, 0xEC, 16, 0, 0, 0]));
        assert!(emissor.codigo.windows(4).any(|bytes| bytes == [0x48, 0x89, 0x84, 0x24]));
        assert_eq!(emissor.pilha, 0);
    }

    #[test]
    fn valida_limites_de_layout_dos_agregados_na_abi() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.layouts_tipos.insert(
            "ParDuplo".into(),
            LayoutAgregado {
                tamanho: 16,
                alinhamento: 8,
                campos: HashMap::from([
                    ("primeiro".into(), (0, 8)),
                    ("segundo".into(), (8, 8)),
                ]),
            },
        );
        emissor.layouts_tipos.insert(
            "AgregadoGrande".into(),
            LayoutAgregado {
                tamanho: 24,
                alinhamento: 8,
                campos: HashMap::from([
                    ("primeiro".into(), (0, 8)),
                    ("segundo".into(), (8, 8)),
                    ("terceiro".into(), (16, 8)),
                ]),
            },
        );

        assert!(emissor.validar_tipo_abi(&Tipo::Estrutura("ParDuplo".into())).is_ok());
        assert!(emissor.validar_tipo_abi(&Tipo::Estrutura("AgregadoGrande".into())).is_ok());
        assert_eq!(emissor.largura_abi_tipo(&Tipo::Estrutura("AgregadoGrande".into())).unwrap(), 1);
        assert!(emissor.validar_tipo_abi(&Tipo::Tupla(vec![
            Tipo::Inteiro64,
            Tipo::Inteiro64,
        ])).is_ok());
        let tupla_grande = Tipo::Tupla(vec![
            Tipo::Inteiro64,
            Tipo::Inteiro64,
            Tipo::Inteiro64,
        ]);
        assert!(emissor.validar_tipo_abi(&tupla_grande).is_ok());
        assert_eq!(emissor.largura_abi_tipo(&tupla_grande).unwrap(), 1);
    }

    #[test]
    fn construi_estrutura_maior_que_dezesseis_bytes_em_memoria() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.layouts_tipos.insert(
            "RegistroGrande".into(),
            LayoutAgregado {
                tamanho: 24,
                alinhamento: 8,
                campos: HashMap::from([
                    ("primeiro".into(), (0, 8)),
                    ("segundo".into(), (8, 8)),
                    ("terceiro".into(), (16, 8)),
                ]),
            },
        );
        emissor.expressao(&ExpressaoIr::ConstrucaoEstrutura {
            nome: "RegistroGrande".into(),
            campos: vec![
                ("primeiro".into(), ExpressaoIr::Constante(ValorIr::Inteiro(11))),
                ("segundo".into(), ExpressaoIr::Constante(ValorIr::Inteiro(22))),
                ("terceiro".into(), ExpressaoIr::Constante(ValorIr::Inteiro(33))),
            ],
        }).unwrap();
        assert!(emissor.codigo.windows(2).any(|bytes| bytes == [0xF3, 0xAA]));
        assert!(!emissor.codigo.windows(2).any(|bytes| bytes == [0xF3, 0xA4]));
        assert!(emissor.codigo.windows(3).any(|bytes| bytes == [0x49, 0x89, 0x83]));
    }

    #[test]
    fn copia_e_atribui_campos_de_agregado_grande() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.layouts_tipos.insert(
            "RegistroGrande".into(),
            LayoutAgregado {
                tamanho: 24,
                alinhamento: 8,
                campos: HashMap::from([
                    ("primeiro".into(), (0, 8)),
                    ("segundo".into(), (8, 8)),
                    ("terceiro".into(), (16, 8)),
                ]),
            },
        );
        let construcao = ExpressaoIr::Normalizar {
            tipo: Tipo::Estrutura("RegistroGrande".into()),
            valor: Box::new(ExpressaoIr::ConstrucaoEstrutura {
                nome: "RegistroGrande".into(),
                campos: vec![
                    ("primeiro".into(), ExpressaoIr::Constante(ValorIr::Inteiro(1))),
                    ("segundo".into(), ExpressaoIr::Constante(ValorIr::Inteiro(2))),
                    ("terceiro".into(), ExpressaoIr::Constante(ValorIr::Inteiro(3))),
                ],
            }),
        };
        emissor.compilar_funcao(&FuncaoIr {
            nome: "principal".into(),
            parametros: Vec::new(),
            retorno: Tipo::Inteiro64,
            corpo: vec![
                InstrucaoIr::Declarar {
                    nome: "registro".into(),
                    tipo: Tipo::Estrutura("RegistroGrande".into()),
                    valor: construcao.clone(),
                    constante: false,
                },
                InstrucaoIr::Expressao(ExpressaoIr::AtribuicaoCampo {
                    raiz: "registro".into(),
                    caminho: vec!["terceiro".into()],
                    valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(99))),
                    tipo: Tipo::Inteiro64,
                }),
                InstrucaoIr::Expressao(ExpressaoIr::Atribuicao {
                    nome: "registro".into(),
                    valor: Box::new(construcao),
                    tipo: Tipo::Estrutura("RegistroGrande".into()),
                }),
            ],
        }).unwrap();
        assert!(emissor.codigo.windows(2).any(|bytes| bytes == [0xF3, 0xA4]));
        assert!(emissor.codigo.windows(3).any(|bytes| bytes == [0x49, 0x89, 0x83]));
    }

    #[test]
    fn acessa_campo_de_agregado_grande_por_deslocamento_real() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.layouts_tipos.insert(
            "RegistroGrande".into(),
            LayoutAgregado {
                tamanho: 24,
                alinhamento: 8,
                campos: HashMap::from([
                    ("primeiro".into(), (0, 8)),
                    ("segundo".into(), (8, 8)),
                    ("terceiro".into(), (16, 8)),
                ]),
            },
        );
        let agregado = ExpressaoIr::Normalizar {
            tipo: Tipo::Estrutura("RegistroGrande".into()),
            valor: Box::new(ExpressaoIr::ConstrucaoEstrutura {
                nome: "RegistroGrande".into(),
                campos: vec![
                    ("primeiro".into(), ExpressaoIr::Constante(ValorIr::Inteiro(11))),
                    ("segundo".into(), ExpressaoIr::Constante(ValorIr::Inteiro(22))),
                    ("terceiro".into(), ExpressaoIr::Constante(ValorIr::Inteiro(33))),
                ],
            }),
        };
        emissor.expressao(&ExpressaoIr::Normalizar {
            tipo: Tipo::Inteiro64,
            valor: Box::new(ExpressaoIr::Membro {
                alvo: Box::new(agregado),
                campo: "terceiro".into(),
            }),
        }).unwrap();
        assert!(emissor.codigo.windows(7).any(|bytes| bytes == [0x49, 0x8B, 0x83, 16, 0, 0, 0]));
    }

    #[test]
    fn rejeita_construcao_de_enum_com_varios_payloads_no_backend() {
        let mut emissor = Emissor::novo(HashMap::new());
        let erro = emissor.expressao(&ExpressaoIr::ConstrucaoEnumeracao {
            nome: "Evento".into(),
            variante: "Dados".into(),
            valores: vec![
                ExpressaoIr::Constante(ValorIr::Inteiro(1)),
                ExpressaoIr::Constante(ValorIr::Inteiro(2)),
            ],
        }).unwrap_err();

        assert!(erro.contains("no máximo um valor de payload"));
    }

    #[test]
    fn rejeita_enum_com_payload_maior_que_um_registro_na_abi() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.layouts_tipos.insert(
            "Evento".into(),
            LayoutAgregado {
                tamanho: 24,
                alinhamento: 8,
                campos: HashMap::from([("payload".into(), (8, 16))]),
            },
        );
        let erro = emissor.validar_tipo_abi(&Tipo::Enumeracao("Evento".into())).unwrap_err();
        assert!(erro.contains("16 bytes") || erro.contains("registrador"));
    }

    #[test]
    fn rejeita_tipo_nao_suportado_sem_fingir_implementacao() {
        assert!(validar_tipo(&Tipo::Texto).is_ok());
        assert!(validar_tipo(&Tipo::Inteiro).is_ok());
        assert!(validar_tipo(&Tipo::Inteiro64).is_ok());
        assert!(validar_tipo(&Tipo::Inteiro8).is_ok());
        assert!(validar_tipo(&Tipo::InteiroU32).is_ok());
        assert!(validar_tipo(&Tipo::Byte).is_ok());
        assert!(validar_tipo(&Tipo::Inteiro128).is_ok());
        assert!(validar_tipo(&Tipo::InteiroU128).is_ok());
    }

    #[test]
    fn conversao_entre_i64_e_u64_verifica_sinal() {
        for (destino, origem) in [
            (Tipo::Inteiro64, Tipo::InteiroU64),
            (Tipo::InteiroU64, Tipo::Inteiro64),
        ] {
            let expressao = ExpressaoIr::Conversao {
                tipo: destino,
                origem: origem.clone(),
                valor: Box::new(ExpressaoIr::Normalizar {
                    tipo: origem,
                    valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(1))),
                }),
            };
            let mut emissor = Emissor::novo(HashMap::new());
            emissor.expressao(&expressao).unwrap();
            assert!(emissor.codigo.windows(3).any(|janela| janela == [0x48, 0x85, 0xC0]));
            assert!(emissor.codigo.windows(2).any(|janela| janela == [0x0F, 0x89]));
            assert!(emissor.codigo.windows(2).any(|janela| janela == [0x0F, 0x05]));
        }
    }

    #[test]
    fn converte_decimal_para_inteiro_com_validacao_de_faixa() {
        for destino in [Tipo::Inteiro8, Tipo::Inteiro64, Tipo::InteiroU32, Tipo::InteiroU64] {
            let expressao = ExpressaoIr::Conversao {
                tipo: destino,
                origem: Tipo::Decimal64,
                valor: Box::new(ExpressaoIr::Normalizar {
                    tipo: Tipo::Decimal64,
                    valor: Box::new(ExpressaoIr::Constante(ValorIr::Decimal(42.75))),
                }),
            };
            let mut emissor = Emissor::novo(HashMap::new());
            emissor.expressao(&expressao).unwrap();
            assert!(emissor.codigo.windows(5).any(|janela| janela == [0xF2, 0x48, 0x0F, 0x2C, 0xC0]));
            assert!(emissor.codigo.windows(4).any(|janela| janela == [0x66, 0x0F, 0x2E, 0xC0]));
        }
    }

    #[test]
    fn converte_inteiros_assinados_e_sem_sinal_no_backend() {
        let origem = ExpressaoIr::Normalizar {
            tipo: Tipo::Inteiro64,
            valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(300))),
        };
        let conversao = ExpressaoIr::Conversao {
            tipo: Tipo::Inteiro16,
            origem: Tipo::Inteiro64,
            valor: Box::new(origem),
        };
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.expressao(&conversao).unwrap();
        assert!(emissor.codigo.windows(4).any(|janela| janela == [0x48, 0xC1, 0xE0, 48]));

        let origem = ExpressaoIr::Normalizar {
            tipo: Tipo::InteiroU64,
            valor: Box::new(ExpressaoIr::Constante(ValorIr::InteiroSemSinal(300))),
        };
        let conversao = ExpressaoIr::Conversao {
            tipo: Tipo::Decimal64,
            origem: Tipo::InteiroU64,
            valor: Box::new(origem),
        };
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.expressao(&conversao).unwrap();
        assert!(emissor.codigo.windows(5).any(|janela| janela == [0xF2, 0x48, 0x0F, 0x2A, 0xC0]));
        assert!(emissor.codigo.windows(5).any(|janela| janela == [0xF2, 0x48, 0x0F, 0x2A, 0xC9]));
    }

    #[test]
    fn normaliza_decimal32_para_ieee754_binary32() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.normalizar_tipo(&Tipo::Decimal32).unwrap();
        assert!(emissor.codigo.windows(4).any(|janela| janela == [0xF2, 0x0F, 0x5A, 0xC0]));
        assert!(emissor.codigo.windows(4).any(|janela| janela == [0xF3, 0x0F, 0x5A, 0xC0]));
    }

    #[test]
    fn emite_aritmetica_decimal_sse2() {
        let expressao = ExpressaoIr::Binaria {
            esquerda: Box::new(ExpressaoIr::Normalizar {
                tipo: Tipo::Decimal64,
                valor: Box::new(ExpressaoIr::Constante(ValorIr::Decimal(1.5))),
            }),
            operador: OperadorIr::Mais,
            direita: Box::new(ExpressaoIr::Normalizar {
                tipo: Tipo::Decimal64,
                valor: Box::new(ExpressaoIr::Constante(ValorIr::Decimal(2.5))),
            }),
        };
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.expressao(&expressao).unwrap();
        assert!(emissor.codigo.windows(4).any(|janela| janela == [0xF2, 0x0F, 0x58, 0xC1]));
    }

    #[test]
    fn comparacao_decimal_respeita_nan() {
        let expressao = ExpressaoIr::Binaria {
            esquerda: Box::new(ExpressaoIr::Normalizar {
                tipo: Tipo::Decimal64,
                valor: Box::new(ExpressaoIr::Constante(ValorIr::Decimal(f64::NAN))),
            }),
            operador: OperadorIr::Igual,
            direita: Box::new(ExpressaoIr::Normalizar {
                tipo: Tipo::Decimal64,
                valor: Box::new(ExpressaoIr::Constante(ValorIr::Decimal(1.0))),
            }),
        };
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.expressao(&expressao).unwrap();
        assert!(emissor.codigo.windows(4).any(|janela| janela == [0x66, 0x0F, 0x2E, 0xC1]));
        assert!(emissor.codigo.windows(3).any(|janela| janela == [0x0F, 0x9B, 0xC1]));
    }

    #[test]
    fn emite_operacoes_aritmeticas_sem_sinal() {
        for (operador, bytes) in [
            (OperadorIr::Divide, &[0x48, 0x31, 0xD2, 0x48, 0xF7, 0xF1][..]),
            (OperadorIr::Menor, &[0x0F, 0x92, 0xC0][..]),
            (OperadorIr::MenorIgual, &[0x0F, 0x96, 0xC0][..]),
            (OperadorIr::Maior, &[0x0F, 0x97, 0xC0][..]),
            (OperadorIr::MaiorIgual, &[0x0F, 0x93, 0xC0][..]),
        ] {
            let expressao = ExpressaoIr::Binaria {
                esquerda: Box::new(ExpressaoIr::Normalizar {
                    tipo: Tipo::InteiroU64,
                    valor: Box::new(ExpressaoIr::Constante(ValorIr::InteiroSemSinal(9))),
                }),
                operador,
                direita: Box::new(ExpressaoIr::Normalizar {
                    tipo: Tipo::InteiroU64,
                    valor: Box::new(ExpressaoIr::Constante(ValorIr::InteiroSemSinal(3))),
                }),
            };
            let mut emissor = Emissor::novo(HashMap::new());
            emissor.expressao(&expressao).unwrap();
            assert!(emissor.codigo.windows(bytes.len()).any(|janela| janela == bytes), "{operador:?}");
        }
    }

    #[test]
    fn normaliza_inteiros_de_largura_reduzida_com_verificacao() {
        for (tipo, sequencia) in [
            (Tipo::Inteiro8, vec![0x48, 0xC1, 0xE0, 56, 0x48, 0xC1, 0xF8, 56]),
            (Tipo::Inteiro16, vec![0x48, 0xC1, 0xE0, 48, 0x48, 0xC1, 0xF8, 48]),
            (Tipo::Inteiro32, vec![0x48, 0xC1, 0xE0, 32, 0x48, 0xC1, 0xF8, 32]),
            (Tipo::InteiroU8, vec![0x48, 0xC1, 0xE8, 8]),
            (Tipo::InteiroU16, vec![0x48, 0xC1, 0xE8, 16]),
            (Tipo::InteiroU32, vec![0x48, 0xC1, 0xE8, 32]),
        ] {
            let mut emissor = Emissor::novo(HashMap::new());
            emissor.normalizar_tipo(&tipo).unwrap();
            assert!(emissor.codigo.windows(sequencia.len()).any(|janela| janela == sequencia));
            assert!(emissor.codigo.windows(2).any(|janela| janela == [0x0F, 0x84]));
            assert!(emissor.codigo.windows(2).any(|janela| janela == [0x0F, 0x05]));
        }
    }

    #[test]
    fn reserva_area_de_agregados_alinhada_sem_colidir_com_slots() {
        let mut emissor = Emissor::novo(HashMap::new());
        let primeiro = emissor.reservar_agregado(24, 8).unwrap();
        let segundo = emissor.reservar_agregado(32, 16).unwrap();
        assert_eq!(primeiro, -8224);
        assert_eq!(segundo % 16, 0);
        assert!(segundo < primeiro);
    }

    #[test]
    fn rejeita_agregado_que_excede_quadro_nativo() {
        let mut emissor = Emissor::novo(HashMap::new());
        assert!(emissor.reservar_agregado(u64::from(TAMANHO_QUADRO), 8).is_err());
        assert!(emissor.reservar_agregado(8, 32).is_err());
    }

    #[test]
    fn aceita_literal_inteiro128_em_dois_registros() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.expressao(&ExpressaoIr::Constante(ValorIr::InteiroSemSinal(u64::MAX as u128))).unwrap();
        assert_eq!(&emissor.codigo[..2], &[0x48, 0xB8]);
        assert_eq!(u64::from_le_bytes(emissor.codigo[2..10].try_into().unwrap()), u64::MAX);
        emissor.expressao(&ExpressaoIr::Constante(ValorIr::InteiroSemSinal(u64::MAX as u128 + 1))).unwrap();
        assert_eq!(u64::from_le_bytes(emissor.codigo[32..40].try_into().unwrap()), 1);
    }

    #[test]
    fn aceita_128_bits_e_decimais_sse2() {
        let mut emissor = Emissor::novo(HashMap::new());
        assert!(emissor.normalizar_tipo(&Tipo::Inteiro128).is_ok());
        assert!(emissor.normalizar_tipo(&Tipo::InteiroU128).is_ok());
        assert!(emissor.normalizar_tipo(&Tipo::Decimal32).is_ok());
        assert!(emissor.normalizar_tipo(&Tipo::Decimal64).is_ok());
    }

    #[test]
    fn codifica_instrucoes_basicas_com_bytes_x86_64_esperados() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.mov_rax(0x0102_0304_0506_0708);
        assert_eq!(&emissor.codigo, &[0x48, 0xB8, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
        emissor.codigo.clear();
        emissor.push_rax();
        emissor.pop_rax();
        assert_eq!(&emissor.codigo, &[0x50, 0x58]);
        assert_eq!(emissor.pilha, 0);
        emissor.codigo.clear();
        emissor.armazenar_rax(-8);
        assert_eq!(&emissor.codigo, &[0x48, 0x89, 0x85, 0xF8, 0xFF, 0xFF, 0xFF]);
        emissor.codigo.clear();
        emissor.carregar_rax(-8);
        assert_eq!(&emissor.codigo, &[0x48, 0x8B, 0x85, 0xF8, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn codifica_movimento_de_argumentos_para_rax() {
        let mut emissor = Emissor::novo(HashMap::new());
        for (registrador, esperado) in [
            (7, &[0x48, 0x89, 0xF8][..]),
            (6, &[0x48, 0x89, 0xF0][..]),
            (2, &[0x48, 0x89, 0xD0][..]),
            (1, &[0x48, 0x89, 0xC8][..]),
            (8, &[0x4C, 0x89, 0xC0][..]),
            (9, &[0x4C, 0x89, 0xC8][..]),
        ] {
            emissor.codigo.clear();
            emissor.mover_registrador_rax(registrador);
            assert_eq!(emissor.codigo.as_slice(), esperado);
        }
    }

    #[test]
    fn codifica_registradores_de_argumentos_system_v() {
        let mut emissor = Emissor::novo(HashMap::new());
        for (registrador, esperado) in [
            (7, &[0x48, 0x89, 0xC7][..]),
            (6, &[0x48, 0x89, 0xC6][..]),
            (2, &[0x48, 0x89, 0xC2][..]),
            (1, &[0x48, 0x89, 0xC1][..]),
            (8, &[0x49, 0x89, 0xC0][..]),
            (9, &[0x49, 0x89, 0xC1][..]),
        ] {
            emissor.codigo.clear();
            emissor.mover_rax_registrador(registrador);
            assert_eq!(emissor.codigo.as_slice(), esperado);
        }
        emissor.codigo.clear();
        emissor.armazenar_registrador(8, -8);
        assert_eq!(emissor.codigo.as_slice(), &[0x4C, 0x89, 0x85, 0xF8, 0xFF, 0xFF, 0xFF]);
        emissor.codigo.clear();
        emissor.armazenar_registrador(9, -16);
        assert_eq!(emissor.codigo.as_slice(), &[0x4C, 0x89, 0x8D, 0xF0, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn codifica_operacoes_aritmeticas_e_comparacao_signed() {
        let casos = [
            (OperadorIr::Mais, &[0x48, 0x01, 0xC8][..]),
            (OperadorIr::Menos, &[0x48, 0x29, 0xC8][..]),
            (OperadorIr::Vezes, &[0x48, 0x0F, 0xAF, 0xC1][..]),
            (OperadorIr::Divide, &[0x48, 0x99, 0x48, 0xF7, 0xF9][..]),
            (OperadorIr::Resto, &[0x48, 0x99, 0x48, 0xF7, 0xF9][..]),
            (OperadorIr::Menor, &[0x48, 0x39, 0xC8, 0x0F, 0x9C, 0xC0][..]),
            (OperadorIr::MenorIgual, &[0x48, 0x39, 0xC8, 0x0F, 0x9E, 0xC0][..]),
            (OperadorIr::Maior, &[0x48, 0x39, 0xC8, 0x0F, 0x9F, 0xC0][..]),
            (OperadorIr::MaiorIgual, &[0x48, 0x39, 0xC8, 0x0F, 0x9D, 0xC0][..]),
            (OperadorIr::Igual, &[0x48, 0x39, 0xC8, 0x0F, 0x94, 0xC0][..]),
            (OperadorIr::Diferente, &[0x48, 0x39, 0xC8, 0x0F, 0x95, 0xC0][..]),
        ];
        for (operador, bytes_esperados) in casos {
            let mut emissor = Emissor::novo(HashMap::new());
            let expressao = ExpressaoIr::Binaria {
                esquerda: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(7))),
                operador,
                direita: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(3))),
            };
            emissor.expressao(&expressao).unwrap();
            assert!(
                emissor.codigo.windows(bytes_esperados.len()).any(|janela| janela == bytes_esperados),
                "codificação ausente para {operador:?}"
            );
        }
    }

    #[test]
    fn emite_multiplicacao_de_inteiros_de_128_bits() {
        for tipo in [Tipo::Inteiro128, Tipo::InteiroU128] {
            let expressao = ExpressaoIr::Binaria {
                esquerda: Box::new(ExpressaoIr::Normalizar {
                    tipo: tipo.clone(),
                    valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(7))),
                }),
                operador: OperadorIr::Vezes,
                direita: Box::new(ExpressaoIr::Normalizar {
                    tipo,
                    valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(9))),
                }),
            };
            let mut emissor = Emissor::novo(HashMap::new());
            emissor.expressao(&expressao).unwrap();
            assert!(emissor.codigo.windows(4).any(|janela| janela == [0x4C, 0x01, 0xC8, 0x4C]));
            assert!(emissor.codigo.windows(4).any(|janela| janela == [0x4C, 0x11, 0xC2, 0x0F]));
        }
    }

    #[test]
    fn emite_divisao_e_resto_de_inteiros_de_128_bits() {
        for (tipo, operador) in [
            (Tipo::Inteiro128, OperadorIr::Divide),
            (Tipo::Inteiro128, OperadorIr::Resto),
            (Tipo::InteiroU128, OperadorIr::Divide),
            (Tipo::InteiroU128, OperadorIr::Resto),
        ] {
            let expressao = ExpressaoIr::Binaria {
                esquerda: Box::new(ExpressaoIr::Normalizar {
                    tipo: tipo.clone(),
                    valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(27))),
                }),
                operador,
                direita: Box::new(ExpressaoIr::Normalizar {
                    tipo,
                    valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(5))),
                }),
            };
            let mut emissor = Emissor::novo(HashMap::new());
            emissor.expressao(&expressao).unwrap();
            assert!(emissor.codigo.windows(4).any(|janela| janela == [0x49, 0xD1, 0xE1, 0x49]));
            assert!(emissor.codigo.windows(4).any(|janela| janela == [0x48, 0x29, 0xC8, 0x48]));
            assert!(emissor.erro_salto.is_none());
        }
    }

    #[test]
    fn emite_conversoes_nativas_entre_inteiro128_e_decimal() {
        for (destino, origem) in [
            (Tipo::Decimal64, Tipo::Inteiro128),
            (Tipo::Decimal32, Tipo::InteiroU128),
        ] {
            let mut para_decimal = Emissor::novo(HashMap::new());
            para_decimal.converter_tipo_explicito(&destino, &origem).unwrap();
            assert!(para_decimal.codigo.windows(4).any(|janela| janela == [0xF2, 0x0F, 0x59, 0xD3]));
            assert!(para_decimal.codigo.windows(4).any(|janela| janela == [0xF2, 0x0F, 0x58, 0xC2]));
            assert!(para_decimal.erro_salto.is_none());
        }

        for destino in [Tipo::Inteiro128, Tipo::InteiroU128] {
            let mut para_inteiro = Emissor::novo(HashMap::new());
            para_inteiro.converter_decimal_para_inteiro(&destino).unwrap();
            assert!(para_inteiro.codigo.windows(4).any(|janela| janela == [0xF2, 0x0F, 0x5E, 0xC1]));
            assert!(para_inteiro.codigo.windows(4).any(|janela| janela == [0xF2, 0x0F, 0x5C, 0xD3]));
            assert!(para_inteiro.erro_salto.is_none());
        }
    }

    #[test]
    fn codifica_saltos_condicionais_e_incondicionais() {
        let mut emissor = Emissor::novo(HashMap::new());
        let condicional = emissor.emitir_jcc(0x84);
        assert_eq!(&emissor.codigo, &[0x0F, 0x84, 0, 0, 0, 0]);
        let destino_condicional = emissor.codigo.len();
        emissor.aplicar_salto(condicional, destino_condicional);
        assert_eq!(&emissor.codigo, &[0x0F, 0x84, 0, 0, 0, 0]);
        emissor.codigo.clear();
        let incondicional = emissor.emitir_jmp();
        assert_eq!(&emissor.codigo, &[0xE9, 0, 0, 0, 0]);
        let destino_incondicional = emissor.codigo.len();
        emissor.aplicar_salto(incondicional, destino_incondicional);
        assert_eq!(&emissor.codigo, &[0xE9, 0, 0, 0, 0]);
    }

    #[test]
    fn rejeita_deslocamentos_relativos_fora_de_i32() {
        assert!(calcular_relativo32(4, i128::from(i32::MAX) + 5).is_err());
        assert!(calcular_relativo32(i128::from(i32::MAX) + 5, 0).is_err());
        assert_eq!(calcular_relativo32(4, i128::from(i32::MAX) + 4).unwrap(), i32::MAX);
        assert_eq!(calcular_relativo32(i128::from(i32::MAX) + 5, 4).unwrap(), i32::MIN);
    }

    #[test]
    fn rejeita_destino_de_salto_fora_do_codigo_gerado() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.codigo.extend_from_slice(&[0xE9, 0, 0, 0, 0]);
        emissor.aplicar_salto(1, 6);
        let erro = emissor.finalizar().unwrap_err();
        assert!(erro.contains("fora do código gerado"));
    }

    #[test]
    fn aceita_literal_inteiro_minimo_em_negacao_unaria() {
        let mut programa = programa_com_retorno(0);
        programa.funcoes.get_mut("principal").unwrap().corpo = vec![
            InstrucaoIr::Devolver(Some(ExpressaoIr::Unaria {
                operador: OperadorIr::Menos,
                valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(i64::MAX as i128 + 1))),
            }))
        ];
        assert!(gerar_executavel(&programa).is_ok());
    }
}

}

pub use implementacao::gerar_executavel;

/// Retorna os identificadores de alvo que possuem backend implementado.
#[must_use]
pub const fn alvos_suportados() -> &'static [&'static str] {
    &["linux-x86_64"]
}

/// Valida um alvo sem recorrer a compilação cruzada ou ferramentas externas.
pub fn validar_alvo(alvo: &str) -> Result<(), String> {
    match alvo {
        "linux-x86_64" => Ok(()),
        "linux-aarch64" | "windows-x86_64" | "macos-x86_64" | "macos-aarch64" | "wasm32" => {
            Err(format!(
                "O alvo '{alvo}' ainda não possui backend implementado. Alvos disponíveis: {}.",
                alvos_suportados().join(", ")
            ))
        }
        _ => Err(format!(
            "Alvo desconhecido: '{alvo}'. Alvos disponíveis: {}.",
            alvos_suportados().join(", ")
        )),
    }
}

#[cfg(test)]
mod testes_alvos {
    use super::*;

    #[test]
    fn aceita_somente_o_alvo_nativo_implementado() {
        assert!(validar_alvo("linux-x86_64").is_ok());
        assert_eq!(alvos_suportados(), &["linux-x86_64"]);
    }

    #[test]
    fn informa_backends_ainda_indisponiveis() {
        for alvo in [
            "linux-aarch64",
            "windows-x86_64",
            "macos-x86_64",
            "macos-aarch64",
            "wasm32",
        ] {
            let erro = validar_alvo(alvo).expect_err("alvo não implementado");
            assert!(erro.contains("ainda não possui backend implementado"));
        }
    }

    #[test]
    fn rejeita_identificador_de_alvo_desconhecido() {
        assert!(
            validar_alvo("dos-16")
                .unwrap_err()
                .contains("Alvo desconhecido")
        );
    }
}
