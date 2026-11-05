//! Classificação da ABI interna x86-64 da Nebula.
//!
//! Agregados pequenos são transportados em registradores. Agregados grandes
//! são passados indiretamente por endereço e retornados em destino oculto.
//! Este módulo também calcula áreas de pilha e armazenamento sem truncar valores.

#![allow(missing_docs)]

/// Representação física de um agregado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClasseAgregado {
    UmRegistrador,
    DoisRegistradores,
    Indireto { tamanho: u64, alinhamento: u64 },
}

/// Classifica o agregado pelo tamanho e valida seu alinhamento.
pub fn classificar_agregado(tamanho: u64, alinhamento: u64) -> Result<ClasseAgregado, String> {
    validar_layout(tamanho, alinhamento)?;
    Ok(match tamanho {
        1..=8 => ClasseAgregado::UmRegistrador,
        9..=16 => ClasseAgregado::DoisRegistradores,
        _ => ClasseAgregado::Indireto {
            tamanho,
            alinhamento,
        },
    })
}

/// Representação física do retorno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetornoAgregado {
    Registradores(ClasseAgregado),
    DestinoOculto { tamanho: u64, alinhamento: u64 },
}

pub fn classificar_retorno_agregado(
    tamanho: u64,
    alinhamento: u64,
) -> Result<RetornoAgregado, String> {
    Ok(match classificar_agregado(tamanho, alinhamento)? {
        ClasseAgregado::Indireto {
            tamanho,
            alinhamento,
        } => RetornoAgregado::DestinoOculto {
            tamanho,
            alinhamento,
        },
        classe => RetornoAgregado::Registradores(classe),
    })
}

/// Localização física de um argumento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalArgumento {
    Registrador(u8),
    PonteiroRegistrador(u8),
    Pilha {
        deslocamento: u64,
        tamanho: u64,
        alinhamento: u64,
    },
    PonteiroPilha {
        deslocamento: u64,
    },
}

/// Plano de passagem de argumentos e tamanho da área de pilha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanoArgumentos {
    pub argumentos: Vec<Vec<LocalArgumento>>,
    pub tamanho_pilha: u64,
    pub destino_oculto: bool,
}

/// Planeja argumentos sem dividir um agregado entre registradores e pilha.
///
/// Agregados grandes ocupam um único argumento-ponteiro. Um retorno grande
/// reserva RDI para o endereço de destino antes dos argumentos explícitos.
pub fn planejar_argumentos(
    agregados: &[(u64, u64)],
    retorno: Option<(u64, u64)>,
) -> Result<PlanoArgumentos, String> {
    let destino_oculto = match retorno {
        Some((tamanho, alinhamento)) => matches!(
            classificar_retorno_agregado(tamanho, alinhamento)?,
            RetornoAgregado::DestinoOculto { .. }
        ),
        None => false,
    };
    let mut proximo_registrador = if destino_oculto { 1usize } else { 0usize };
    let mut deslocamento_pilha = 0u64;
    let mut argumentos = Vec::with_capacity(agregados.len());

    for &(tamanho, alinhamento) in agregados {
        let classe = classificar_agregado(tamanho, alinhamento)?;
        let indireto = matches!(classe, ClasseAgregado::Indireto { .. });
        let (palavras, tamanho_pilha, alinhamento_pilha) = match classe {
            ClasseAgregado::UmRegistrador => (1usize, 8u64, 8u64),
            ClasseAgregado::DoisRegistradores => (2, 16, alinhamento.clamp(8, 16)),
            ClasseAgregado::Indireto { .. } => (1, 8, 8),
        };
        let mut locais = Vec::with_capacity(palavras);

        // O argumento agregado é indivisível: ou todos os registradores cabem,
        // ou a representação inteira vai para a área de pilha.
        if proximo_registrador
            .checked_add(palavras)
            .is_some_and(|fim| fim <= 6)
        {
            for indice in 0..palavras {
                let registrador = (proximo_registrador + indice) as u8;
                locais.push(if indireto {
                    LocalArgumento::PonteiroRegistrador(registrador)
                } else {
                    LocalArgumento::Registrador(registrador)
                });
            }
            proximo_registrador += palavras;
        } else {
            deslocamento_pilha = alinhar(deslocamento_pilha, alinhamento_pilha)?;
            if indireto {
                locais.push(LocalArgumento::PonteiroPilha {
                    deslocamento: deslocamento_pilha,
                });
            } else {
                locais.push(LocalArgumento::Pilha {
                    deslocamento: deslocamento_pilha,
                    tamanho: tamanho_pilha,
                    alinhamento: alinhamento_pilha,
                });
            }
            deslocamento_pilha = deslocamento_pilha
                .checked_add(tamanho_pilha)
                .ok_or("Área de argumentos excede o limite de endereçamento.")?;
        }
        argumentos.push(locais);
    }

    Ok(PlanoArgumentos {
        argumentos,
        tamanho_pilha: alinhar(deslocamento_pilha, 16)?,
        destino_oculto,
    })
}

/// Reserva armazenamento de um agregado no quadro de uma função.
///
/// Os deslocamentos são negativos em relação a RBP e incluem o alinhamento
/// solicitado. O chamador deve garantir que o quadro comporte o tamanho final.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlocacaoAgregado {
    pub deslocamento: i32,
    pub tamanho: u64,
    pub alinhamento: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanoArmazenamento {
    pub alocacoes: Vec<AlocacaoAgregado>,
    pub tamanho_quadro: u64,
}

