//! Análise explícita de propriedade, empréstimos e destruição lexical.
//!
//! Este módulo fornece o núcleo de fluxo de propriedade para a análise semântica.
//! Ele não acessa memória diretamente: opera sobre lugares abstratos e deve ser
//! alimentado pelo lowering/CFG do compilador.

use std::collections::{HashMap, HashSet, VecDeque};

/// Projeção de um lugar: campo, índice conhecido ou índice dinâmico.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProjecaoLugar {
    Campo(String),
    IndiceConstante(u64),
    IndiceDinamico,
}

/// Lugar abstrato identificando uma variável ou parte dela.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lugar {
    pub raiz: String,
    pub projecoes: Vec<ProjecaoLugar>,
}

impl Lugar {
    /// Cria um lugar que representa uma variável inteira.
    #[must_use]
    pub fn novo(raiz: impl Into<String>) -> Self {
        Self { raiz: raiz.into(), projecoes: Vec::new() }
    }

    /// Acrescenta uma projeção de campo.
    #[must_use]
    pub fn campo(mut self, nome: impl Into<String>) -> Self {
        self.projecoes.push(ProjecaoLugar::Campo(nome.into()));
        self
    }

    /// Acrescenta um índice constante.
    #[must_use]
    pub fn indice(mut self, indice: u64) -> Self {
        self.projecoes.push(ProjecaoLugar::IndiceConstante(indice));
        self
    }

    /// Acrescenta um índice dinâmico, conservadoramente sobreposto a qualquer índice.
    #[must_use]
    pub fn indice_dinamico(mut self) -> Self {
        self.projecoes.push(ProjecaoLugar::IndiceDinamico);
        self
    }

    /// Informa se dois lugares podem designar a mesma memória.
    #[must_use]
    pub fn sobrepoe(&self, outro: &Self) -> bool {
        if self.raiz != outro.raiz { return false; }
        for (a, b) in self.projecoes.iter().zip(&outro.projecoes) {
            if a == b { continue; }
            return match (a, b) {
                (ProjecaoLugar::IndiceDinamico, _)
                | (_, ProjecaoLugar::IndiceDinamico) => true,
                (ProjecaoLugar::Campo(_), ProjecaoLugar::Campo(_))
                | (ProjecaoLugar::IndiceConstante(_), ProjecaoLugar::IndiceConstante(_)) => false,
                _ => true,
            };
        }
        // Um lugar é prefixo do outro: o agregado inteiro contém a projeção.
        true
    }
}

/// Tipo de empréstimo estático.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoEmprestimo {
    Compartilhado,
    Exclusivo,
}

/// Contextos que autorizam operações inseguras específicas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextoInseguro {
    /// Bloco de código seguro que bloqueia contextos inseguros externos.
    Seguro,
    Critico,
    Privilegio,
    Montagem,
}

/// Operações que não podem ser executadas implicitamente em código seguro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperacaoInsegura {
    DesreferenciarPonteiroBruto,
    CriarReferenciaDeEndereco,
    AcessarMemoria,
    ReinterpretarRepresentacao,
    ChamarExternoNaoVerificado,
    Montagem,
    OperacaoPrivilegiada,
    LiberarAlocacao,
    AcessarMemoriaMapeada,
}

/// Provas exigidas pelo verificador antes de autorizar uma operação insegura.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProvasInseguras {
    pub endereco_valido: bool,
    pub alinhamento_valido: bool,
    pub inicializacao_comprovada: bool,
    pub aliasing_controlado: bool,
    pub limites_comprovados: bool,
    pub abi_validada: bool,
    pub alvo_compativel: bool,
    pub autorizacao_sistema: bool,
}

/// Erros produzidos pelo verificador de propriedade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroPropriedade {
    VariavelDuplicada(String),
    VariavelNaoDeclarada(String),
    UsoAposMovimento(Lugar),
    MovimentoDuranteEmprestimo(Lugar),
    EscritaDuranteEmprestimo(Lugar),
    ConflitoEmprestimo { lugar: Lugar, existente: TipoEmprestimo, solicitado: TipoEmprestimo },
    EmprestimoDuplicado(u64),
    IdentificadoresEmprestimoEsgotados,
    EmprestimoInexistente(u64),
    EmprestimoVivoAoDestruir(Lugar),
    ReferenciaEscapaVidaUtil { lugar: Lugar, escopo_proprietario: usize, escopo_destino: usize },
    EscopoGlobal,
    OperacaoInseguraForaDeBloco,
    BlocoFluxoInexistente(usize),
    ProfundidadesEscopoDivergentes { atual: usize, alternativo: usize },
    ContextosInsegurosDivergentes,
    BlocoInseguroNaoAberto,
    ContextoInseguroIncompativel { operacao: OperacaoInsegura, contexto: Option<ContextoInseguro> },
    ObrigacaoInseguraNaoComprovada { operacao: OperacaoInsegura, obrigacao: &'static str },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct VariavelPropriedade {
    escopo: usize,
    ordem: u64,
    movidos: HashSet<Lugar>,
}

/// Região lexical inferida de uma referência.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VidaUtil {
    /// Escopo no qual o valor proprietário foi inicializado.
    pub escopo_proprietario: usize,
    /// Escopo no qual a referência foi criada.
    pub escopo_referencia: usize,
    /// Indica que a origem é uma referência recebida como parâmetro.
    pub parametro_origem: Option<usize>,
    /// Identificador do empréstimo que sustenta a referência, quando criado localmente.
    pub emprestimo: Option<u64>,
}

impl VidaUtil {
    /// Uma referência pode ser armazenada no destino somente se o proprietário
    /// continuar vivo durante todo o escopo de destino.
    #[must_use]
    pub fn pode_ser_armazenada_em(self, escopo_destino: usize) -> bool {
        self.escopo_proprietario <= escopo_destino
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EmprestimoAtivo {
    lugar: Lugar,
    tipo: TipoEmprestimo,
    vida_util: VidaUtil,
}

/// Ação de destruição determinística emitida ao sair de um escopo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcaoDestruicao {
    /// Lugar que ainda pertence ao escopo e deve ser destruído.
    pub lugar: Lugar,
    /// Projeções já movidas que não podem ser destruídas novamente.
    pub partes_movidas: Vec<Lugar>,
}

/// Estado do verificador para um fluxo de controle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalisadorPropriedade {
    escopo: usize,
    proximo_ordem: u64,
    variaveis: HashMap<String, VariavelPropriedade>,
    emprestimos: HashMap<u64, EmprestimoAtivo>,
    contextos_inseguros: Vec<ContextoInseguro>,
}

impl Default for AnalisadorPropriedade {
    fn default() -> Self { Self::novo() }
}

