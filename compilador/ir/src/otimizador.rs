//! Otimizador conservador da IR da Nebula.
//!
//! Os níveis são cumulativos: O1 dobra constantes; O2 simplifica fluxo
//! comprovadamente constante; O3 remove instruções inalcançáveis; Os também
//! remove blocos vazios. Nenhuma transformação elimina expressões com efeitos.

use crate::{ExpressaoIr, InstrucaoIr, OperadorIr, ProgramaIr, ValorIr};
use std::collections::HashMap;

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
    /// Quantidade de leituras substituídas por constantes imutáveis locais.
    pub propagacoes_constantes: usize,
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
    if matches!(nivel, NivelOtimizacao::O2 | NivelOtimizacao::O3 | NivelOtimizacao::Os) {
        propagar_constantes_lista(&mut programa.instrucoes, &mut HashMap::new(), &mut relatorio);
        for funcao in programa.funcoes.values_mut() {
            propagar_constantes_lista(&mut funcao.corpo, &mut HashMap::new(), &mut relatorio);
        }
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

// Propagação limitada a constantes literais/expressões já constantes e ao
// fluxo sequencial do mesmo escopo. Ramos e laços recebem cópias do estado,
// evitando propagar fatos que não sejam verdadeiros em todos os caminhos.
fn propagar_constantes_lista(
    instrucoes: &mut [InstrucaoIr],
    constantes: &mut HashMap<String, ValorIr>,
    relatorio: &mut RelatorioOtimizacao,
) {
    for instrucao in instrucoes {
        match instrucao {
            InstrucaoIr::Declarar { nome, valor, constante, .. } => {
                propagar_constantes_expressao(valor, constantes, relatorio);
                constantes.remove(nome);
                if *constante {
                    if let ExpressaoIr::Constante(valor) = valor {
                        constantes.insert(nome.clone(), valor.clone());
                    }
                }
            }
            InstrucaoIr::Expressao(expressao) => {
                propagar_constantes_expressao(expressao, constantes, relatorio);
            }
            InstrucaoIr::Bloco(itens) => {
                let mut internos = constantes.clone();
                propagar_constantes_lista(itens, &mut internos, relatorio);
            }
            InstrucaoIr::Escolha { valor, casos } => {
                propagar_constantes_expressao(valor, constantes, relatorio);
                for caso in casos {
                    let mut estado_caso = constantes.clone();
                    propagar_constantes_lista(&mut caso.corpo, &mut estado_caso, relatorio);
                }
            }
            InstrucaoIr::Se { condicao, entao, senao } => {
                propagar_constantes_expressao(condicao, constantes, relatorio);
                let mut estado_entao = constantes.clone();
                let mut estado_senao = constantes.clone();
                propagar_constantes_lista(entao, &mut estado_entao, relatorio);
                propagar_constantes_lista(senao, &mut estado_senao, relatorio);
            }
            InstrucaoIr::Enquanto { condicao, corpo } => {
                propagar_constantes_expressao(condicao, constantes, relatorio);
                let mut estado_laco = constantes.clone();
                propagar_constantes_lista(corpo, &mut estado_laco, relatorio);
            }
            InstrucaoIr::Para { inicial, condicao, atualizacao, corpo } => {
                let mut estado_laco = constantes.clone();
                if let Some(inicial) = inicial {
                    propagar_constantes_lista(std::slice::from_mut(inicial.as_mut()), &mut estado_laco, relatorio);
                }
                if let Some(condicao) = condicao {
                    propagar_constantes_expressao(condicao, &mut estado_laco, relatorio);
                }
                if let Some(atualizacao) = atualizacao {
                    propagar_constantes_expressao(atualizacao, &mut estado_laco, relatorio);
                }
                propagar_constantes_lista(corpo, &mut estado_laco, relatorio);
            }
            InstrucaoIr::Devolver(Some(expressao)) => {
                propagar_constantes_expressao(expressao, constantes, relatorio);
            }
            InstrucaoIr::Devolver(None) | InstrucaoIr::Parar | InstrucaoIr::Seguir => {}
        }
    }
}

fn propagar_constantes_expressao(
    expressao: &mut ExpressaoIr,
    constantes: &mut HashMap<String, ValorIr>,
    relatorio: &mut RelatorioOtimizacao,
) {
    match expressao {
        ExpressaoIr::Variavel(nome) => {
            if let Some(valor) = constantes.get(nome) {
                *expressao = ExpressaoIr::Constante(valor.clone());
                relatorio.propagacoes_constantes += 1;
            }
        }
        ExpressaoIr::Lista(itens) | ExpressaoIr::Tupla(itens) => {
            for item in itens { propagar_constantes_expressao(item, constantes, relatorio); }
        }
        ExpressaoIr::Normalizar { valor, .. } | ExpressaoIr::Conversao { valor, .. } | ExpressaoIr::Unaria { valor, .. } => propagar_constantes_expressao(valor, constantes, relatorio),
        ExpressaoIr::ChamadaIndireta { alvo, argumentos } => {
            propagar_constantes_expressao(alvo, constantes, relatorio);
            for argumento in argumentos { propagar_constantes_expressao(argumento, constantes, relatorio); }
        }
        ExpressaoIr::Binaria { esquerda, direita, .. } => {
            propagar_constantes_expressao(esquerda, constantes, relatorio);
            propagar_constantes_expressao(direita, constantes, relatorio);
        }
        ExpressaoIr::Atribuicao { nome, valor, .. } => {
            propagar_constantes_expressao(valor, constantes, relatorio);
            constantes.remove(nome);
        }
        ExpressaoIr::AtribuicaoCampo { raiz, valor, .. } => {
            propagar_constantes_expressao(valor, constantes, relatorio);
            constantes.remove(raiz);
        }
        ExpressaoIr::Chamada { argumentos, .. } | ExpressaoIr::ChamadaQualificada { argumentos, .. } => {
            for argumento in argumentos { propagar_constantes_expressao(argumento, constantes, relatorio); }
        }
        ExpressaoIr::Indexacao { alvo, indice } => {
            propagar_constantes_expressao(alvo, constantes, relatorio);
            propagar_constantes_expressao(indice, constantes, relatorio);
        }
        ExpressaoIr::Membro { alvo, .. } => {
            propagar_constantes_expressao(alvo, constantes, relatorio);
        }
        ExpressaoIr::ConstrucaoEstrutura { campos, .. } => {
            for (_, valor) in campos {
                propagar_constantes_expressao(valor, constantes, relatorio);
            }
        }
        ExpressaoIr::ConstrucaoEnumeracao { valores, .. } => {
            for valor in valores { propagar_constantes_expressao(valor, constantes, relatorio); }
        }
        ExpressaoIr::Constante(_) => {}
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
        InstrucaoIr::Escolha { valor, casos } => {
            otimizar_expressao(valor, relatorio);
            for caso in casos {
                otimizar_lista(&mut caso.corpo, nivel, relatorio);
            }
        }
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
        ExpressaoIr::Lista(itens) | ExpressaoIr::Tupla(itens) => {
            for item in itens {
                otimizar_expressao(item, relatorio);
            }
        }
        ExpressaoIr::Normalizar { valor, .. } | ExpressaoIr::Conversao { valor, .. } | ExpressaoIr::Unaria { valor, .. } => otimizar_expressao(valor, relatorio),
        ExpressaoIr::ChamadaIndireta { alvo, argumentos } => {
            otimizar_expressao(alvo, relatorio);
            for argumento in argumentos { otimizar_expressao(argumento, relatorio); }
        }
        ExpressaoIr::Binaria { esquerda, direita, .. } => {
            otimizar_expressao(esquerda, relatorio);
            otimizar_expressao(direita, relatorio);
        }
        ExpressaoIr::Atribuicao { valor, .. } | ExpressaoIr::AtribuicaoCampo { valor, .. } => otimizar_expressao(valor, relatorio),
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
        ExpressaoIr::Membro { alvo, .. } => otimizar_expressao(alvo, relatorio),
        ExpressaoIr::ConstrucaoEstrutura { campos, .. } => {
            for (_, valor) in campos {
                otimizar_expressao(valor, relatorio);
            }
        }
        ExpressaoIr::ConstrucaoEnumeracao { valores, .. } => {
            for valor in valores { otimizar_expressao(valor, relatorio); }
        }
        ExpressaoIr::Constante(_) | ExpressaoIr::Variavel(_) | ExpressaoIr::ReferenciaFuncao { .. } | ExpressaoIr::ReferenciaFuncaoQualificada { .. } => {}
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
    use std::collections::HashMap;

    fn programa(expressao: ExpressaoIr) -> ProgramaIr {
        ProgramaIr {
            instrucoes: vec![InstrucaoIr::Expressao(expressao)],
            funcoes: HashMap::new(),
            modulos: HashMap::new(),
            layouts_tipos: HashMap::new(),
        }
    }

    #[test]
    fn o2_propaga_constante_imutavel_em_fluxo_sequencial() {
        let mut ir = ProgramaIr {
            instrucoes: vec![
                InstrucaoIr::Declarar { nome: "limite".into(), tipo: nebula_semantica::Tipo::Inteiro, valor: ExpressaoIr::Constante(ValorIr::Inteiro(42)), constante: true },
                InstrucaoIr::Devolver(Some(ExpressaoIr::Variavel("limite".into()))),
            ],
            funcoes: HashMap::new(),
            modulos: HashMap::new(),
            layouts_tipos: HashMap::new(),
        };
        let relatorio = otimizar(&mut ir, NivelOtimizacao::O2).expect("O2");
        assert_eq!(relatorio.propagacoes_constantes, 1);
        assert!(matches!(&ir.instrucoes[1], InstrucaoIr::Devolver(Some(ExpressaoIr::Constante(ValorIr::Inteiro(42))))));
    }

    #[test]
    fn propagacao_nao_vaza_de_ramos_condicionais() {
        let mut ir = ProgramaIr {
            instrucoes: vec![
                InstrucaoIr::Se { condicao: ExpressaoIr::Constante(ValorIr::Logico(true)), entao: vec![
                    InstrucaoIr::Declarar { nome: "interno".into(), tipo: nebula_semantica::Tipo::Inteiro, valor: ExpressaoIr::Constante(ValorIr::Inteiro(7)), constante: true },
                ], senao: vec![] },
                InstrucaoIr::Devolver(Some(ExpressaoIr::Variavel("interno".into()))),
            ],
            funcoes: HashMap::new(),
            modulos: HashMap::new(),
            layouts_tipos: HashMap::new(),
        };
        otimizar(&mut ir, NivelOtimizacao::O2).expect("O2");
        assert!(matches!(&ir.instrucoes[1], InstrucaoIr::Devolver(Some(ExpressaoIr::Variavel(nome))) if nome == "interno"));
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
    fn o2_preserva_escopo_do_ramo_escolhido() {
        let mut ir = ProgramaIr {
            instrucoes: vec![
                InstrucaoIr::Se {
                    condicao: ExpressaoIr::Constante(ValorIr::Logico(true)),
                    entao: vec![InstrucaoIr::Declarar {
                        nome: "interno".into(),
                        tipo: nebula_semantica::Tipo::Inteiro,
                        valor: ExpressaoIr::Constante(ValorIr::Inteiro(1)),
                        constante: false,
                    }],
                    senao: vec![],
                },
                InstrucaoIr::Expressao(ExpressaoIr::Variavel("interno".into())),
            ],
            funcoes: HashMap::new(),
            modulos: HashMap::new(),
            layouts_tipos: HashMap::new(),
        };
        let original = crate::executar(&ir, 100).unwrap_err();
        otimizar(&mut ir, NivelOtimizacao::O2).expect("O2");
        let otimizado = crate::executar(&ir, 100).unwrap_err();
        assert_eq!(original, otimizado);
    }

    #[test]
    fn o2_preserva_inicializador_de_para_com_condicao_falsa() {
        let mut ir = ProgramaIr {
            instrucoes: vec![InstrucaoIr::Para {
                inicial: Some(Box::new(InstrucaoIr::Expressao(
                    ExpressaoIr::Chamada {
                        nome: "exibir".into(),
                        argumentos: vec![ExpressaoIr::Constante(ValorIr::Inteiro(7))],
                    },
                ))),
                condicao: Some(ExpressaoIr::Constante(ValorIr::Logico(false))),
                atualizacao: None,
                corpo: vec![],
            }],
            funcoes: HashMap::new(),
            modulos: HashMap::new(),
            layouts_tipos: HashMap::new(),
        };
        let original = crate::executar(&ir, 100).expect("IR original");
        otimizar(&mut ir, NivelOtimizacao::O2).expect("O2");
        let otimizado = crate::executar(&ir, 100).expect("IR otimizada");
        assert_eq!(original.saida, vec!["7"]);
        assert_eq!(otimizado.saida, original.saida);
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
