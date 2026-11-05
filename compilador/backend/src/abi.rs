//! Classificação da ABI interna x86-64 da Nebula.
//!
//! Agregados de até 16 bytes são retornados em registradores. Agregados
//! maiores são representados por endereço: argumentos recebem um ponteiro
//! indireto e retornos usam um destino oculto fornecido pelo chamador.
//! Esta camada descreve a convenção; o backend só deve emitir uma chamada
//! quando também conseguir materializar/c copiar o armazenamento indicado.

/// Forma física de um valor agregado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClasseAgregado {
    /// Até oito bytes, transportados em RAX (ou no registrador de argumento).
    UmRegistrador,
    /// De nove a dezesseis bytes, transportados em RAX:RDX.
    DoisRegistradores,
    /// Mais de dezesseis bytes: endereço de armazenamento, nunca truncado.
    Indireto { tamanho: u64, alinhamento: u64 },
}

/// Classifica um agregado conforme seu tamanho e alinhamento.
pub fn classificar_agregado(tamanho: u64, alinhamento: u64) -> Result<ClasseAgregado, String> {
    if tamanho == 0 {
        return Err("Agregado de tamanho zero não possui representação ABI.".into());
    }
    if alinhamento == 0 || !alinhamento.is_power_of_two() {
        return Err(format!("Alinhamento ABI inválido: {alinhamento}."));
    }
    Ok(match tamanho {
        1..=8 => ClasseAgregado::UmRegistrador,
        9..=16 => ClasseAgregado::DoisRegistradores,
        _ => ClasseAgregado::Indireto { tamanho, alinhamento },
    })
}

/// Forma de retorno de uma função.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetornoAgregado {
    Registradores(ClasseAgregado),
    /// O chamador passa o endereço de destino no primeiro argumento (RDI).
    DestinoOculto { tamanho: u64, alinhamento: u64 },
}

/// Classifica o retorno agregado, reservando o destino oculto quando necessário.
pub fn classificar_retorno_agregado(
    tamanho: u64,
    alinhamento: u64,
) -> Result<RetornoAgregado, String> {
    let classe = classificar_agregado(tamanho, alinhamento)?;
    Ok(match classe {
        ClasseAgregado::Indireto { tamanho, alinhamento } => {
            RetornoAgregado::DestinoOculto { tamanho, alinhamento }
        }
        classe => RetornoAgregado::Registradores(classe),
    })
}

/// Localização física de um argumento na convenção interna.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalArgumento {
    /// Índice do registrador inteiro: RDI, RSI, RDX, RCX, R8, R9.
    Registrador(u8),
    /// Deslocamento positivo, em bytes, a partir do início da área de argumentos.
    Pilha { deslocamento: u64, tamanho: u64, alinhamento: u64 },
}

/// Plano de passagem de argumentos inteiros/indiretos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanoArgumentos {
    pub argumentos: Vec<Vec<LocalArgumento>>,
    pub tamanho_pilha: u64,
    pub destino_oculto: bool,
}

/// Cria um plano de argumentos. Cada argumento ocupa um registrador por palavra
/// de até oito bytes; agregados grandes ocupam um único ponteiro.
/// O retorno grande reserva RDI antes dos argumentos explícitos.
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
    let mut proximo_registrador = usize::from(destino_oculto);
    let mut deslocamento_pilha = 0u64;
    let mut argumentos = Vec::with_capacity(agregados.len());

    for &(tamanho, alinhamento) in agregados {
        let classe = classificar_agregado(tamanho, alinhamento)?;
        let palavras = match classe {
            ClasseAgregado::UmRegistrador => 1,
            ClasseAgregado::DoisRegistradores => 2,
            ClasseAgregado::Indireto { .. } => 1,
        };
        let mut locais = Vec::with_capacity(palavras);
        for palavra in 0..palavras {
            if proximo_registrador < 6 {
                locais.push(LocalArgumento::Registrador(proximo_registrador as u8));
                proximo_registrador += 1;
            } else {
                let (tamanho_pilha, alinhamento_pilha) = match classe {
                    ClasseAgregado::Indireto { .. } => (8, 8),
                    _ => (8, 8),
                };
                deslocamento_pilha = alinhar(deslocamento_pilha, alinhamento_pilha)?;
                locais.push(LocalArgumento::Pilha {
                    deslocamento: deslocamento_pilha,
                    tamanho: tamanho_pilha,
                    alinhamento: alinhamento_pilha,
                });
                deslocamento_pilha = deslocamento_pilha.checked_add(tamanho_pilha)
                    .ok_or("Área de argumentos excede o limite de endereçamento.")?;
            }
            let _ = palavra;
        }
        argumentos.push(locais);
    }

    Ok(PlanoArgumentos {
        argumentos,
        tamanho_pilha: alinhar(deslocamento_pilha, 16)?,
        destino_oculto,
    })
}

fn alinhar(valor: u64, alinhamento: u64) -> Result<u64, String> {
    valor.checked_add(alinhamento - 1)
        .map(|v| v & !(alinhamento - 1))
        .ok_or_else(|| "Overflow ao alinhar a área de argumentos ABI.".into())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn classifica_agregados_nas_fronteiras_dos_registradores() {
        assert_eq!(classificar_agregado(1, 1).unwrap(), ClasseAgregado::UmRegistrador);
        assert_eq!(classificar_agregado(8, 8).unwrap(), ClasseAgregado::UmRegistrador);
        assert_eq!(classificar_agregado(9, 1).unwrap(), ClasseAgregado::DoisRegistradores);
        assert_eq!(classificar_agregado(16, 16).unwrap(), ClasseAgregado::DoisRegistradores);
        assert_eq!(
            classificar_agregado(17, 8).unwrap(),
            ClasseAgregado::Indireto { tamanho: 17, alinhamento: 8 }
        );
        assert_eq!(
            classificar_agregado(4096, 64).unwrap(),
            ClasseAgregado::Indireto { tamanho: 4096, alinhamento: 64 }
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
            RetornoAgregado::DestinoOculto { tamanho: 24, alinhamento: 8 }
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
        assert_eq!(plano.argumentos[1], vec![LocalArgumento::Registrador(2)]);
        assert_eq!(
            plano.argumentos[2],
            vec![LocalArgumento::Registrador(3), LocalArgumento::Registrador(4)]
        );
    }

    #[test]
    fn argumentos_excedentes_vao_para_area_de_pilha_alinhada() {
        let plano = planejar_argumentos(&[(8, 8); 7], None).unwrap();
        assert_eq!(plano.argumentos[5], vec![LocalArgumento::Registrador(5)]);
        assert_eq!(
            plano.argumentos[6],
            vec![LocalArgumento::Pilha { deslocamento: 0, tamanho: 8, alinhamento: 8 }]
        );
        assert_eq!(plano.tamanho_pilha, 16);
    }
}
