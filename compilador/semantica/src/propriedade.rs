//! Análise explícita de propriedade, empréstimos e destruição lexical.
//!
//! Este módulo fornece o núcleo de fluxo de propriedade para a análise semântica.
//! Ele não acessa memória diretamente: opera sobre lugares abstratos e deve ser
//! alimentado pelo lowering/CFG do compilador.

use std::collections::{HashMap, HashSet};

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
    EmprestimoInexistente(u64),
    EmprestimoVivoAoDestruir(Lugar),
    EscopoGlobal,
    OperacaoInseguraForaDeBloco,
    BlocoInseguroNaoAberto,
}

#[derive(Clone, Debug)]
struct VariavelPropriedade {
    escopo: usize,
    ordem: u64,
    movidos: HashSet<Lugar>,
}

#[derive(Clone, Debug)]
struct EmprestimoAtivo {
    lugar: Lugar,
    tipo: TipoEmprestimo,
    escopo: usize,
}

/// Ação de destruição determinística emitida ao sair de um escopo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcaoDestruicao {
    pub lugar: Lugar,
}

/// Estado do verificador para um fluxo de controle.
#[derive(Clone, Debug)]
pub struct AnalisadorPropriedade {
    escopo: usize,
    proximo_ordem: u64,
    variaveis: HashMap<String, VariavelPropriedade>,
    emprestimos: HashMap<u64, EmprestimoAtivo>,
    profundidade_insegura: usize,
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
            profundidade_insegura: 0,
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
        estado.movidos.retain(|movido| !movido.sobrepoe(lugar));
        Ok(())
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
        self.emprestimos.insert(identificador, EmprestimoAtivo {
            lugar, tipo, escopo: self.escopo,
        });
        Ok(())
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
            locais.contains(&emprestimo.lugar.raiz) && emprestimo.escopo < nivel
        }) {
            return Err(ErroPropriedade::EmprestimoVivoAoDestruir(emprestimo.lugar.clone()));
        }
        self.emprestimos.retain(|_, emprestimo| emprestimo.escopo != nivel);
        let mut destruicoes: Vec<_> = self.variaveis.iter()
            .filter(|(_, variavel)| variavel.escopo == nivel)
            .map(|(nome, variavel)| (variavel.ordem, AcaoDestruicao { lugar: Lugar::novo(nome) }))
            .collect();
        destruicoes.sort_by_key(|(ordem, _)| std::cmp::Reverse(*ordem));
        self.variaveis.retain(|_, variavel| variavel.escopo != nivel);
        self.escopo -= 1;
        Ok(destruicoes.into_iter().map(|(_, acao)| acao).collect())
    }

    /// Entra em um bloco inseguro explícito.
    pub fn entrar_inseguro(&mut self) { self.profundidade_insegura += 1; }

    /// Sai do bloco inseguro atual.
    pub fn sair_inseguro(&mut self) -> Result<(), ErroPropriedade> {
        if self.profundidade_insegura == 0 {
            return Err(ErroPropriedade::BlocoInseguroNaoAberto);
        }
        self.profundidade_insegura -= 1;
        Ok(())
    }

    /// Valida uma operação que exige contexto inseguro.
    pub fn validar_operacao_insegura(&self) -> Result<(), ErroPropriedade> {
        if self.profundidade_insegura == 0 {
            Err(ErroPropriedade::OperacaoInseguraForaDeBloco)
        } else {
            Ok(())
        }
    }

    /// Retorna o número de empréstimos ativos.
    #[must_use]
    pub fn quantidade_emprestimos(&self) -> usize { self.emprestimos.len() }
}

#[cfg(test)]
mod testes {
    use super::*;

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
    fn operacao_insegura_exige_bloco_explicito() {
        let mut fluxo = AnalisadorPropriedade::novo();
        assert_eq!(fluxo.validar_operacao_insegura(), Err(ErroPropriedade::OperacaoInseguraForaDeBloco));
        fluxo.entrar_inseguro();
        assert!(fluxo.validar_operacao_insegura().is_ok());
        fluxo.sair_inseguro().unwrap();
        assert!(fluxo.validar_operacao_insegura().is_err());
    }
}
