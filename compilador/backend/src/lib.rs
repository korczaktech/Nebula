//! Backend nativo da Nebula: geração direta de executáveis ELF x86-64.
#![forbid(unsafe_code)]

#[rustfmt::skip]
mod implementacao {
use nebula_ir::{ExpressaoIr, FuncaoIr, InstrucaoIr, OperadorIr, ProgramaIr, ValorIr};
use nebula_semantica::Tipo;
use std::collections::HashMap;

const TAMANHO_CABECALHO: usize = 4096;
const TAMANHO_QUADRO: u32 = 4096;
const LIMITE_VARIAVEIS: usize = 512;

/// Compila a IR em um executável ELF64 estático para Linux x86-64.
///
/// O arquivo é construído diretamente em Rust. Não gera C/C++, assembly textual,
/// objeto intermediário nem invoca linker ou compilador externo.
pub fn gerar_executavel(programa: &ProgramaIr) -> Result<Vec<u8>, String> {
    if !programa.modulos.is_empty() {
        return Err("O backend x86-64 ainda não oferece ligação de módulos.".into());
    }

    let mut globais = HashMap::new();
    for instrucao in &programa.instrucoes {
        if let InstrucaoIr::Declarar { nome, tipo, .. } = instrucao {
            validar_tipo(tipo)?;
            let deslocamento = globais.len() * 8;
            if globais.insert(nome.clone(), deslocamento).is_some() {
                return Err(format!("Variável global '{nome}' declarada mais de uma vez."));
            }
        }
    }
    let mut emissor = Emissor::novo(globais);
    emissor.iniciar_programa(programa)?;
    let (codigo, tamanho_dados) = emissor.finalizar()?;
    gerar_elf(&codigo, tamanho_dados)
}

struct Emissor {
    codigo: Vec<u8>,
    funcoes: HashMap<String, usize>,
    chamadas: Vec<(usize, String)>,
    escopos: Vec<HashMap<String, i32>>,
    proximo_slot: usize,
    pilha: usize,
    lacos: Vec<Laco>,
    globais: HashMap<String, usize>,
    referencias_globais: Vec<(usize, String)>,
    funcao_atual: String,
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
            escopos: Vec::new(),
            proximo_slot: 0,
            pilha: 0,
            lacos: Vec::new(),
            globais,
            referencias_globais: Vec::new(),
            funcao_atual: String::new(),
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

        // O ponto de entrada chama as rotinas geradas e encerra pelo syscall Linux.
        let mut chamadas_inicio = Vec::new();
        if !programa.instrucoes.is_empty() {
            self.codigo.push(0xE8);
            chamadas_inicio.push((self.codigo.len(), "__nebula_inicio".to_owned()));
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
                nome: "__nebula_inicio".into(),
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
        validar_tipo(&funcao.retorno)?;
        self.funcoes.insert(funcao.nome.clone(), self.codigo.len());
        self.funcao_atual = funcao.nome.clone();
        self.escopos.clear();
        self.escopos.push(HashMap::new());
        self.proximo_slot = 0;
        self.pilha = 0;
        self.lacos.clear();

        // Prólogo e quadro fixo: mantém alinhamento ABI e evita alocação dinâmica no compilador.
        self.codigo.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5, 0x48, 0x81, 0xEC]);
        self.codigo.extend_from_slice(&TAMANHO_QUADRO.to_le_bytes());

        if funcao.parametros.len() > 6 {
            return Err(format!("A função '{}' excede seis parâmetros no ABI x86-64 inicial.", funcao.nome));
        }
        let registradores = [7u8, 6, 2, 1, 8, 9]; // rdi, rsi, rdx, rcx, r8, r9
        for (indice, (nome, tipo)) in funcao.parametros.iter().enumerate() {
            validar_tipo(tipo)?;
            let deslocamento = self.declarar(nome)?;
            self.armazenar_registrador(registradores[indice], deslocamento);
        }

