//! Runtime seguro da Nebula: valores, escopos e memória gerenciada.
//!
//! O runtime usa apenas abstrações seguras de Rust. Valores compostos podem
//! compartilhar listas protegidas por locks, permitindo uso entre tarefas
//! sem expor ponteiros ou referências inválidas à linguagem.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Valor dinâmico utilizado pelo runtime da Nebula.
#[derive(Clone, Debug)]
pub enum Valor {
    /// Valor ausente de retorno.
    Vazio,
    /// Ausência explícita de valor.
    Nulo,
    /// Valor lógico.
    Logico(bool),
    /// Inteiro assinado de 128 bits.
    Inteiro(i128),
    /// Número decimal de 64 bits.
    Decimal(f64),
    /// Texto UTF-8.
    Texto(String),
    /// Caractere Unicode.
    Caractere(char),
    /// Lista compartilhada e protegida contra acesso concorrente.
    Lista(Arc<RwLock<Vec<Valor>>>),
    /// Intervalo inteiro inclusivo.
    Intervalo { inicio: i128, fim: i128 },
}

impl PartialEq for Valor {
    fn eq(&self, outro: &Self) -> bool {
        match (self, outro) {
            (Self::Vazio, Self::Vazio) | (Self::Nulo, Self::Nulo) => true,
            (Self::Logico(a), Self::Logico(b)) => a == b,
            (Self::Inteiro(a), Self::Inteiro(b)) => a == b,
            (Self::Decimal(a), Self::Decimal(b)) => a == b,
            (Self::Texto(a), Self::Texto(b)) => a == b,
            (Self::Caractere(a), Self::Caractere(b)) => a == b,
            (
                Self::Intervalo {
                    inicio: ai,
                    fim: af,
                },
                Self::Intervalo {
                    inicio: bi,
                    fim: bf,
                },
            ) => ai == bi && af == bf,
            (Self::Lista(a), Self::Lista(b)) => match (a.read(), b.read()) {
                (Ok(a), Ok(b)) => *a == *b,
                _ => false,
            },
            _ => false,
        }
    }
}

/// Operação inteira sujeita a verificação de overflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperacaoInteira {
    /// Soma.
    Soma,
    /// Subtração.
    Subtracao,
    /// Multiplicação.
    Multiplicacao,
    /// Divisão.
    Divisao,
    /// Resto.
    Resto,
    /// Potência com expoente não negativo.
    Potencia,
}

/// Erros recuperáveis do runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroRuntime {
    /// O identificador não existe em nenhum escopo ativo.
    NomeNaoEncontrado(String),
    /// O identificador já existe no escopo atual.
    NomeDuplicado(String),
    /// Tentativa de alterar uma variável constante.
    VariavelImutavel(String),
    /// Tentativa de remover o escopo global.
    EscopoGlobal,
    /// Operação solicitada para um tipo incompatível.
    TipoIncompativel(&'static str),
    /// Índice fora dos limites da lista.
    IndiceForaDosLimites { indice: usize, tamanho: usize },
    /// Lock de uma estrutura compartilhada foi envenenado.
    MemoriaCorrompida,
    /// A operação inteira excedeu a faixa de i128.
    Overflow,
    /// Divisão ou resto por zero.
    DivisaoPorZero,
    /// Expoente negativo não é aceito para potência inteira.
    ExpoenteNegativo,
}

/// Resultado padrão das operações do runtime.
pub type Resultado<T> = Result<T, ErroRuntime>;

#[derive(Clone, Debug)]
struct Celula {
    valor: Valor,
    mutavel: bool,
}

/// Memória de execução com escopos léxicos e sombreamento.
#[derive(Clone, Debug)]
pub struct Memoria {
    escopos: Vec<HashMap<String, Celula>>,
}

impl Default for Memoria {
    fn default() -> Self {
        Self::nova()
    }
}

impl Memoria {
    /// Cria a memória com o escopo global aberto.
    #[must_use]
    pub fn nova() -> Self {
        Self {
            escopos: vec![HashMap::new()],
        }
    }

    /// Retorna a quantidade de escopos ativos, incluindo o global.
    #[must_use]
    pub fn quantidade_escopos(&self) -> usize {
        self.escopos.len()
    }

    /// Abre um novo escopo léxico.
    pub fn abrir_escopo(&mut self) {
        self.escopos.push(HashMap::new());
    }

    /// Fecha o escopo atual sem permitir remover o escopo global.
    pub fn fechar_escopo(&mut self) -> Resultado<()> {
        if self.escopos.len() == 1 {
            return Err(ErroRuntime::EscopoGlobal);
        }
        self.escopos.pop();
        Ok(())
    }

