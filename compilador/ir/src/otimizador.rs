//! Otimizador conservador da IR da Nebula.
//!
//! Os níveis são cumulativos: O1 dobra constantes; O2 simplifica fluxo
//! comprovadamente constante; O3 remove instruções inalcançáveis; Os também
//! remove blocos vazios. Nenhuma transformação elimina expressões com efeitos.

use crate::{ExpressaoIr, InstrucaoIr, OperadorIr, ProgramaIr, ValorIr};

/// Nível de otimização solicitado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NivelOtimizacao {
    /// Preserva a IR sem transformações.
    O0,
    /// Executa dobramento conservador de constantes.
    O1,
    /// Simplifica ramos e laços com condições constantes.
    O2,
    /// Remove instruções posteriores a transferências incondicionais.
    O3,
    /// Aplica O3 e remove blocos vazios.
    Os,
}

/// Estatísticas das transformações efetivamente realizadas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RelatorioOtimizacao {
    /// Quantidade de expressões substituídas por constantes.
    pub constantes_dobradas: usize,
    /// Quantidade de ramos condicionais simplificados.
    pub ramos_simplificados: usize,
    /// Quantidade de laços comprovadamente inexecutáveis removidos.
    pub lacos_removidos: usize,
    /// Quantidade de instruções inalcançáveis removidas.
    pub instrucoes_inalcancaveis: usize,
    /// Quantidade de blocos vazios removidos.
    pub blocos_vazios_removidos: usize,
}

/// Otimiza um programa IR sem executar código nem remover efeitos observáveis.
///
/// Cada nível inclui as transformações dos níveis anteriores. Operações
/// aritméticas que podem produzir overflow, divisão por zero ou valores
/// decimais não finitos permanecem na IR para preservar o comportamento.
pub fn otimizar(
    programa: &mut ProgramaIr,
    nivel: NivelOtimizacao,
) -> Result<RelatorioOtimizacao, String> {
    let mut relatorio = RelatorioOtimizacao::default();
    if nivel == NivelOtimizacao::O0 {
        return Ok(relatorio);
    }

    otimizar_lista(&mut programa.instrucoes, nivel, &mut relatorio);
    for funcao in programa.funcoes.values_mut() {
        otimizar_lista(&mut funcao.corpo, nivel, &mut relatorio);
    }
    Ok(relatorio)
}

fn otimizar_lista(
    instrucoes: &mut Vec<InstrucaoIr>,
    nivel: NivelOtimizacao,
    relatorio: &mut RelatorioOtimizacao,
) {
    let mut indice = 0;
    while indice < instrucoes.len() {
        otimizar_instrucao(&mut instrucoes[indice], nivel, relatorio);

        if nivel != NivelOtimizacao::O1 {
            let ramo = match &instrucoes[indice] {
                InstrucaoIr::Se {
                    condicao: ExpressaoIr::Constante(ValorIr::Logico(condicao)),
                    entao,
                    senao,
                } => Some(if *condicao { entao.clone() } else { senao.clone() }),
                _ => None,
            };
            if let Some(escolhido) = ramo {
                instrucoes[indice] = InstrucaoIr::Bloco(escolhido);
                relatorio.ramos_simplificados += 1;
            } else if matches!(
                &instrucoes[indice],
                InstrucaoIr::Enquanto {
                    condicao: ExpressaoIr::Constante(ValorIr::Logico(false)),
                    ..
                }
            ) {
                instrucoes.remove(indice);
                relatorio.lacos_removidos += 1;
                continue;
            } else {
                let para_inexecutavel = matches!(
                    &instrucoes[indice],
                    InstrucaoIr::Para {
                        condicao: Some(ExpressaoIr::Constante(ValorIr::Logico(false))),
                        ..
                    }
                );
                if para_inexecutavel {
                    let inicial = match &mut instrucoes[indice] {
                        InstrucaoIr::Para { inicial, .. } => inicial.take(),
                        _ => None,
                    };
                    if let Some(inicial) = inicial {
                        instrucoes[indice] = InstrucaoIr::Bloco(vec![*inicial]);
                    } else {
                        instrucoes.remove(indice);
                        relatorio.lacos_removidos += 1;
                        continue;
                    }
                    relatorio.lacos_removidos += 1;
                }
            }
        }

        if nivel == NivelOtimizacao::Os
            && matches!(&instrucoes[indice], InstrucaoIr::Bloco(itens) if itens.is_empty())
        {
            instrucoes.remove(indice);
            relatorio.blocos_vazios_removidos += 1;
            continue;
        }

        if matches!(nivel, NivelOtimizacao::O3 | NivelOtimizacao::Os)
            && termina_fluxo(&instrucoes[indice])
        {
            let removidas = instrucoes.len() - indice - 1;
            instrucoes.truncate(indice + 1);
            relatorio.instrucoes_inalcancaveis += removidas;
            break;
        }

        indice += 1;
    }
}