        // Parâmetros e declarações do corpo compartilham o escopo da função.
        for instrucao in &funcao.corpo {
            self.instrucao(instrucao)?;
        }
        self.codigo.extend_from_slice(&[0x31, 0xC0, 0xC9, 0xC3]); // retorno implícito zero
        self.escopos.clear();
        Ok(())
    }

    fn bloco(&mut self, instrucoes: &[InstrucaoIr]) -> Result<(), String> {
        self.escopos.push(HashMap::new());
        for instrucao in instrucoes {
            self.instrucao(instrucao)?;
        }
        self.escopos.pop();
        Ok(())
    }

    fn instrucao(&mut self, instrucao: &InstrucaoIr) -> Result<(), String> {
        match instrucao {
            InstrucaoIr::Declarar { nome, tipo, valor, .. } => {
                validar_tipo(tipo)?;
                self.expressao(valor)?;
                if self.funcao_atual == "__nebula_inicio"
                    && self.escopos.len() == 1
                    && self.globais.contains_key(nome)
                {
                    self.armazenar_global(nome);
                } else {
                    let deslocamento = self.declarar(nome)?;
                    self.armazenar_rax(deslocamento);
                }
            }
            InstrucaoIr::Expressao(expressao) => self.expressao(expressao)?,
            InstrucaoIr::Devolver(valor) => {
                if let Some(valor) = valor { self.expressao(valor)?; }
                else { self.codigo.extend_from_slice(&[0x31, 0xC0]); }
                self.codigo.extend_from_slice(&[0xC9, 0xC3]);
            }
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
            InstrucaoIr::Para { inicial, condicao, atualizacao, corpo } => {
                self.escopos.push(HashMap::new());
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
                self.escopos.pop();
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
            ExpressaoIr::Constante(ValorIr::Inteiro(valor)) => {
                let valor = i64::try_from(*valor).map_err(|_| "Literal inteiro fora do intervalo de 64 bits.")?;
                self.mov_rax(valor);
            }
            ExpressaoIr::Constante(ValorIr::Logico(valor)) => self.mov_rax(i64::from(*valor)),
            ExpressaoIr::Constante(ValorIr::Caractere(valor)) if valor.is_ascii() => self.mov_rax(*valor as i64),
            ExpressaoIr::Constante(ValorIr::Decimal(_)
                | ValorIr::Texto(_)
                | ValorIr::Lista(_)
                | ValorIr::Intervalo(_, _)
                | ValorIr::IntervaloDecimal(_, _)
                | ValorIr::Nulo
                | ValorIr::Vazio) => return Err("Literal ainda não suportado pelo backend nativo sem runtime.".into()),
            ExpressaoIr::Constante(ValorIr::Caractere(_)) => return Err("Caractere não ASCII ainda não é suportado pelo alvo inicial.".into()),
            ExpressaoIr::Variavel(nome) => {
                if let Some(deslocamento) = self.buscar_local(nome) {
                    self.carregar_rax(deslocamento);
                } else if self.globais.contains_key(nome) {
                    self.carregar_global(nome);
                } else {
                    return Err(format!("Variável '{nome}' não encontrada no backend."));
                }
            }
            ExpressaoIr::Lista(_) | ExpressaoIr::Indexacao { .. } => {
                return Err("Listas e indexação aguardam a implementação do suporte de memória/runtime.".into());
            }
            ExpressaoIr::Unaria { operador, valor } => {
                self.expressao(valor)?;
                match operador {
                    OperadorIr::Menos => self.codigo.extend_from_slice(&[0x48, 0xF7, 0xD8]),
                    OperadorIr::Nao | OperadorIr::Negacao => {
                        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0, 0x0F, 0x94, 0xC0, 0x48, 0x0F, 0xB6, 0xC0]);
                    }
                    OperadorIr::Mais => {}
                    _ => return Err("Operador unário não suportado.".into()),
                }
            }
            ExpressaoIr::Binaria { esquerda, operador, direita } => {
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
                match operador {
                    OperadorIr::Mais => self.codigo.extend_from_slice(&[0x48, 0x01, 0xC8]),
                    OperadorIr::Menos => self.codigo.extend_from_slice(&[0x48, 0x29, 0xC8]),
                    OperadorIr::Vezes => self.codigo.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]),
                    OperadorIr::Divide | OperadorIr::Resto => {
                        // A divisão por zero termina com código estável, em vez de SIGFPE.
                        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]);
                        let divisor_valido = self.emitir_jcc(0x85);
                        self.encerrar_com_erro(2);
                        self.aplicar_salto(divisor_valido, self.codigo.len());

                        // INT64_MIN / -1 não é representável em signed division.
                        // Rejeita a operação com o mesmo código estável da divisão por zero.
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
                        if *operador == OperadorIr::Resto { self.codigo.extend_from_slice(&[0x48, 0x89, 0xD0]); }
                    }
                    OperadorIr::Potencia => {
                        // Exponenciação inteira por quadratura: O(log expoente), sem runtime.
                        // Expoente negativo é rejeitado com código de saída 2.
                        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]);
                        let expoente_nao_negativo = self.emitir_jcc(0x89); // JNS
                        self.encerrar_com_erro(2);
                        self.aplicar_salto(expoente_nao_negativo, self.codigo.len());
                        self.codigo.extend_from_slice(&[0x48, 0x89, 0xC2]); // rdx = base
                        self.mov_rax(1); // acumulador
                        let inicio = self.codigo.len();
                        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC9]);
                        let fim = self.emitir_jcc(0x84);
                        self.codigo.extend_from_slice(&[0xF6, 0xC1, 0x01]);
                        let multiplicar = self.emitir_jcc(0x84);
                        self.codigo.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]);
                        self.aplicar_salto(multiplicar, self.codigo.len());
                        self.codigo.extend_from_slice(&[0x48, 0xD1, 0xE9]); // expoente >>= 1
                        let terminar = self.emitir_jcc(0x84);
                        self.codigo.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xD2]); // base *= base
                        self.emitir_salto_para(inicio);
                        let destino_fim = self.codigo.len();
                        self.aplicar_salto(fim, destino_fim);
                        self.aplicar_salto(terminar, destino_fim);
                    }
                    OperadorIr::Igual | OperadorIr::Diferente | OperadorIr::Menor | OperadorIr::MenorIgual | OperadorIr::Maior | OperadorIr::MaiorIgual => {
                        self.codigo.extend_from_slice(&[0x48, 0x39, 0xC8]);
                        let set = match operador {
                            OperadorIr::Igual => 0x94, OperadorIr::Diferente => 0x95,
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
            ExpressaoIr::Atribuicao { nome, valor } => {
                self.expressao(valor)?;
                if let Some(deslocamento) = self.buscar_local(nome) {
                    self.armazenar_rax(deslocamento);
                } else if self.globais.contains_key(nome) {
                    self.armazenar_global(nome);
                } else {
                    return Err(format!("Variável '{nome}' não encontrada no backend."));
                }
            }
            ExpressaoIr::Chamada { nome, argumentos } => {
                if nome == "exibir" || nome == "ler_texto" {
                    return Err(format!("A função '{nome}' pertence à biblioteca padrão/runtime, ainda não incluída nesta etapa."));
                }
                if argumentos.len() > 6 {
                    return Err("O ABI x86-64 inicial aceita até seis argumentos.".into());
                }
                for argumento in argumentos {
                    self.expressao(argumento)?;
                    self.push_rax();
                }
                let registradores = [7u8, 6, 2, 1, 8, 9];
                for indice in (0..argumentos.len()).rev() {
                    self.pop_rax(); // recupera argumento
                    self.mover_rax_registrador(registradores[indice]);
                }
                if self.pilha % 2 == 1 { self.codigo.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]); }
                self.codigo.push(0xE8);
                let posicao = self.codigo.len();
                self.codigo.extend_from_slice(&[0, 0, 0, 0]);
                self.chamadas.push((posicao, nome.clone()));
                if self.pilha % 2 == 1 { self.codigo.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]); }
            }
            ExpressaoIr::ChamadaQualificada { .. } => return Err("Chamadas de módulos ainda não são suportadas pelo backend nativo.".into()),
        }
        Ok(())
    }

    fn declarar(&mut self, nome: &str) -> Result<i32, String> {
        if self.escopos.last().is_some_and(|escopo| escopo.contains_key(nome)) {
            return Err(format!("Identificador '{nome}' declarado mais de uma vez no mesmo escopo."));
        }
        if self.proximo_slot >= LIMITE_VARIAVEIS {
            return Err("A função excedeu o limite inicial de 512 posições locais.".into());
        }
        self.proximo_slot += 1;
        let deslocamento = -((self.proximo_slot * 8) as i32);
        self.escopos.last_mut().expect("escopo ativo").insert(nome.to_owned(), deslocamento);
        Ok(deslocamento)
    }

    fn buscar_local(&self, nome: &str) -> Option<i32> {
        self.escopos.iter().rev().find_map(|escopo| escopo.get(nome).copied())
    }

    fn carregar_global(&mut self, nome: &str) {
        self.codigo.extend_from_slice(&[0x48, 0x8B, 0x05]); // mov rax, [rip + disp32]
        let posicao = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 4]);
        self.referencias_globais.push((posicao, nome.to_owned()));
    }

    fn armazenar_global(&mut self, nome: &str) {
        self.codigo.extend_from_slice(&[0x48, 0x89, 0x05]); // mov [rip + disp32], rax
        let posicao = self.codigo.len();
        self.codigo.extend_from_slice(&[0; 4]);
        self.referencias_globais.push((posicao, nome.to_owned()));
    }

    fn encerrar_com_erro(&mut self, codigo: u8) {
        // Linux x86-64: exit(codigo), syscall 60.
        self.codigo.extend_from_slice(&[0xBF, codigo, 0, 0, 0, 0xB8, 0x3C, 0, 0, 0, 0x0F, 0x05]);
    }

    fn normalizar_logico(&mut self) {
        self.codigo.extend_from_slice(&[0x48, 0x85, 0xC0, 0x0F, 0x95, 0xC0, 0x48, 0x0F, 0xB6, 0xC0]);
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

    fn carregar_rax(&mut self, deslocamento: i32) {
        self.codigo.extend_from_slice(&[0x48, 0x8B, 0x85]);
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
    }

    fn armazenar_registrador(&mut self, registrador: u8, deslocamento: i32) {
        self.codigo.push(if registrador >= 8 { 0x4C } else { 0x48 });
        self.codigo.extend_from_slice(&[0x89, 0x85 | ((registrador & 7) << 3)]);
        self.codigo.extend_from_slice(&deslocamento.to_le_bytes());
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
        let relativo = destino as i64 - (posicao + 4) as i64;
        self.codigo[posicao..posicao + 4].copy_from_slice(&(relativo as i32).to_le_bytes());
    }

    fn finalizar(mut self) -> Result<(Vec<u8>, usize), String> {
        for (posicao, nome) in self.chamadas {
            let destino = self.funcoes.get(&nome)
                .ok_or_else(|| format!("Função '{}' não foi encontrada durante a geração nativa.", nome))?;
            let relativo = *destino as i64 - (posicao + 4) as i64;
            let relativo = i32::try_from(relativo).map_err(|_| "Chamada fora do alcance relativo x86-64.")?;
            self.codigo[posicao..posicao + 4].copy_from_slice(&relativo.to_le_bytes());
        }

        let fim_texto = TAMANHO_CABECALHO.checked_add(self.codigo.len())
            .ok_or("Tamanho do segmento de código excedeu o limite do alvo.")?;
        let deslocamento_dados = alinhar_pagina(fim_texto)?;
        let endereco_dados = 0x400000u64 + deslocamento_dados as u64;
        for (posicao, nome) in self.referencias_globais {
            let deslocamento = *self.globais.get(&nome)
                .ok_or_else(|| format!("Variável global '{nome}' não foi alocada."))?;
            let destino = endereco_dados + deslocamento as u64;
            let origem = 0x401000u64 + (posicao + 4) as u64;
            let relativo = i32::try_from(destino as i64 - origem as i64)
                .map_err(|_| "Variável global fora do alcance RIP-relative x86-64.")?;
            self.codigo[posicao..posicao + 4].copy_from_slice(&relativo.to_le_bytes());
        }
        Ok((self.codigo, self.globais.len() * 8))
    }
}

