//! Otimizador conservador da IR da Nebula.
 //!
 //! A primeira etapa disponibiliza O0 (sem transformações) e O1 (dobramento
 //! de constantes comprovadamente seguro). Níveis mais agressivos permanecem
 //! explicitamente indisponíveis até possuírem passes e provas de equivalência.

use crate::{ExpressaoIr, InstrucaoIr, OperadorIr, ProgramaIr, ValorIr};

/// Nível de otimização solicitado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NivelOtimizacao {
    /// Preserva a IR sem transformações.
    O0,
    /// Executa dobramento conservador de constantes.
    O1,
    /// Reservado para passes avançados ainda não implementados.
    O2,
    /// Reservado para passes agressivos ainda não implementados.
    O3,
    /// Reservado para otimização de tamanho ainda não implementada.
    Os,
}

/// Estatísticas das transformações efetivamente realizadas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RelatorioOtimizacao {
    /// Quantidade de expressões substituídas por constantes.
    pub constantes_dobradas: usize,
}

/// Otimiza um programa IR sem executar código nem remover efeitos observáveis.
///
/// O1 só substitui expressões cujos operandos são constantes e cuja operação
/// pode ser calculada sem overflow, divisão por zero ou resultado não finito.
pub fn otimizar(
    programa: &mut ProgramaIr,
    nivel: NivelOtimizacao,
) -> Result<RelatorioOtimizacao, String> {
    match nivel {
        NivelOtimizacao::O0 => Ok(RelatorioOtimizacao::default()),
        NivelOtimizacao::O1 => {
            let mut relatorio = RelatorioOtimizacao::default();
            for instrucao in &mut programa.instrucoes {
                otimizar_instrucao(instrucao, &mut relatorio);
            }
            for funcao in programa.funcoes.values_mut() {
                for instrucao in &mut funcao.corpo {
                    otimizar_instrucao(instrucao, &mut relatorio);
                }
            }
            Ok(relatorio)
        }
        NivelOtimizacao::O2 | NivelOtimizacao::O3 | NivelOtimizacao::Os => {
            Err("Este nível de otimização ainda não está disponível; use O0 ou O1.".into())
        }
    }
}

fn otimizar_instrucao(instrucao: &mut InstrucaoIr, relatorio: &mut RelatorioOtimizacao) {
    match instrucao {
        InstrucaoIr::Declarar { valor, .. } | InstrucaoIr::Expressao(valor) => {
            otimizar_expressao(valor, relatorio);
        }
        InstrucaoIr::Se { condicao, entao, senao } => {
            otimizar_expressao(condicao, relatorio);
            for item in entao.iter_mut().chain(senao.iter_mut()) {
                otimizar_instrucao(item, relatorio);
            }
        }
        InstrucaoIr::Enquanto { condicao, corpo } => {
            otimizar_expressao(condicao, relatorio);
            for item in corpo {
                otimizar_instrucao(item, relatorio);
            }
        }
        InstrucaoIr::Para { inicial, condicao, atualizacao, corpo } => {
            if let Some(inicial) = inicial {
                otimizar_instrucao(inicial, relatorio);
            }
            if let Some(condicao) = condicao {
                otimizar_expressao(condicao, relatorio);
            }
            if let Some(atualizacao) = atualizacao {
                otimizar_expressao(atualizacao, relatorio);
            }
            for item in corpo {
                otimizar_instrucao(item, relatorio);
            }
        }
        InstrucaoIr::Devolver(valor) => {
            if let Some(valor) = valor {
                otimizar_expressao(valor, relatorio);
            }
        }
        InstrucaoIr::Parar | InstrucaoIr::Seguir => {}
    }
}