    /// Declara uma variável no escopo atual.
    pub fn declarar(
        &mut self,
        nome: impl Into<String>,
        valor: Valor,
        mutavel: bool,
    ) -> Resultado<()> {
        let nome = nome.into();
        let escopo = self
            .escopos
            .last_mut()
            .expect("escopo global sempre existe");
        if escopo.contains_key(&nome) {
            return Err(ErroRuntime::NomeDuplicado(nome));
        }
        escopo.insert(nome, Celula { valor, mutavel });
        Ok(())
    }

    /// Obtém uma cópia do valor visível, respeitando sombreamento.
    pub fn obter(&self, nome: &str) -> Resultado<Valor> {
        self.escopos
            .iter()
            .rev()
            .find_map(|escopo| escopo.get(nome))
            .map(|celula| celula.valor.clone())
            .ok_or_else(|| ErroRuntime::NomeNaoEncontrado(nome.to_owned()))
    }

    /// Atribui um novo valor à declaração visível mais próxima.
    pub fn atribuir(&mut self, nome: &str, valor: Valor) -> Resultado<()> {
        for escopo in self.escopos.iter_mut().rev() {
            if let Some(celula) = escopo.get_mut(nome) {
                if !celula.mutavel {
                    return Err(ErroRuntime::VariavelImutavel(nome.to_owned()));
                }
                celula.valor = valor;
                return Ok(());
            }
        }
        Err(ErroRuntime::NomeNaoEncontrado(nome.to_owned()))
    }

    /// Atualiza um elemento de uma lista vinculada a uma variável mutável.
    pub fn definir_elemento_lista(
        &mut self,
        nome: &str,
        indice: usize,
        valor: Valor,
    ) -> Resultado<()> {
        let celula = self
            .escopos
            .iter()
            .rev()
            .find_map(|escopo| escopo.get_mut(nome))
            .ok_or_else(|| ErroRuntime::NomeNaoEncontrado(nome.to_owned()))?;

        if !celula.mutavel {
            return Err(ErroRuntime::VariavelImutavel(nome.to_owned()));
        }

        let Valor::Lista(lista) = &celula.valor else {
            return Err(ErroRuntime::TipoIncompativel(
                "a variável não contém uma lista",
            ));
        };

        let mut itens = lista.write().map_err(|_| ErroRuntime::MemoriaCorrompida)?;
        let tamanho = itens.len();
        let destino = itens
            .get_mut(indice)
            .ok_or(ErroRuntime::IndiceForaDosLimites { indice, tamanho })?;
        *destino = valor;
        Ok(())
    }

    /// Lê um elemento de lista sem expor referências internas.
    pub fn elemento_lista(&self, nome: &str, indice: usize) -> Resultado<Valor> {
        let valor = self.obter(nome)?;
        let Valor::Lista(lista) = valor else {
            return Err(ErroRuntime::TipoIncompativel(
                "a variável não contém uma lista",
            ));
        };
        let itens = lista.read().map_err(|_| ErroRuntime::MemoriaCorrompida)?;
        itens
            .get(indice)
            .cloned()
            .ok_or(ErroRuntime::IndiceForaDosLimites {
                indice,
                tamanho: itens.len(),
            })
    }

    /// Retorna a quantidade de elementos de uma lista.
    pub fn comprimento_lista(&self, nome: &str) -> Resultado<usize> {
        let valor = self.obter(nome)?;
        let Valor::Lista(lista) = valor else {
            return Err(ErroRuntime::TipoIncompativel(
                "a variável não contém uma lista",
            ));
        };
        let itens = lista
            .read()
            .map_err(|_| ErroRuntime::MemoriaCorrompida)?;
        Ok(itens.len())
    }
}

/// Retorna a quantidade de caracteres Unicode de um texto, não seus bytes.
#[must_use]
pub fn comprimento_texto(texto: &str) -> usize {
    texto.chars().count()
}

/// Obtém o caractere Unicode na posição informada.
pub fn caractere_texto(texto: &str, indice: usize) -> Resultado<char> {
    texto
        .chars()
        .nth(indice)
        .ok_or(ErroRuntime::IndiceForaDosLimites {
            indice,
            tamanho: comprimento_texto(texto),
        })
}