impl AnalisadorPropriedade {
    /// Cria um estado com escopo global vazio.
    #[must_use]
    pub fn novo() -> Self {
        Self {
            escopo: 0,
            proximo_ordem: 0,
            variaveis: HashMap::new(),
            emprestimos: HashMap::new(),
            contextos_inseguros: Vec::new(),
        }
    }

    /// Profundidade lexical atual.
    #[must_use]
    pub fn profundidade_escopo(&self) -> usize { self.escopo }

    /// Declara uma variável inicializada no escopo atual.
    pub fn declarar(&mut self, nome: impl Into<String>) -> Result<Lugar, ErroPropriedade> {
        let nome = nome.into();
        if self.variaveis.contains_key(&nome) {
            return Err(ErroPropriedade::VariavelDuplicada(nome));
        }
        let ordem = self.proximo_ordem;
        self.proximo_ordem = self.proximo_ordem.saturating_add(1);
        self.variaveis.insert(nome.clone(), VariavelPropriedade {
            escopo: self.escopo, ordem, movidos: HashSet::new(),
        });
        Ok(Lugar::novo(nome))
    }

    /// Verifica se um lugar ainda está integralmente inicializado.
    pub fn ler(&self, lugar: &Lugar) -> Result<(), ErroPropriedade> {
        let variavel = self.variaveis.get(&lugar.raiz)
            .ok_or_else(|| ErroPropriedade::VariavelNaoDeclarada(lugar.raiz.clone()))?;
        if variavel.movidos.iter().any(|movido| movido.sobrepoe(lugar)) {
            return Err(ErroPropriedade::UsoAposMovimento(lugar.clone()));
        }
        Ok(())
    }

    /// Move um lugar, invalidando-o até que seja reinicializado.
    pub fn mover(&mut self, lugar: &Lugar) -> Result<(), ErroPropriedade> {
        self.ler(lugar)?;
        if self.emprestimos.values().any(|emprestimo| emprestimo.lugar.sobrepoe(lugar)) {
            return Err(ErroPropriedade::MovimentoDuranteEmprestimo(lugar.clone()));
        }
        self.variaveis.get_mut(&lugar.raiz)
            .ok_or_else(|| ErroPropriedade::VariavelNaoDeclarada(lugar.raiz.clone()))?
            .movidos.insert(lugar.clone());
        Ok(())
    }

    /// Inicializa ou reinicializa um lugar.
    pub fn atribuir(&mut self, lugar: &Lugar) -> Result<(), ErroPropriedade> {
        let variavel = self.variaveis.get(&lugar.raiz)
            .ok_or_else(|| ErroPropriedade::VariavelNaoDeclarada(lugar.raiz.clone()))?;
        if self.emprestimos.values().any(|emprestimo| emprestimo.lugar.sobrepoe(lugar)) {
            return Err(ErroPropriedade::EscritaDuranteEmprestimo(lugar.clone()));
        }
        let _ = variavel;
        let estado = self.variaveis.get_mut(&lugar.raiz).expect("validado acima");
        estado.movidos.retain(|movido| {
            movido.raiz != lugar.raiz
                || lugar.projecoes.len() > movido.projecoes.len()
                || !lugar.projecoes.iter().zip(&movido.projecoes).all(|(a, b)| a == b)
        });
        Ok(())
    }

    /// Retorna o escopo proprietário de um lugar registrado.
    #[must_use]
    pub fn escopo_do_lugar(&self, lugar: &Lugar) -> Option<usize> {
        self.variaveis.get(&lugar.raiz).map(|variavel| variavel.escopo)
    }

    /// Verifica se a vida útil do proprietário cobre o escopo de destino.
    pub fn validar_destino_referencia(
        &self,
        lugar: &Lugar,
        escopo_destino: usize,
    ) -> Result<VidaUtil, ErroPropriedade> {
        let escopo_proprietario = self.escopo_do_lugar(lugar)
            .ok_or_else(|| ErroPropriedade::VariavelNaoDeclarada(lugar.raiz.clone()))?;
        let vida_util = VidaUtil {
            escopo_proprietario,
            escopo_referencia: self.escopo,
            parametro_origem: None,
            emprestimo: None,
        };
        if !vida_util.pode_ser_armazenada_em(escopo_destino) {
            return Err(ErroPropriedade::ReferenciaEscapaVidaUtil {
                lugar: lugar.clone(),
                escopo_proprietario,
                escopo_destino,
            });
        }
        Ok(vida_util)
    }

    /// Cria um empréstimo que permanece ativo até encerramento explícito ou saída do escopo.
    pub fn emprestar(
        &mut self,
        identificador: u64,
        lugar: Lugar,
        tipo: TipoEmprestimo,
    ) -> Result<(), ErroPropriedade> {
        self.ler(&lugar)?;
        if self.emprestimos.contains_key(&identificador) {
            return Err(ErroPropriedade::EmprestimoDuplicado(identificador));
        }
        if let Some(existente) = self.emprestimos.values().find(|ativo| {
            ativo.lugar.sobrepoe(&lugar)
                && (ativo.tipo == TipoEmprestimo::Exclusivo || tipo == TipoEmprestimo::Exclusivo)
        }) {
            return Err(ErroPropriedade::ConflitoEmprestimo {
                lugar, existente: existente.tipo, solicitado: tipo,
            });
        }
        let escopo_proprietario = self.escopo_do_lugar(&lugar)
            .ok_or_else(|| ErroPropriedade::VariavelNaoDeclarada(lugar.raiz.clone()))?;
        let vida_util = VidaUtil {
            escopo_proprietario,
            escopo_referencia: self.escopo,
            parametro_origem: None,
            emprestimo: Some(identificador),
        };
        if !vida_util.pode_ser_armazenada_em(self.escopo) {
            return Err(ErroPropriedade::ReferenciaEscapaVidaUtil {
                lugar,
                escopo_proprietario,
                escopo_destino: self.escopo,
            });
        }
        self.emprestimos.insert(identificador, EmprestimoAtivo {
            lugar, tipo, vida_util,
        });
        Ok(())
    }

    /// Estende a região de um empréstimo até o escopo de destino da referência.
    pub fn estender_vida_util_emprestimo(
        &mut self,
        identificador: u64,
        escopo_destino: usize,
    ) -> Result<(), ErroPropriedade> {
        let emprestimo = self.emprestimos.get_mut(&identificador)
            .ok_or(ErroPropriedade::EmprestimoInexistente(identificador))?;
        if !emprestimo.vida_util.pode_ser_armazenada_em(escopo_destino) {
            return Err(ErroPropriedade::ReferenciaEscapaVidaUtil {
                lugar: emprestimo.lugar.clone(),
                escopo_proprietario: emprestimo.vida_util.escopo_proprietario,
                escopo_destino,
            });
        }
        emprestimo.vida_util.escopo_referencia =
            emprestimo.vida_util.escopo_referencia.min(escopo_destino);
        Ok(())
    }