fn otimizar_instrucao(
    instrucao: &mut InstrucaoIr,
    nivel: NivelOtimizacao,
    relatorio: &mut RelatorioOtimizacao,
) {
    match instrucao {
        InstrucaoIr::Declarar { valor, .. } | InstrucaoIr::Expressao(valor) => {
            otimizar_expressao(valor, relatorio);
        }
        InstrucaoIr::Bloco(itens) => otimizar_lista(itens, nivel, relatorio),
        InstrucaoIr::Se { condicao, entao, senao } => {
            otimizar_expressao(condicao, relatorio);
            otimizar_lista(entao, nivel, relatorio);
            otimizar_lista(senao, nivel, relatorio);
        }
        InstrucaoIr::Enquanto { condicao, corpo } => {
            otimizar_expressao(condicao, relatorio);
            otimizar_lista(corpo, nivel, relatorio);
        }
        InstrucaoIr::Para { inicial, condicao, atualizacao, corpo } => {
            if let Some(inicial) = inicial {
                otimizar_instrucao(inicial, nivel, relatorio);
            }
            if let Some(condicao) = condicao {
                otimizar_expressao(condicao, relatorio);
            }
            if let Some(atualizacao) = atualizacao {
                otimizar_expressao(atualizacao, relatorio);
            }
            otimizar_lista(corpo, nivel, relatorio);
        }
        InstrucaoIr::Devolver(valor) => {
            if let Some(valor) = valor {
                otimizar_expressao(valor, relatorio);
            }
        }
        InstrucaoIr::Parar | InstrucaoIr::Seguir => {}
    }
}