/// Executa uma operação inteira com overflow e divisão por zero verificados.
pub fn operar_inteiro(operacao: OperacaoInteira, esquerda: i128, direita: i128) -> Resultado<i128> {
    match operacao {
        OperacaoInteira::Soma => esquerda.checked_add(direita),
        OperacaoInteira::Subtracao => esquerda.checked_sub(direita),
        OperacaoInteira::Multiplicacao => esquerda.checked_mul(direita),
        OperacaoInteira::Divisao => {
            if direita == 0 {
                return Err(ErroRuntime::DivisaoPorZero);
            }
            esquerda.checked_div(direita)
        }
        OperacaoInteira::Resto => {
            if direita == 0 {
                return Err(ErroRuntime::DivisaoPorZero);
            }
            esquerda.checked_rem(direita)
        }
        OperacaoInteira::Potencia => {
            let expoente = u32::try_from(direita).map_err(|_| ErroRuntime::ExpoenteNegativo)?;
            esquerda.checked_pow(expoente)
        }
    }
    .ok_or(ErroRuntime::Overflow)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn escopos_sombreiam_e_restauram_variaveis() {
        let mut memoria = Memoria::nova();
        memoria.declarar("valor", Valor::Inteiro(10), true).unwrap();
        memoria.abrir_escopo();
        memoria.declarar("valor", Valor::Inteiro(20), true).unwrap();
        assert_eq!(memoria.obter("valor"), Ok(Valor::Inteiro(20)));
        memoria.fechar_escopo().unwrap();
        assert_eq!(memoria.obter("valor"), Ok(Valor::Inteiro(10)));
    }

    #[test]
    fn impede_reatribuicao_de_constante() {
        let mut memoria = Memoria::nova();
        memoria
            .declarar("limite", Valor::Inteiro(9), false)
            .unwrap();
        assert_eq!(
            memoria.atribuir("limite", Valor::Inteiro(10)),
            Err(ErroRuntime::VariavelImutavel("limite".into()))
        );
    }

    #[test]
    fn impede_declaracao_duplicada_no_mesmo_escopo() {
        let mut memoria = Memoria::nova();
        memoria.declarar("x", Valor::Inteiro(1), true).unwrap();
        assert_eq!(
            memoria.declarar("x", Valor::Inteiro(2), true),
            Err(ErroRuntime::NomeDuplicado("x".into()))
        );
    }

    #[test]
    fn nao_fecha_escopo_global() {
        let mut memoria = Memoria::nova();
        assert_eq!(memoria.fechar_escopo(), Err(ErroRuntime::EscopoGlobal));
        assert_eq!(memoria.quantidade_escopos(), 1);
    }

    #[test]
    fn listas_compartilhadas_podem_ser_lidas_e_alteradas() {
        let lista = Valor::Lista(Arc::new(RwLock::new(vec![
            Valor::Inteiro(1),
            Valor::Inteiro(2),
        ])));
        let mut memoria = Memoria::nova();
        memoria.declarar("itens", lista, true).unwrap();
        memoria
            .definir_elemento_lista("itens", 1, Valor::Inteiro(7))
            .unwrap();
        assert_eq!(memoria.elemento_lista("itens", 1), Ok(Valor::Inteiro(7)));
        assert_eq!(memoria.comprimento_lista("itens"), Ok(2));
    }

    #[test]
    fn rejeita_indice_invalido() {
        let lista = Valor::Lista(Arc::new(RwLock::new(vec![Valor::Inteiro(1)])));
        let mut memoria = Memoria::nova();
        memoria.declarar("itens", lista, true).unwrap();
        assert_eq!(
            memoria.elemento_lista("itens", 2),
            Err(ErroRuntime::IndiceForaDosLimites {
                indice: 2,
                tamanho: 1
            })
        );
    }

    #[test]
    fn texto_conta_caracteres_unicode() {
        assert_eq!(comprimento_texto("ação 🌌"), 6);
        assert_eq!(caractere_texto("ação 🌌", 4), Ok('🌌'));
    }

    #[test]
    fn aritmetica_detecta_overflow_e_divisao_por_zero() {
        assert_eq!(
            operar_inteiro(OperacaoInteira::Soma, i128::MAX, 1),
            Err(ErroRuntime::Overflow)
        );
        assert_eq!(
            operar_inteiro(OperacaoInteira::Divisao, 10, 0),
            Err(ErroRuntime::DivisaoPorZero)
        );
        assert_eq!(operar_inteiro(OperacaoInteira::Potencia, 2, 10), Ok(1024));
    }

    #[test]
    fn lista_em_variavel_constante_nao_pode_ser_alterada() {
        let lista = Valor::Lista(Arc::new(RwLock::new(vec![Valor::Inteiro(1)])));
        let mut memoria = Memoria::nova();
        memoria.declarar("fixa", lista, false).unwrap();
        assert_eq!(
            memoria.definir_elemento_lista("fixa", 0, Valor::Inteiro(2)),
            Err(ErroRuntime::VariavelImutavel("fixa".into()))
        );
    }
}