    /// Região de vida útil associada ao empréstimo ativo.
    #[must_use]
    pub fn vida_util_emprestimo(&self, identificador: u64) -> Option<VidaUtil> {
        self.emprestimos.get(&identificador).map(|emprestimo| emprestimo.vida_util)
    }

    /// Encerra um empréstimo identificado.
    pub fn encerrar_emprestimo(&mut self, identificador: u64) -> Result<(), ErroPropriedade> {
        self.emprestimos.remove(&identificador)
            .map(|_| ())
            .ok_or(ErroPropriedade::EmprestimoInexistente(identificador))
    }

    /// Abre um novo escopo lexical.
    pub fn entrar_escopo(&mut self) { self.escopo = self.escopo.saturating_add(1); }

    /// Fecha o escopo e devolve as destruições em ordem LIFO.
    ///
    /// Empréstimos criados no escopo são encerrados antes da destruição dos
    /// valores locais. Empréstimos criados em escopos internos não podem
    /// sobreviver ao escopo que os originou.
    pub fn sair_escopo(&mut self) -> Result<Vec<AcaoDestruicao>, ErroPropriedade> {
        if self.escopo == 0 { return Err(ErroPropriedade::EscopoGlobal); }
        let nivel = self.escopo;
        let locais: HashSet<String> = self.variaveis.iter()
            .filter(|(_, variavel)| variavel.escopo == nivel)
            .map(|(nome, _)| nome.clone())
            .collect();
        if let Some(emprestimo) = self.emprestimos.values().find(|emprestimo| {
            locais.contains(&emprestimo.lugar.raiz)
                && emprestimo.vida_util.escopo_referencia < nivel
        }) {
            return Err(ErroPropriedade::EmprestimoVivoAoDestruir(emprestimo.lugar.clone()));
        }
        self.emprestimos.retain(|_, emprestimo| emprestimo.vida_util.escopo_referencia != nivel);
        let mut destruicoes: Vec<_> = self.variaveis.iter()
            .filter(|(_, variavel)| variavel.escopo == nivel)
            .filter_map(|(nome, variavel)| {
                // Um valor integralmente movido não pertence mais a este escopo.
                if variavel.movidos.iter().any(|movido| {
                    movido.raiz == *nome && movido.projecoes.is_empty()
                }) {
                    return None;
                }
                let mut partes_movidas: Vec<_> = variavel.movidos.iter().cloned().collect();
                partes_movidas.sort();
                Some((variavel.ordem, AcaoDestruicao {
                    lugar: Lugar::novo(nome),
                    partes_movidas,
                }))
            })
            .collect();
        destruicoes.sort_by_key(|(ordem, _)| std::cmp::Reverse(*ordem));
        self.variaveis.retain(|_, variavel| variavel.escopo != nivel);
        self.escopo -= 1;
        Ok(destruicoes.into_iter().map(|(_, acao)| acao).collect())
    }

    /// Entra em um bloco inseguro explícito.
    pub fn entrar_inseguro(&mut self) {
        self.entrar_contexto_inseguro(ContextoInseguro::Critico);
    }

    /// Entra em um contexto inseguro com permissões delimitadas.
    pub fn entrar_contexto_inseguro(&mut self, contexto: ContextoInseguro) {
        self.contextos_inseguros.push(contexto);
    }

    /// Sai do bloco inseguro atual.
    pub fn sair_inseguro(&mut self) -> Result<(), ErroPropriedade> {
        self.contextos_inseguros.pop()
            .map(|_| ())
            .ok_or(ErroPropriedade::BlocoInseguroNaoAberto)
    }

    /// Valida apenas a presença de algum contexto inseguro.
    /// Para operações concretas, prefira validar_operacao_especifica.
    pub fn validar_operacao_insegura(&self) -> Result<(), ErroPropriedade> {
        match self.contextos_inseguros.last() {
            Some(ContextoInseguro::Seguro) | None => {
                Err(ErroPropriedade::OperacaoInseguraForaDeBloco)
            }
            Some(_) => Ok(()),
        }
    }

    /// Confere o contexto autorizado e as provas obrigatórias da operação.
    pub fn validar_operacao_especifica(
        &self,
        operacao: OperacaoInsegura,
        provas: ProvasInseguras,
    ) -> Result<(), ErroPropriedade> {
        let contexto = self.contextos_inseguros.last().copied();
        let contexto_aceito = match operacao {
            OperacaoInsegura::Montagem => contexto == Some(ContextoInseguro::Montagem),
            OperacaoInsegura::OperacaoPrivilegiada => contexto == Some(ContextoInseguro::Privilegio),
            OperacaoInsegura::DesreferenciarPonteiroBruto
            | OperacaoInsegura::CriarReferenciaDeEndereco
            | OperacaoInsegura::AcessarMemoria
            | OperacaoInsegura::ReinterpretarRepresentacao
            | OperacaoInsegura::ChamarExternoNaoVerificado
            | OperacaoInsegura::LiberarAlocacao
            | OperacaoInsegura::AcessarMemoriaMapeada => contexto == Some(ContextoInseguro::Critico),
        };
        if !contexto_aceito {
            return Err(ErroPropriedade::ContextoInseguroIncompativel { operacao, contexto });
        }
        let obrigacoes: &[(&str, bool)] = match operacao {
            OperacaoInsegura::DesreferenciarPonteiroBruto
            | OperacaoInsegura::CriarReferenciaDeEndereco => &[
                ("endereço válido", provas.endereco_valido),
                ("alinhamento válido", provas.alinhamento_valido),
                ("inicialização comprovada", provas.inicializacao_comprovada),
                ("aliasing controlado", provas.aliasing_controlado),
            ],
            OperacaoInsegura::AcessarMemoria | OperacaoInsegura::AcessarMemoriaMapeada => &[
                ("endereço válido", provas.endereco_valido),
                ("alinhamento válido", provas.alinhamento_valido),
                ("limites comprovados", provas.limites_comprovados),
                ("inicialização comprovada", provas.inicializacao_comprovada),
                ("aliasing controlado", provas.aliasing_controlado),
            ],
            OperacaoInsegura::ReinterpretarRepresentacao => &[
                ("alinhamento válido", provas.alinhamento_valido),
                ("inicialização comprovada", provas.inicializacao_comprovada),
                ("ABI validada", provas.abi_validada),
            ],
            OperacaoInsegura::ChamarExternoNaoVerificado => &[
                ("ABI validada", provas.abi_validada),
                ("alvo compatível", provas.alvo_compativel),
            ],
            OperacaoInsegura::Montagem => &[
                ("alvo compatível", provas.alvo_compativel),
                ("ABI validada", provas.abi_validada),
            ],
            OperacaoInsegura::OperacaoPrivilegiada => &[
                ("autorização do sistema", provas.autorizacao_sistema),
                ("alvo compatível", provas.alvo_compativel),
            ],
            OperacaoInsegura::LiberarAlocacao => &[
                ("endereço válido", provas.endereco_valido),
                ("limites comprovados", provas.limites_comprovados),
                ("aliasing controlado", provas.aliasing_controlado),
            ],
        };
        if let Some((obrigacao, _)) = obrigacoes.iter().find(|(_, comprovada)| !comprovada) {
            return Err(ErroPropriedade::ObrigacaoInseguraNaoComprovada {
                operacao,
                obrigacao,
            });
        }
        Ok(())
    }