fn validar_tipo(tipo: &Tipo) -> Result<(), String> {
    match tipo {
        Tipo::Vazio | Tipo::Logico | Tipo::Inteiro | Tipo::Inteiro64 => Ok(()),
        _ => Err(format!("Tipo '{}' ainda não é suportado pelo backend sem runtime.", tipo.nome())),
    }
}

fn alinhar_pagina(valor: usize) -> Result<usize, String> {
    valor.checked_add(0xFFF)
        .map(|valor| valor & !0xFFF)
        .ok_or_else(|| "Tamanho do executável excedeu o limite do alvo ELF64.".into())
}

fn gerar_elf(codigo: &[u8], tamanho_dados: usize) -> Result<Vec<u8>, String> {
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
        assert_eq!(u16::from_le_bytes([binario[56], binario[57]]), 1);
        assert_eq!(u32::from_le_bytes(binario[64..68].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(binario[68..72].try_into().unwrap()), 5);
        assert_eq!(u64::from_le_bytes(binario[72..80].try_into().unwrap()), 0);
        assert_eq!(u64::from_le_bytes(binario[80..88].try_into().unwrap()), 0x400000);
        assert_eq!(u64::from_le_bytes(binario[96..104].try_into().unwrap()), binario.len() as u64);
        assert_eq!(u64::from_le_bytes(binario[104..112].try_into().unwrap()), binario.len() as u64);
        assert_eq!(u64::from_le_bytes(binario[112..120].try_into().unwrap()), 0x1000);
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
        assert_eq!(u64::from_le_bytes(binario[152..160].try_into().unwrap()), 8);
        assert_eq!(u64::from_le_bytes(binario[160..168].try_into().unwrap()), 8);
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
    fn rejeita_funcao_com_mais_de_seis_parametros() {
        let mut programa = programa_com_retorno(0);
        programa.funcoes.insert("excesso".into(), FuncaoIr {
            nome: "excesso".into(),
            parametros: (0..7).map(|indice| (format!("p{indice}"), Tipo::Inteiro)).collect(),
            retorno: Tipo::Inteiro,
            corpo: Vec::new(),
        });
        assert!(gerar_executavel(&programa).unwrap_err().contains("seis parâmetros"));
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
    fn parametros_e_corpo_compartilham_escopo_da_funcao() {
        let mut programa = programa_com_retorno(0);
        programa.funcoes.get_mut("principal").unwrap().parametros = vec![("valor".into(), Tipo::Inteiro)];
        programa.funcoes.get_mut("principal").unwrap().corpo = vec![
            InstrucaoIr::Declarar {
                nome: "valor".into(),
                tipo: Tipo::Inteiro,
                valor: ExpressaoIr::Constante(ValorIr::Inteiro(1)),
                constante: false,
            }
        ];
        assert!(gerar_executavel(&programa).unwrap_err().contains("mesmo escopo"));
    }

    #[test]
    fn rejeita_declaracao_duplicada_no_mesmo_escopo() {
        let mut emissor = Emissor::novo(HashMap::new());
        emissor.escopos.push(HashMap::new());
        assert!(emissor.declarar("valor").is_ok());
        assert!(emissor.declarar("valor").is_err());
    }

    #[test]
    fn rejeita_recurso_que_exige_runtime_ainda_nao_implementado() {
        let mut programa = programa_com_retorno(0);
        programa.funcoes.get_mut("principal").unwrap().corpo = vec![
            InstrucaoIr::Expressao(ExpressaoIr::Chamada { nome: "exibir".into(), argumentos: vec![ExpressaoIr::Constante(ValorIr::Inteiro(1))] })
        ];
        assert!(gerar_executavel(&programa).unwrap_err().contains("runtime"));
    }

    #[test]
    fn rejeita_tipo_nao_suportado_sem_fingir_implementacao() {
        assert!(validar_tipo(&Tipo::Texto).is_err());
        assert!(validar_tipo(&Tipo::Inteiro).is_ok());
        assert!(validar_tipo(&Tipo::Inteiro64).is_ok());
        assert!(validar_tipo(&Tipo::Inteiro8).is_err());
        assert!(validar_tipo(&Tipo::Byte).is_err());
    }
}

}

pub use implementacao::gerar_executavel;