/// Planeja armazenamento local contíguo para agregados em um quadro de pilha.
pub fn planejar_armazenamento(layouts: &[(u64, u64)]) -> Result<PlanoArmazenamento, String> {
    let mut topo = 0u64;
    let mut alocacoes = Vec::with_capacity(layouts.len());
    for &(tamanho, alinhamento) in layouts {
        validar_layout(tamanho, alinhamento)?;
        topo = alinhar(
            topo.checked_add(tamanho)
                .ok_or("Quadro de pilha excede o limite de tamanho.")?,
            alinhamento,
        )?;
        let deslocamento = i32::try_from(topo)
            .ok()
            .and_then(i32::checked_neg)
            .ok_or("Deslocamento do agregado excede o endereçamento x86-64.")?;
        alocacoes.push(AlocacaoAgregado {
            deslocamento,
            tamanho,
            alinhamento,
        });
    }
    Ok(PlanoArmazenamento {
        alocacoes,
        tamanho_quadro: alinhar(topo, 16)?,
    })
}

fn validar_layout(tamanho: u64, alinhamento: u64) -> Result<(), String> {
    if tamanho == 0 {
        return Err("Agregado de tamanho zero não possui representação ABI.".into());
    }
    if alinhamento == 0 || !alinhamento.is_power_of_two() {
        return Err(format!("Alinhamento ABI inválido: {alinhamento}."));
    }
    Ok(())
}

fn alinhar(valor: u64, alinhamento: u64) -> Result<u64, String> {
    valor
        .checked_add(alinhamento - 1)
        .map(|v| v & !(alinhamento - 1))
        .ok_or_else(|| "Overflow ao alinhar a área ABI.".into())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn classifica_fronteiras_dos_registradores() {
        assert_eq!(
            classificar_agregado(1, 1).unwrap(),
            ClasseAgregado::UmRegistrador
        );
        assert_eq!(
            classificar_agregado(8, 8).unwrap(),
            ClasseAgregado::UmRegistrador
        );
        assert_eq!(
            classificar_agregado(9, 1).unwrap(),
            ClasseAgregado::DoisRegistradores
        );
        assert_eq!(
            classificar_agregado(16, 16).unwrap(),
            ClasseAgregado::DoisRegistradores
        );
        assert_eq!(
            classificar_agregado(17, 8).unwrap(),
            ClasseAgregado::Indireto {
                tamanho: 17,
                alinhamento: 8
            }
        );
        assert_eq!(
            classificar_agregado(4096, 64).unwrap(),
            ClasseAgregado::Indireto {
                tamanho: 4096,
                alinhamento: 64
            }
        );
    }

    #[test]
    fn rejeita_layouts_invalidos() {
        assert!(classificar_agregado(0, 1).is_err());
        assert!(classificar_agregado(8, 0).is_err());
        assert!(classificar_agregado(8, 3).is_err());
    }

    #[test]
    fn retorno_grande_usa_destino_oculto() {
        assert_eq!(
            classificar_retorno_agregado(24, 8).unwrap(),
            RetornoAgregado::DestinoOculto {
                tamanho: 24,
                alinhamento: 8
            }
        );
        assert_eq!(
            classificar_retorno_agregado(16, 8).unwrap(),
            RetornoAgregado::Registradores(ClasseAgregado::DoisRegistradores)
        );
    }

    #[test]
    fn destino_oculto_reserva_primeiro_registrador() {
        let plano = planejar_argumentos(&[(8, 8), (24, 8), (16, 8)], Some((32, 16))).unwrap();
        assert!(plano.destino_oculto);
        assert_eq!(plano.argumentos[0], vec![LocalArgumento::Registrador(1)]);
        assert_eq!(plano.argumentos[1], vec![LocalArgumento::PonteiroRegistrador(2)]);
        assert_eq!(
            plano.argumentos[2],
            vec![
                LocalArgumento::Registrador(3),
                LocalArgumento::Registrador(4)
            ]
        );
    }

    #[test]
    fn agregado_nao_e_dividido_entre_registrador_e_pilha() {
        let mut argumentos = vec![(8, 8); 5];
        argumentos.push((16, 16));
        let plano = planejar_argumentos(&argumentos, None).unwrap();
        assert_eq!(plano.argumentos[4], vec![LocalArgumento::Registrador(4)]);
        assert_eq!(
            plano.argumentos[5],
            vec![LocalArgumento::Pilha {
                deslocamento: 0,
                tamanho: 16,
                alinhamento: 16
            }]
        );
        assert_eq!(plano.tamanho_pilha, 16);
    }

    #[test]
    fn argumento_grande_e_passado_por_um_ponteiro() {
        let plano = planejar_argumentos(&[(24, 8)], None).unwrap();
        assert_eq!(
            plano.argumentos[0],
            vec![LocalArgumento::PonteiroRegistrador(0)]
        );
    }

    #[test]
    fn ponteiro_de_agregado_grande_vai_para_pilha_quando_faltam_registradores() {
        let argumentos = vec![(8, 8); 6]
            .into_iter()
            .chain([(24, 8)])
            .collect::<Vec<_>>();
        let plano = planejar_argumentos(&argumentos, None).unwrap();
        assert_eq!(
            plano.argumentos[6],
            vec![LocalArgumento::PonteiroPilha { deslocamento: 0 }]
        );
        assert_eq!(plano.tamanho_pilha, 16);
    }

    #[test]
    fn quadro_respeita_alinhamento_e_tamanho() {
        let plano = planejar_armazenamento(&[(24, 8), (32, 16), (17, 1)]).unwrap();
        assert_eq!(plano.alocacoes[0].deslocamento, -24);
        assert_eq!(plano.alocacoes[1].deslocamento, -64);
        assert_eq!(plano.alocacoes[2].deslocamento, -81);
        assert_eq!(plano.tamanho_quadro, 96);
    }

    #[test]
    fn quadro_rejeita_overflow_de_deslocamento() {
        assert!(planejar_armazenamento(&[(u64::MAX, 8)]).is_err());
    }
}