    /// Une estados alternativos de fluxo de controle de forma conservadora.
    ///
    /// Movimentos e empréstimos ativos em qualquer caminho são preservados.
    /// Identificadores criados em ramos clonados podem coincidir; nesse caso,
    /// empréstimos distintos recebem identificadores novos para que nenhum
    /// deles desapareça na junção.
    pub fn mesclar_caminhos(&mut self, caminhos: &[Self]) -> Result<(), ErroPropriedade> {
        for caminho in caminhos {
            if caminho.escopo != self.escopo {
                return Err(ErroPropriedade::ProfundidadesEscopoDivergentes {
                    atual: self.escopo,
                    alternativo: caminho.escopo,
                });
            }
            if caminho.contextos_inseguros != self.contextos_inseguros {
                return Err(ErroPropriedade::ContextosInsegurosDivergentes);
            }
        }

        for (nome, variavel) in &mut self.variaveis {
            for caminho in caminhos {
                if let Some(outro) = caminho.variaveis.get(nome) {
                    variavel.movidos.extend(outro.movidos.iter().cloned());
                }
            }
        }

        let mut emprestimos = self.emprestimos.clone();
        let mut proximo = emprestimos.keys()
            .chain(caminhos.iter().flat_map(|caminho| caminho.emprestimos.keys()))
            .copied()
            .max()
            .unwrap_or(0)
            .checked_add(1);

        for caminho in caminhos {
            for (identificador, emprestimo) in &caminho.emprestimos {
                match emprestimos.get(identificador) {
                    None => {
                        emprestimos.insert(*identificador, emprestimo.clone());
                    }
                    Some(existente)
                        if existente.lugar == emprestimo.lugar
                            && existente.tipo == emprestimo.tipo
                            && existente.vida_util == emprestimo.vida_util => {}
                    Some(_) => {
                        let novo = loop {
                            let candidato = proximo.ok_or(ErroPropriedade::IdentificadoresEmprestimoEsgotados)?;
                            proximo = candidato.checked_add(1);
                            if !emprestimos.contains_key(&candidato) {
                                break candidato;
                            }
                        };
                        emprestimos.insert(novo, emprestimo.clone());
                    }
                }
            }
            self.proximo_ordem = self.proximo_ordem.max(caminho.proximo_ordem);
        }

        self.emprestimos = emprestimos;
        Ok(())
    }

    /// Próximo identificador livre após todos os empréstimos atualmente ativos.
    #[must_use]
    pub fn proximo_identificador_emprestimo(&self) -> Option<u64> {
        self.emprestimos.keys().copied().max().unwrap_or(0).checked_add(1)
    }

    /// Retorna o número de empréstimos ativos.
    #[must_use]
    pub fn quantidade_emprestimos(&self) -> usize { self.emprestimos.len() }
}


/// Operações que o analisador de fluxo de controle propaga entre blocos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperacaoFluxoPropriedade {
    Ler(Lugar),
    Mover(Lugar),
    Atribuir(Lugar),
    Emprestar { identificador: u64, lugar: Lugar, tipo: TipoEmprestimo },
    EncerrarEmprestimo(u64),
    EstenderVidaUtilEmprestimo { identificador: u64, escopo_destino: usize },
    EntrarEscopo,
    Declarar(String),
    SairEscopo,
    EntrarContextoInseguro(ContextoInseguro),
    SairContextoInseguro,
    ValidarOperacaoInsegura { operacao: OperacaoInsegura, provas: ProvasInseguras },
}

/// Bloco básico do grafo de fluxo de controle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlocoFluxoPropriedade {
    pub operacoes: Vec<OperacaoFluxoPropriedade>,
    pub sucessores: Vec<usize>,
}

/// Diagnóstico associado à operação e ao bloco que o produziu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErroFluxoPropriedade {
    pub bloco: usize,
    pub operacao: usize,
    pub erro: ErroPropriedade,
}

/// Estados de entrada/saída e erros encontrados na análise do CFG.
#[derive(Clone, Debug)]
pub struct ResultadoFluxoPropriedade {
    pub entradas: Vec<Option<AnalisadorPropriedade>>,
    pub saidas: Vec<Option<AnalisadorPropriedade>>,
    pub erros: Vec<ErroFluxoPropriedade>,
    pub destruicoes: Vec<DestruicaoFluxoPropriedade>,
}

/// Destruição lexical emitida por uma operação de saída de escopo no CFG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestruicaoFluxoPropriedade {
    pub bloco: usize,
    pub operacao: usize,
    pub acao: AcaoDestruicao,
}