fn termina_fluxo(instrucao: &InstrucaoIr) -> bool {
    match instrucao {
        InstrucaoIr::Devolver(_) | InstrucaoIr::Parar | InstrucaoIr::Seguir => true,
        InstrucaoIr::Bloco(itens) => itens.last().is_some_and(termina_fluxo),
        InstrucaoIr::Se { entao, senao, .. } => {
            !senao.is_empty()
                && entao.last().is_some_and(termina_fluxo)
                && senao.last().is_some_and(termina_fluxo)
        }
        _ => false,
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
        (OperadorIr::Negacao, ValorIr::Inteiro(v)) => v.checked_neg().and_then(inteiro),
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
        (V::Inteiro(a), O::Mais, V::Inteiro(b)) => a.checked_add(*b).and_then(inteiro),
        (V::Inteiro(a), O::Menos, V::Inteiro(b)) => a.checked_sub(*b).and_then(inteiro),
        (V::Inteiro(a), O::Vezes, V::Inteiro(b)) => a.checked_mul(*b).and_then(inteiro),
        (V::Inteiro(a), O::Divide, V::Inteiro(b)) => a.checked_div(*b).and_then(inteiro),
        (V::Inteiro(a), O::Resto, V::Inteiro(b)) => a.checked_rem(*b).and_then(inteiro),
        (V::Inteiro(a), O::Potencia, V::Inteiro(b)) if (0..=u32::MAX as i128).contains(b) => {
            a.checked_pow(*b as u32).and_then(inteiro)
        }
        (V::Inteiro(a), O::Intervalo, V::Inteiro(b)) => Some(V::Intervalo(*a, *b)),
        (V::Inteiro(a), O::Menor, V::Inteiro(b)) => Some(V::Logico(a < b)),
        (V::Inteiro(a), O::MenorIgual, V::Inteiro(b)) => Some(V::Logico(a <= b)),
        (V::Inteiro(a), O::Maior, V::Inteiro(b)) => Some(V::Logico(a > b)),
        (V::Inteiro(a), O::MaiorIgual, V::Inteiro(b)) => Some(V::Logico(a >= b)),
        (V::Decimal(a), O::Mais, V::Decimal(b)) => decimal(*a + *b),
        (V::Decimal(a), O::Menos, V::Decimal(b)) => decimal(*a - *b),
        (V::Decimal(a), O::Vezes, V::Decimal(b)) => decimal(*a * *b),
        (V::Decimal(a), O::Divide, V::Decimal(b)) if *b != 0.0 => decimal(*a / *b),
        (V::Decimal(a), O::Resto, V::Decimal(b)) if *b != 0.0 => decimal(*a % *b),
        (V::Decimal(a), O::Potencia, V::Decimal(b)) => decimal((*a).powf(*b)),
        (V::Decimal(a), O::Intervalo, V::Decimal(b)) => Some(V::IntervaloDecimal(*a, *b)),
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

fn inteiro(valor: i128) -> Option<ValorIr> {
    (i64::MIN as i128..=i64::MAX as i128)
        .contains(&valor)
        .then_some(ValorIr::Inteiro(valor))
}

fn decimal(valor: f64) -> Option<ValorIr> {
    valor.is_finite().then_some(ValorIr::Decimal(valor))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::FuncaoIr;
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
    fn o0_nao_modifica_a_ir() {
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
    fn o2_simplifica_ramo_constante_preservando_escopo() {
        let mut ir = programa(ExpressaoIr::Constante(ValorIr::Logico(true)));
        ir.instrucoes[0] = InstrucaoIr::Se {
            condicao: ExpressaoIr::Constante(ValorIr::Logico(true)),
            entao: vec![InstrucaoIr::Expressao(ExpressaoIr::Constante(ValorIr::Inteiro(1)))],
            senao: vec![InstrucaoIr::Expressao(ExpressaoIr::Constante(ValorIr::Inteiro(2)))],
        };
        let relatorio = otimizar(&mut ir, NivelOtimizacao::O2).expect("O2");
        assert_eq!(relatorio.ramos_simplificados, 1);
        assert!(matches!(&ir.instrucoes[0], InstrucaoIr::Bloco(itens) if itens.len() == 1 && matches!(itens[0], InstrucaoIr::Expressao(ExpressaoIr::Constante(ValorIr::Inteiro(1))))));
    }

    #[test]
    fn o2_remove_laco_enquanto_com_condicao_falsa() {
        let mut ir = programa(ExpressaoIr::Constante(ValorIr::Logico(false)));
        ir.instrucoes[0] = InstrucaoIr::Enquanto {
            condicao: ExpressaoIr::Constante(ValorIr::Logico(false)),
            corpo: vec![InstrucaoIr::Parar],
        };
        let relatorio = otimizar(&mut ir, NivelOtimizacao::O2).expect("O2");
        assert_eq!(relatorio.lacos_removidos, 1);
        assert!(ir.instrucoes.is_empty());
    }

    #[test]
    fn o3_remove_instrucoes_apos_retorno() {
        let mut ir = programa(ExpressaoIr::Constante(ValorIr::Inteiro(1)));
        ir.instrucoes = vec![
            InstrucaoIr::Devolver(Some(ExpressaoIr::Constante(ValorIr::Inteiro(1)))),
            InstrucaoIr::Expressao(ExpressaoIr::Chamada { nome: "efeito".into(), argumentos: vec![] }),
        ];
        let relatorio = otimizar(&mut ir, NivelOtimizacao::O3).expect("O3");
        assert_eq!(relatorio.instrucoes_inalcancaveis, 1);
        assert_eq!(ir.instrucoes.len(), 1);
    }

    #[test]
    fn os_remove_blocos_vazios() {
        let mut ir = programa(ExpressaoIr::Constante(ValorIr::Inteiro(1)));
        ir.instrucoes[0] = InstrucaoIr::Bloco(vec![]);
        let relatorio = otimizar(&mut ir, NivelOtimizacao::Os).expect("Os");
        assert_eq!(relatorio.blocos_vazios_removidos, 1);
        assert!(ir.instrucoes.is_empty());
    }
}