fn otimizar_expressao(expressao: &mut ExpressaoIr, relatorio: &mut RelatorioOtimizacao) {
    match expressao {
        ExpressaoIr::Lista(itens) => {
            for item in itens {
                otimizar_expressao(item, relatorio);
            }
        }
        ExpressaoIr::Unaria { valor, .. } => otimizar_expressao(valor, relatorio),
        ExpressaoIr::Binaria { esquerda, direita, .. } => {
            otimizar_expressao(esquerda, relatorio);
            otimizar_expressao(direita, relatorio);
        }
        ExpressaoIr::Atribuicao { valor, .. } => otimizar_expressao(valor, relatorio),
        ExpressaoIr::Chamada { argumentos, .. }
        | ExpressaoIr::ChamadaQualificada { argumentos, .. } => {
            for argumento in argumentos {
                otimizar_expressao(argumento, relatorio);
            }
        }
        ExpressaoIr::Indexacao { alvo, indice } => {
            otimizar_expressao(alvo, relatorio);
            otimizar_expressao(indice, relatorio);
        }
        ExpressaoIr::Constante(_) | ExpressaoIr::Variavel(_) => {}
    }

    let dobrada = match expressao {
        ExpressaoIr::Unaria { operador, valor } => {
            if let ExpressaoIr::Constante(valor) = valor.as_ref() {
                dobrar_unaria(*operador, valor)
            } else {
                None
            }
        }
        ExpressaoIr::Binaria { esquerda, operador, direita } => {
            if let (ExpressaoIr::Constante(a), ExpressaoIr::Constante(b)) =
                (esquerda.as_ref(), direita.as_ref())
            {
                dobrar_binaria(a, *operador, b)
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some(valor) = dobrada {
        *expressao = ExpressaoIr::Constante(valor);
        relatorio.constantes_dobradas += 1;
    }
}

fn dobrar_unaria(operador: OperadorIr, valor: &ValorIr) -> Option<ValorIr> {
    match (operador, valor) {
        (OperadorIr::Negacao, ValorIr::Inteiro(v)) => v.checked_neg().map(ValorIr::Inteiro),
        (OperadorIr::Negacao, ValorIr::Decimal(v)) => {
            let resultado = -*v;
            resultado.is_finite().then_some(ValorIr::Decimal(resultado))
        }
        (OperadorIr::Nao, ValorIr::Logico(v)) => Some(ValorIr::Logico(!v)),
        _ => None,
    }
}

fn dobrar_binaria(a: &ValorIr, op: OperadorIr, b: &ValorIr) -> Option<ValorIr> {
    use OperadorIr as O;
    use ValorIr as V;

    match (a, op, b) {
        (V::Inteiro(a), O::Mais, V::Inteiro(b)) => a.checked_add(*b).map(V::Inteiro),
        (V::Inteiro(a), O::Menos, V::Inteiro(b)) => a.checked_sub(*b).map(V::Inteiro),
        (V::Inteiro(a), O::Vezes, V::Inteiro(b)) => a.checked_mul(*b).map(V::Inteiro),
        (V::Inteiro(a), O::Divide, V::Inteiro(b)) => a.checked_div(*b).map(V::Inteiro),
        (V::Inteiro(a), O::Resto, V::Inteiro(b)) => a.checked_rem(*b).map(V::Inteiro),
        (V::Inteiro(a), O::Potencia, V::Inteiro(b)) if (0..=u32::MAX as i128).contains(b) => {
            a.checked_pow(*b as u32).map(V::Inteiro)
        }
        (V::Inteiro(a), O::Intervalo, V::Inteiro(b)) => Some(V::Intervalo(*a, *b)),
        (V::Inteiro(a), O::Menor, V::Inteiro(b)) => Some(V::Logico(a < b)),
        (V::Inteiro(a), O::MenorIgual, V::Inteiro(b)) => Some(V::Logico(a <= b)),
        (V::Inteiro(a), O::Maior, V::Inteiro(b)) => Some(V::Logico(a > b)),
        (V::Inteiro(a), O::MaiorIgual, V::Inteiro(b)) => Some(V::Logico(a >= b)),
        (V::Decimal(a), O::Mais, V::Decimal(b)) => decimal(a + b),
        (V::Decimal(a), O::Menos, V::Decimal(b)) => decimal(a - b),
        (V::Decimal(a), O::Vezes, V::Decimal(b)) => decimal(a * b),
        (V::Decimal(a), O::Divide, V::Decimal(b)) if *b != 0.0 => decimal(a / b),
        (V::Decimal(a), O::Resto, V::Decimal(b)) if *b != 0.0 => decimal(a % b),
        (V::Decimal(a), O::Potencia, V::Decimal(b)) => decimal(a.powf(*b)),
        (V::Decimal(a), O::Intervalo, V::Decimal(b)) => {
            Some(V::IntervaloDecimal(*a, *b))
        }
        (V::Decimal(a), O::Menor, V::Decimal(b)) => Some(V::Logico(a < b)),
        (V::Decimal(a), O::MenorIgual, V::Decimal(b)) => Some(V::Logico(a <= b)),
        (V::Decimal(a), O::Maior, V::Decimal(b)) => Some(V::Logico(a > b)),
        (V::Decimal(a), O::MaiorIgual, V::Decimal(b)) => Some(V::Logico(a >= b)),
        (V::Logico(a), O::E, V::Logico(b)) => Some(V::Logico(*a && *b)),
        (V::Logico(a), O::Ou, V::Logico(b)) => Some(V::Logico(*a || *b)),
        (V::Texto(a), O::Mais, V::Texto(b)) => Some(V::Texto(format!("{a}{b}"))),
        (a, O::Igual, b) => Some(V::Logico(a == b)),
        (a, O::Diferente, b) => Some(V::Logico(a != b)),
        _ => None,
    }
}

fn decimal(valor: f64) -> Option<ValorIr> {
    valor.is_finite().then_some(ValorIr::Decimal(valor))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::{FuncaoIr, Tipo};
    use std::collections::HashMap;

    fn programa(expressao: ExpressaoIr) -> ProgramaIr {
        ProgramaIr {
            instrucoes: vec![InstrucaoIr::Expressao(expressao)],
            funcoes: HashMap::new(),
            modulos: HashMap::new(),
        }
    }

    #[test]
    fn dobra_aritmetica_inteira_sem_overflow() {
        let mut ir = programa(ExpressaoIr::Binaria {
            esquerda: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(20))),
            operador: OperadorIr::Mais,
            direita: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(22))),
        });
        let relatorio = otimizar(&mut ir, NivelOtimizacao::O1).expect("O1");
        assert_eq!(relatorio.constantes_dobradas, 1);
        assert_eq!(ir.instrucoes, vec![InstrucaoIr::Expressao(
            ExpressaoIr::Constante(ValorIr::Inteiro(42))
        )]);
    }

    #[test]
    fn preserva_operacao_com_overflow() {
        let mut ir = programa(ExpressaoIr::Binaria {
            esquerda: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(i128::MAX))),
            operador: OperadorIr::Mais,
            direita: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(1))),
        });
        otimizar(&mut ir, NivelOtimizacao::O1).expect("O1");
        assert!(matches!(ir.instrucoes[0], InstrucaoIr::Expressao(
            ExpressaoIr::Binaria { .. }
        )));
    }

    #[test]
    fn nao_dobra_divisao_por_zero() {
        let mut ir = programa(ExpressaoIr::Binaria {
            esquerda: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(8))),
            operador: OperadorIr::Divide,
            direita: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(0))),
        });
        otimizar(&mut ir, NivelOtimizacao::O1).expect("O1");
        assert!(matches!(ir.instrucoes[0], InstrucaoIr::Expressao(
            ExpressaoIr::Binaria { .. }
        )));
    }

    #[test]
    fn dobra_expressoes_aninhadas() {
        let mut ir = programa(ExpressaoIr::Binaria {
            esquerda: Box::new(ExpressaoIr::Binaria {
                esquerda: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(2))),
                operador: OperadorIr::Vezes,
                direita: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(3))),
            }),
            operador: OperadorIr::Mais,
            direita: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(4))),
        });
        otimizar(&mut ir, NivelOtimizacao::O1).expect("O1");
        assert!(matches!(&ir.instrucoes[0], InstrucaoIr::Expressao(
            ExpressaoIr::Constante(ValorIr::Inteiro(10))
        )));
    }

    #[test]
    fn O0_nao_modifica_a_ir() {
        let mut ir = programa(ExpressaoIr::Binaria {
            esquerda: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(1))),
            operador: OperadorIr::Mais,
            direita: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(2))),
        });
        let original = ir.clone();
        otimizar(&mut ir, NivelOtimizacao::O0).expect("O0");
        assert_eq!(ir, original);
    }

    #[test]
    fn rejeita_niveis_ainda_nao_implementados() {
        let mut ir = ProgramaIr {
            instrucoes: Vec::new(),
            funcoes: HashMap::<String, FuncaoIr>::new(),
            modulos: HashMap::new(),
        };
        assert!(otimizar(&mut ir, NivelOtimizacao::O2).is_err());
        assert!(otimizar(&mut ir, NivelOtimizacao::O3).is_err());
        assert!(otimizar(&mut ir, NivelOtimizacao::Os).is_err());
    }

    #[test]
    fn dobra_logica_sem_eliminar_avaliacao() {
        let mut ir = programa(ExpressaoIr::Binaria {
            esquerda: Box::new(ExpressaoIr::Constante(ValorIr::Logico(true))),
            operador: OperadorIr::E,
            direita: Box::new(ExpressaoIr::Constante(ValorIr::Logico(false))),
        });
        otimizar(&mut ir, NivelOtimizacao::O1).expect("O1");
        assert!(matches!(&ir.instrucoes[0], InstrucaoIr::Expressao(
            ExpressaoIr::Constante(ValorIr::Logico(false))
        )));
    }
}