/// Analisa movimentos e empréstimos em todos os caminhos alcançáveis do CFG.
///
/// A junção é conservadora: um movimento ou empréstimo presente em qualquer
/// predecessor permanece ativo no bloco sucessor. O algoritmo usa uma fila
/// de trabalho e reprocessa blocos até que os estados de entrada estabilizem,
/// inclusive em ciclos. Blocos inalcançáveis não são analisados.
pub fn analisar_fluxo_controle_propriedade(
    inicial: &AnalisadorPropriedade,
    blocos: &[BlocoFluxoPropriedade],
    entrada: usize,
) -> Result<ResultadoFluxoPropriedade, ErroPropriedade> {
    if entrada >= blocos.len() {
        return Err(ErroPropriedade::BlocoFluxoInexistente(entrada));
    }
    for bloco in blocos {
        for sucessor in &bloco.sucessores {
            if *sucessor >= blocos.len() {
                return Err(ErroPropriedade::BlocoFluxoInexistente(*sucessor));
            }
        }
    }

    let mut entradas = vec![None; blocos.len()];
    let mut saidas = vec![None; blocos.len()];
    let mut erros = Vec::new();
    let mut destruicoes = Vec::new();
    let mut fila = VecDeque::new();
    let mut enfileirados = HashSet::new();

    entradas[entrada] = Some(inicial.clone());
    fila.push_back(entrada);
    enfileirados.insert(entrada);

    while let Some(indice) = fila.pop_front() {
        enfileirados.remove(&indice);
        let Some(mut estado) = entradas[indice].clone() else {
            continue;
        };

        for (indice_operacao, operacao) in blocos[indice].operacoes.iter().enumerate() {
            let resultado = match operacao {
                OperacaoFluxoPropriedade::Ler(lugar) => estado.ler(lugar).map(|_| Vec::new()),
                OperacaoFluxoPropriedade::Mover(lugar) => estado.mover(lugar).map(|_| Vec::new()),
                OperacaoFluxoPropriedade::Atribuir(lugar) => estado.atribuir(lugar).map(|_| Vec::new()),
                OperacaoFluxoPropriedade::Emprestar { identificador, lugar, tipo } => {
                    estado.emprestar(*identificador, lugar.clone(), *tipo).map(|_| Vec::new())
                }
                OperacaoFluxoPropriedade::EncerrarEmprestimo(identificador) => {
                    estado.encerrar_emprestimo(*identificador).map(|_| Vec::new())
                }
                OperacaoFluxoPropriedade::EstenderVidaUtilEmprestimo { identificador, escopo_destino } => {
                    estado.estender_vida_util_emprestimo(*identificador, *escopo_destino).map(|_| Vec::new())
                }
                OperacaoFluxoPropriedade::EntrarEscopo => {
                    estado.entrar_escopo();
                    Ok(Vec::new())
                }
                OperacaoFluxoPropriedade::Declarar(nome) => {
                    estado.declarar(nome.clone()).map(|_| Vec::new())
                }
                OperacaoFluxoPropriedade::SairEscopo => estado.sair_escopo(),
                OperacaoFluxoPropriedade::EntrarContextoInseguro(contexto) => {
                    estado.entrar_contexto_inseguro(*contexto);
                    Ok(Vec::new())
                }
                OperacaoFluxoPropriedade::SairContextoInseguro => {
                    estado.sair_inseguro().map(|_| Vec::new())
                }
                OperacaoFluxoPropriedade::ValidarOperacaoInsegura { operacao, provas } => {
                    estado.validar_operacao_especifica(*operacao, *provas).map(|_| Vec::new())
                }
            };
            match resultado {
                Ok(acoes) => {
                    for acao in acoes {
                        let destruicao = DestruicaoFluxoPropriedade {
                            bloco: indice,
                            operacao: indice_operacao,
                            acao,
                        };
                        if !destruicoes.contains(&destruicao) {
                            destruicoes.push(destruicao);
                        }
                    }
                }
                Err(erro) => {
                let diagnostico = ErroFluxoPropriedade {
                    bloco: indice,
                    operacao: indice_operacao,
                    erro,
                };
                if !erros.contains(&diagnostico) {
                    erros.push(diagnostico);
                }
                }
            }
        }

        saidas[indice] = Some(estado.clone());
        for sucessor in &blocos[indice].sucessores {
            let mudou = match entradas[*sucessor].as_mut() {
                None => {
                    entradas[*sucessor] = Some(estado.clone());
                    true
                }
                Some(estado_entrada) => {
                    let anterior = estado_entrada.clone();
                    estado_entrada.mesclar_caminhos(&[estado.clone()])?;
                    *estado_entrada != anterior
                }
            };
            if mudou && enfileirados.insert(*sucessor) {
                fila.push_back(*sucessor);
            }
        }
    }

    destruicoes.sort_by_key(|item| (item.bloco, item.operacao));
    Ok(ResultadoFluxoPropriedade { entradas, saidas, erros, destruicoes })
}

#[cfg(test)]
mod testes {
    use super::*;


    #[test]
    fn cfg_junta_movimento_de_um_ramo_e_rejeita_leitura_posterior() {
        let mut inicial = AnalisadorPropriedade::novo();
        inicial.declarar("recurso").unwrap();
        let blocos = vec![
            BlocoFluxoPropriedade { operacoes: vec![], sucessores: vec![1, 2] },
            BlocoFluxoPropriedade {
                operacoes: vec![OperacaoFluxoPropriedade::Mover(Lugar::novo("recurso"))],
                sucessores: vec![3],
            },
            BlocoFluxoPropriedade { operacoes: vec![], sucessores: vec![3] },
            BlocoFluxoPropriedade {
                operacoes: vec![OperacaoFluxoPropriedade::Ler(Lugar::novo("recurso"))],
                sucessores: vec![],
            },
        ];
        let resultado = analisar_fluxo_controle_propriedade(&inicial, &blocos, 0).unwrap();
        assert!(resultado.erros.iter().any(|erro|
            erro.bloco == 3 && matches!(erro.erro, ErroPropriedade::UsoAposMovimento(_))
        ));
    }

    #[test]
    fn cfg_junta_emprestimo_de_um_ramo_e_bloqueia_movimento() {
        let mut inicial = AnalisadorPropriedade::novo();
        inicial.declarar("valor").unwrap();
        let blocos = vec![
            BlocoFluxoPropriedade { operacoes: vec![], sucessores: vec![1, 2] },
            BlocoFluxoPropriedade {
                operacoes: vec![OperacaoFluxoPropriedade::Emprestar {
                    identificador: 91,
                    lugar: Lugar::novo("valor"),
                    tipo: TipoEmprestimo::Compartilhado,
                }],
                sucessores: vec![3],
            },
            BlocoFluxoPropriedade { operacoes: vec![], sucessores: vec![3] },
            BlocoFluxoPropriedade {
                operacoes: vec![OperacaoFluxoPropriedade::Mover(Lugar::novo("valor"))],
                sucessores: vec![],
            },
        ];
        let resultado = analisar_fluxo_controle_propriedade(&inicial, &blocos, 0).unwrap();
        assert!(resultado.erros.iter().any(|erro|
            erro.bloco == 3 && matches!(erro.erro, ErroPropriedade::MovimentoDuranteEmprestimo(_))
        ));
    }

    #[test]
    fn cfg_processa_ciclo_ate_estabilizar_e_ignora_bloco_inalcancavel() {
        let mut inicial = AnalisadorPropriedade::novo();
        inicial.declarar("contador").unwrap();
        let blocos = vec![
            BlocoFluxoPropriedade { operacoes: vec![], sucessores: vec![1] },
            BlocoFluxoPropriedade { operacoes: vec![], sucessores: vec![1, 2] },
            BlocoFluxoPropriedade { operacoes: vec![], sucessores: vec![] },
            BlocoFluxoPropriedade {
                operacoes: vec![OperacaoFluxoPropriedade::Ler(Lugar::novo("nao_alcancavel"))],
                sucessores: vec![],
            },
        ];
        let resultado = analisar_fluxo_controle_propriedade(&inicial, &blocos, 0).unwrap();
        assert!(resultado.entradas[1].is_some());
        assert!(resultado.entradas[2].is_some());
        assert!(resultado.entradas[3].is_none());
        assert!(resultado.erros.is_empty());
    }

    #[test]
    fn cfg_valida_operacao_unsafe_dentro_de_contexto_explicito() {
        let provas = ProvasInseguras {
            endereco_valido: true,
            alinhamento_valido: true,
            inicializacao_comprovada: true,
            aliasing_controlado: true,
            ..ProvasInseguras::default()
        };
        let blocos = vec![BlocoFluxoPropriedade {
            operacoes: vec![
                OperacaoFluxoPropriedade::EntrarContextoInseguro(ContextoInseguro::Critico),
                OperacaoFluxoPropriedade::ValidarOperacaoInsegura {
                    operacao: OperacaoInsegura::DesreferenciarPonteiroBruto,
                    provas,
                },
                OperacaoFluxoPropriedade::SairContextoInseguro,
            ],
            sucessores: vec![],
        }];
        let resultado = analisar_fluxo_controle_propriedade(
            &AnalisadorPropriedade::novo(), &blocos, 0
        ).unwrap();
        assert!(resultado.erros.is_empty());
    }

    #[test]
    fn cfg_bloqueia_unsafe_herdado_dentro_de_bloco_seguro() {
        let provas = ProvasInseguras {
            endereco_valido: true,
            alinhamento_valido: true,
            inicializacao_comprovada: true,
            aliasing_controlado: true,
            ..ProvasInseguras::default()
        };
        let blocos = vec![BlocoFluxoPropriedade {
            operacoes: vec![
                OperacaoFluxoPropriedade::EntrarContextoInseguro(ContextoInseguro::Critico),
                OperacaoFluxoPropriedade::EntrarContextoInseguro(ContextoInseguro::Seguro),
                OperacaoFluxoPropriedade::ValidarOperacaoInsegura {
                    operacao: OperacaoInsegura::DesreferenciarPonteiroBruto,
                    provas,
                },
                OperacaoFluxoPropriedade::SairContextoInseguro,
                OperacaoFluxoPropriedade::SairContextoInseguro,
            ],
            sucessores: vec![],
        }];
        let resultado = analisar_fluxo_controle_propriedade(
            &AnalisadorPropriedade::novo(), &blocos, 0
        ).unwrap();
        assert!(resultado.erros.iter().any(|erro|
            erro.operacao == 2
                && matches!(erro.erro, ErroPropriedade::ContextoInseguroIncompativel { .. })
        ));
    }

    #[test]
    fn cfg_emite_destruicao_lifo_ao_sair_de_escopo() {
        let blocos = vec![BlocoFluxoPropriedade {
            operacoes: vec![
                OperacaoFluxoPropriedade::EntrarEscopo,
                OperacaoFluxoPropriedade::Declarar("primeiro".into()),
                OperacaoFluxoPropriedade::Declarar("segundo".into()),
                OperacaoFluxoPropriedade::SairEscopo,
            ],
            sucessores: vec![],
        }];
        let resultado = analisar_fluxo_controle_propriedade(
            &AnalisadorPropriedade::novo(), &blocos, 0
        ).unwrap();
        let nomes = resultado.destruicoes.iter()
            .map(|item| item.acao.lugar.raiz.as_str())
            .collect::<Vec<_>>();
        assert_eq!(nomes, vec!["segundo", "primeiro"]);
        assert!(resultado.erros.is_empty());
    }

    #[test]
    fn cfg_rejeita_referencia_que_escapa_da_vida_util_do_proprietario() {
        let blocos = vec![BlocoFluxoPropriedade {
            operacoes: vec![
                OperacaoFluxoPropriedade::EntrarEscopo,
                OperacaoFluxoPropriedade::Declarar("local".into()),
                OperacaoFluxoPropriedade::Emprestar {
                    identificador: 501,
                    lugar: Lugar::novo("local"),
                    tipo: TipoEmprestimo::Compartilhado,
                },
                OperacaoFluxoPropriedade::EstenderVidaUtilEmprestimo {
                    identificador: 501,
                    escopo_destino: 0,
                },
                OperacaoFluxoPropriedade::SairEscopo,
            ],
            sucessores: vec![],
        }];
        let resultado = analisar_fluxo_controle_propriedade(
            &AnalisadorPropriedade::novo(), &blocos, 0
        ).unwrap();
        assert!(resultado.erros.iter().any(|erro|
            erro.operacao == 3
                && matches!(erro.erro, ErroPropriedade::ReferenciaEscapaVidaUtil { .. })
        ));
    }

    #[test]
    fn cfg_nao_emite_destruicao_para_valor_integralmente_movido() {
        let blocos = vec![BlocoFluxoPropriedade {
            operacoes: vec![
                OperacaoFluxoPropriedade::EntrarEscopo,
                OperacaoFluxoPropriedade::Declarar("recurso".into()),
                OperacaoFluxoPropriedade::Mover(Lugar::novo("recurso")),
                OperacaoFluxoPropriedade::SairEscopo,
            ],
            sucessores: vec![],
        }];
        let resultado = analisar_fluxo_controle_propriedade(
            &AnalisadorPropriedade::novo(), &blocos, 0
        ).unwrap();
        assert!(resultado.destruicoes.is_empty());
        assert!(resultado.erros.is_empty());
    }

    #[test]
    fn cfg_rejeita_aresta_para_bloco_inexistente() {
        let blocos = vec![BlocoFluxoPropriedade { operacoes: vec![], sucessores: vec![4] }];
        assert_eq!(
            analisar_fluxo_controle_propriedade(&AnalisadorPropriedade::novo(), &blocos, 0)
                .unwrap_err(),
            ErroPropriedade::BlocoFluxoInexistente(4)
        );
    }

    #[test]
    fn campos_independentes_nao_sobrepoem_mas_agregado_contem_campos() {
        let a = Lugar::novo("pessoa").campo("nome");
        let b = Lugar::novo("pessoa").campo("idade");
        assert!(!a.sobrepoe(&b));
        assert!(Lugar::novo("pessoa").sobrepoe(&a));
    }

    #[test]
    fn indice_dinamico_e_conservador() {
        let a = Lugar::novo("itens").indice_dinamico();
        let b = Lugar::novo("itens").indice(4);
        assert!(a.sobrepoe(&b));
        assert!(!Lugar::novo("itens").indice(3).sobrepoe(&b));
    }

    #[test]
    fn move_parcial_preserva_irmaos_e_reinicializacao_restaura_caminho() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.declarar("pessoa").unwrap();
        let nome = Lugar::novo("pessoa").campo("nome");
        let idade = Lugar::novo("pessoa").campo("idade");
        fluxo.mover(&nome).unwrap();
        assert!(fluxo.ler(&idade).is_ok());
        assert!(matches!(fluxo.ler(&Lugar::novo("pessoa")), Err(ErroPropriedade::UsoAposMovimento(_))));
        fluxo.atribuir(&nome).unwrap();
        assert!(fluxo.ler(&Lugar::novo("pessoa")).is_ok());
    }

    #[test]
    fn atribuir_campo_nao_restaura_agregado_inteiro_movido() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.declarar("pessoa").unwrap();
        fluxo.mover(&Lugar::novo("pessoa")).unwrap();
        fluxo.atribuir(&Lugar::novo("pessoa").campo("nome")).unwrap();
        assert!(matches!(
            fluxo.ler(&Lugar::novo("pessoa")),
            Err(ErroPropriedade::UsoAposMovimento(_))
        ));
    }

    #[test]
    fn emprestimo_registra_regiao_de_vida_util_do_proprietario() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.declarar("global").unwrap();
        fluxo.entrar_escopo();
        fluxo.emprestar(42, Lugar::novo("global"), TipoEmprestimo::Compartilhado).unwrap();
        assert_eq!(
            fluxo.vida_util_emprestimo(42),
            Some(VidaUtil { escopo_proprietario: 0, escopo_referencia: 1, parametro_origem: None, emprestimo: Some(42) })
        );
    }

    #[test]
    fn vida_util_permite_referencia_em_escopo_interno_de_proprietario_externo() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.declarar("global").unwrap();
        fluxo.entrar_escopo();
        let vida = fluxo.validar_destino_referencia(&Lugar::novo("global"), 1).unwrap();
        assert_eq!(vida.escopo_proprietario, 0);
        assert_eq!(vida.escopo_referencia, 1);
        assert!(vida.pode_ser_armazenada_em(1));
    }

    #[test]
    fn emprestimo_estendido_sobrevive_ao_escopo_de_criacao() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.declarar("global").unwrap();
        fluxo.entrar_escopo();
        fluxo.emprestar(77, Lugar::novo("global"), TipoEmprestimo::Compartilhado).unwrap();
        fluxo.estender_vida_util_emprestimo(77, 0).unwrap();
        fluxo.sair_escopo().unwrap();
        assert_eq!(fluxo.quantidade_emprestimos(), 1);
        assert!(matches!(
            fluxo.mover(&Lugar::novo("global")),
            Err(ErroPropriedade::MovimentoDuranteEmprestimo(_))
        ));
        fluxo.encerrar_emprestimo(77).unwrap();
        assert!(fluxo.mover(&Lugar::novo("global")).is_ok());
    }

    #[test]
    fn vida_util_rejeita_referencia_que_escapa_do_proprietario_local() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.entrar_escopo();
        fluxo.declarar("local").unwrap();
        assert!(matches!(
            fluxo.validar_destino_referencia(&Lugar::novo("local"), 0),
            Err(ErroPropriedade::ReferenciaEscapaVidaUtil {
                escopo_proprietario: 1,
                escopo_destino: 0,
                ..
            })
        ));
    }

    #[test]
    fn rejeita_movimento_e_escrita_com_emprestimo_ativo() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.declarar("valor").unwrap();
        fluxo.emprestar(1, Lugar::novo("valor"), TipoEmprestimo::Compartilhado).unwrap();
        assert!(matches!(fluxo.mover(&Lugar::novo("valor")), Err(ErroPropriedade::MovimentoDuranteEmprestimo(_))));
        assert!(matches!(fluxo.atribuir(&Lugar::novo("valor")), Err(ErroPropriedade::EscritaDuranteEmprestimo(_))));
        fluxo.encerrar_emprestimo(1).unwrap();
        fluxo.mover(&Lugar::novo("valor")).unwrap();
    }

    #[test]
    fn permite_leitores_e_rejeita_alias_mutavel() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.declarar("valor").unwrap();
        fluxo.emprestar(1, Lugar::novo("valor"), TipoEmprestimo::Compartilhado).unwrap();
        fluxo.emprestar(2, Lugar::novo("valor"), TipoEmprestimo::Compartilhado).unwrap();
        assert!(matches!(
            fluxo.emprestar(3, Lugar::novo("valor"), TipoEmprestimo::Exclusivo),
            Err(ErroPropriedade::ConflitoEmprestimo { .. })
        ));
        fluxo.encerrar_emprestimo(1).unwrap();
        fluxo.encerrar_emprestimo(2).unwrap();
        fluxo.emprestar(3, Lugar::novo("valor"), TipoEmprestimo::Exclusivo).unwrap();
    }

    #[test]
    fn destruicao_lexical_e_lifo_e_encerra_emprestimos_locais() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.entrar_escopo();
        fluxo.declarar("primeiro").unwrap();
        fluxo.declarar("segundo").unwrap();
        fluxo.emprestar(1, Lugar::novo("primeiro"), TipoEmprestimo::Compartilhado).unwrap();
        let acoes = fluxo.sair_escopo().unwrap();
        assert_eq!(acoes.iter().map(|a| a.lugar.raiz.as_str()).collect::<Vec<_>>(), vec!["segundo", "primeiro"]);
        assert_eq!(fluxo.quantidade_emprestimos(), 0);
    }

    #[test]
    fn rejeita_juncao_de_fluxos_com_escopos_diferentes() {
        let mut base = AnalisadorPropriedade::novo();
        let mut alternativo = base.clone();
        alternativo.entrar_escopo();
        assert_eq!(
            base.mesclar_caminhos(&[alternativo]),
            Err(ErroPropriedade::ProfundidadesEscopoDivergentes {
                atual: 0,
                alternativo: 1,
            })
        );
    }

    #[test]
    fn rejeita_juncao_de_fluxos_com_contextos_inseguros_diferentes() {
        let mut base = AnalisadorPropriedade::novo();
        let mut alternativo = base.clone();
        alternativo.entrar_contexto_inseguro(ContextoInseguro::Critico);
        assert_eq!(
            base.mesclar_caminhos(&[alternativo]),
            Err(ErroPropriedade::ContextosInsegurosDivergentes)
        );
    }

    #[test]
    fn junção_de_ramos_preserva_emprestimos_com_ids_colidentes() {
        let mut base = AnalisadorPropriedade::novo();
        base.declarar("esquerda").unwrap();
        base.declarar("direita").unwrap();
        base.declarar("terceira").unwrap();

        let mut ramo_a = base.clone();
        ramo_a.emprestar(1, Lugar::novo("esquerda"), TipoEmprestimo::Compartilhado).unwrap();
        let mut ramo_b = base.clone();
        ramo_b.emprestar(1, Lugar::novo("direita"), TipoEmprestimo::Compartilhado).unwrap();

        base.mesclar_caminhos(&[ramo_a, ramo_b]).unwrap();
        assert_eq!(base.quantidade_emprestimos(), 2);
        assert!(base.mover(&Lugar::novo("esquerda")).is_err());
        assert!(base.mover(&Lugar::novo("direita")).is_err());

        let proximo = base.proximo_identificador_emprestimo().unwrap();
        base.emprestar(proximo, Lugar::novo("terceira"), TipoEmprestimo::Compartilhado).unwrap();
        assert_eq!(base.quantidade_emprestimos(), 3);
    }

    #[test]
    fn junção_deduplica_o_mesmo_emprestimo_presente_em_dois_ramos() {
        let mut base = AnalisadorPropriedade::novo();
        base.declarar("valor").unwrap();
        let mut ramo_a = base.clone();
        ramo_a.emprestar(7, Lugar::novo("valor"), TipoEmprestimo::Compartilhado).unwrap();
        let ramo_b = ramo_a.clone();

        base.mesclar_caminhos(&[ramo_a, ramo_b]).unwrap();
        assert_eq!(base.quantidade_emprestimos(), 1);
    }

    #[test]
    fn destruicao_omite_valor_integralmente_movido() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.entrar_escopo();
        fluxo.declarar("recurso").unwrap();
        fluxo.mover(&Lugar::novo("recurso")).unwrap();
        assert!(fluxo.sair_escopo().unwrap().is_empty());
    }

    #[test]
    fn destruicao_parcial_registra_projecoes_que_nao_devem_ser_destruidas() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.entrar_escopo();
        fluxo.declarar("pessoa").unwrap();
        let campo = Lugar::novo("pessoa").campo("nome");
        fluxo.mover(&campo).unwrap();
        let destruicoes = fluxo.sair_escopo().unwrap();
        assert_eq!(destruicoes.len(), 1);
        assert_eq!(destruicoes[0].lugar, Lugar::novo("pessoa"));
        assert_eq!(destruicoes[0].partes_movidas, vec![campo]);
    }

    #[test]
    fn operacao_insegura_exige_bloco_explicito() {
        let mut fluxo = AnalisadorPropriedade::novo();
        assert_eq!(fluxo.validar_operacao_insegura(), Err(ErroPropriedade::OperacaoInseguraForaDeBloco));
        fluxo.entrar_inseguro();
        assert!(fluxo.validar_operacao_insegura().is_ok());
        fluxo.sair_inseguro().unwrap();
        assert!(fluxo.validar_operacao_insegura().is_err());
    }

    #[test]
    fn todas_as_operacoes_inseguras_exigem_contexto_e_provas_especificos() {
        let operacoes = [
            (OperacaoInsegura::DesreferenciarPonteiroBruto, ContextoInseguro::Critico),
            (OperacaoInsegura::CriarReferenciaDeEndereco, ContextoInseguro::Critico),
            (OperacaoInsegura::AcessarMemoria, ContextoInseguro::Critico),
            (OperacaoInsegura::ReinterpretarRepresentacao, ContextoInseguro::Critico),
            (OperacaoInsegura::ChamarExternoNaoVerificado, ContextoInseguro::Critico),
            (OperacaoInsegura::Montagem, ContextoInseguro::Montagem),
            (OperacaoInsegura::OperacaoPrivilegiada, ContextoInseguro::Privilegio),
            (OperacaoInsegura::LiberarAlocacao, ContextoInseguro::Critico),
            (OperacaoInsegura::AcessarMemoriaMapeada, ContextoInseguro::Critico),
        ];
        let provas_completas = ProvasInseguras {
            endereco_valido: true,
            alinhamento_valido: true,
            inicializacao_comprovada: true,
            aliasing_controlado: true,
            limites_comprovados: true,
            abi_validada: true,
            alvo_compativel: true,
            autorizacao_sistema: true,
        };

        for (operacao, contexto) in operacoes {
            let mut analisador = AnalisadorPropriedade::novo();
            analisador.entrar_contexto_inseguro(contexto);
            assert!(
                analisador.validar_operacao_especifica(operacao, provas_completas).is_ok(),
                "A operação {operacao:?} deve aceitar seu contexto e todas as provas"
            );
            assert!(
                matches!(
                    analisador.validar_operacao_especifica(operacao, ProvasInseguras::default()),
                    Err(ErroPropriedade::ObrigacaoInseguraNaoComprovada { .. })
                ),
                "A operação {operacao:?} não pode aceitar provas ausentes"
            );
        }
    }

    #[test]
    fn contexto_critico_nao_autoriza_operacao_privilegiada_ou_montagem() {
        let mut analisador = AnalisadorPropriedade::novo();
        analisador.entrar_contexto_inseguro(ContextoInseguro::Critico);
        let provas = ProvasInseguras {
            alvo_compativel: true,
            abi_validada: true,
            autorizacao_sistema: true,
            ..ProvasInseguras::default()
        };
        assert!(matches!(
            analisador.validar_operacao_especifica(OperacaoInsegura::OperacaoPrivilegiada, provas),
            Err(ErroPropriedade::ContextoInseguroIncompativel { .. })
        ));
        assert!(matches!(
            analisador.validar_operacao_especifica(OperacaoInsegura::Montagem, provas),
            Err(ErroPropriedade::ContextoInseguroIncompativel { .. })
        ));
    }

    #[test]
    fn operacao_insegura_exige_contexto_especifico_e_provas() {
        let mut fluxo = AnalisadorPropriedade::novo();
        fluxo.entrar_contexto_inseguro(ContextoInseguro::Critico);
        assert!(matches!(
            fluxo.validar_operacao_especifica(OperacaoInsegura::Montagem, ProvasInseguras::default()),
            Err(ErroPropriedade::ContextoInseguroIncompativel { .. })
        ));
        assert!(matches!(
            fluxo.validar_operacao_especifica(OperacaoInsegura::DesreferenciarPonteiroBruto, ProvasInseguras::default()),
            Err(ErroPropriedade::ObrigacaoInseguraNaoComprovada { .. })
        ));
        let provas = ProvasInseguras {
            endereco_valido: true,
            alinhamento_valido: true,
            inicializacao_comprovada: true,
            aliasing_controlado: true,
            ..ProvasInseguras::default()
        };
        assert!(fluxo.validar_operacao_especifica(OperacaoInsegura::DesreferenciarPonteiroBruto, provas).is_ok());
    }
}
