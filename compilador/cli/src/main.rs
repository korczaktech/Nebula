//! Interface de linha de comando da Nebula.

#![forbid(unsafe_code)]

use nebula_fonte::ArquivoFonte;
use nebula_ir::otimizador::NivelOtimizacao;
use nebula_ir::{ExpressaoIr, InstrucaoIr, ProgramaIr, baixar};
use nebula_semantica::{AssinaturaFuncao, ModulosDisponiveis, Tipo, analisar_semantica};
use nebula_sintaxe::{NoAst, Programa, Visibilidade, analisar};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[rustfmt::skip]
fn main() {
    let mut argumentos = std::env::args().skip(1);
    match argumentos.next().as_deref() {
        Some("versao" | "--version" | "-V") => println!(
            "Nebula {} — compilador nativo inicial",
            env!("CARGO_PKG_VERSION")
        ),
        Some("ajuda" | "--help" | "-h") | None => ajuda(),
        Some("compilar") => {
            let Some(caminho) = argumentos.next() else { eprintln!("Uso: nebula compilar <arquivo.nbx> [-o <saida>]"); std::process::exit(2); };
            let mut saida = PathBuf::from(&caminho).with_extension("out");
            let mut nivel = NivelOtimizacao::O1;
            while let Some(arg) = argumentos.next() {
                if arg == "-o" {
                    let Some(valor) = argumentos.next() else { eprintln!("Erro: -o exige um caminho."); std::process::exit(2); };
                    saida = PathBuf::from(valor);
                } else if arg == "--otimizacao" {
                    let Some(valor) = argumentos.next() else { eprintln!("Erro: --otimizacao exige O0, O1, O2, O3 ou Os."); std::process::exit(2); };
                    nivel = nivel_otimizacao(&valor).unwrap_or_else(|erro| { eprintln!("{erro}"); std::process::exit(2); });
                } else if arg.starts_with("-O") {
                    nivel = nivel_otimizacao(&arg[1..]).unwrap_or_else(|erro| { eprintln!("{erro}"); std::process::exit(2); });
                } else {
                    eprintln!("Argumento desconhecido: {arg}");
                    std::process::exit(2);
                }
            }
            if let Err(erro) = compilar_arquivo_com_nivel(Path::new(&caminho), &saida, nivel) {
                eprintln!("Erro de compilação: {erro}");
                std::process::exit(1);
            }
            println!("Executável gerado: {}", saida.display());
        }
        Some("executar" | "testar") => {
            let Some(caminho) = argumentos.next() else { eprintln!("Uso: nebula executar|testar <arquivo.nbx>"); std::process::exit(2); };
            if argumentos.next().is_some() { eprintln!("Erro: executar aceita somente um arquivo."); std::process::exit(2); }
            let temporario = std::env::temp_dir().join(format!("nebula-{}{}", std::process::id(), if cfg!(windows) { ".exe" } else { "" }));
            if let Err(erro) = compilar_arquivo(Path::new(&caminho), &temporario) { eprintln!("Erro de compilação: {erro}"); std::process::exit(1); }
            let status = Command::new(&temporario).status();
            let _ = std::fs::remove_file(&temporario);
            match status { Ok(s) if s.success() => {}, Ok(s) => std::process::exit(s.code().unwrap_or(1)), Err(e) => { eprintln!("Não foi possível executar o programa: {e}"); std::process::exit(1); } }
        }
        Some("interpretar") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula interpretar <arquivo.nbx>");
                std::process::exit(2);
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: interpretar aceita somente um arquivo.");
                std::process::exit(2);
            }
            match interpretar_arquivo(Path::new(&caminho)) {
                Ok(linhas) => for linha in linhas { println!("{linha}"); },
                Err(erro) => { eprintln!("Erro de execução: {erro}"); std::process::exit(1); }
            }
        }
        Some("ir") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula ir <arquivo.nbx>");
                std::process::exit(2);
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: o comando 'ir' aceita somente um arquivo.");
                std::process::exit(2);
            }
            mostrar_ir(Path::new(&caminho));
        }
        Some("analisar") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula analisar <arquivo.nbx>");
                std::process::exit(2);
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: o comando 'analisar' aceita somente um arquivo.");
                std::process::exit(2);
            }
            analisar_arquivo(Path::new(&caminho));
        }
        Some("verificar" | "check") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula verificar <arquivo.nbx>");
                std::process::exit(2);
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: o comando 'verificar' aceita somente um arquivo.");
                eprintln!("Uso: nebula verificar <arquivo.nbx>");
                std::process::exit(2);
            }
            verificar(Path::new(&caminho));
        }
        Some(comando) => {
            eprintln!("Comando desconhecido: {comando}");
            eprintln!("Execute 'nebula ajuda' para consultar os comandos disponíveis.");
            std::process::exit(2);
        }
    }
}

fn interpretar_arquivo(caminho: &Path) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(caminho)
        .map_err(|erro| format!("não foi possível ler '{}': {erro}", caminho.display()))?;
    let conteudo = String::from_utf8(bytes).map_err(|erro| {
        format!(
            "arquivo não contém UTF-8 válido no byte {}",
            erro.utf8_error().valid_up_to()
        )
    })?;
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let sintaxe = analisar(&fonte);
    if !sintaxe.diagnosticos.is_empty() {
        return Err(sintaxe
            .diagnosticos
            .iter()
            .map(|d| format!("{} — {}", d.codigo, d.mensagem))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let (modulos, programas_modulos) = carregar_modulos(caminho, &sintaxe.programa)?;
    let resultado = baixar(&sintaxe.programa, &modulos);
    if !resultado.diagnosticos.is_empty() {
        return Err(resultado.diagnosticos.join("\n"));
    }
    let mut programa = resultado.programa.expect("IR validada");
    for (caminho_modulo, modulo) in programas_modulos {
        for (nome, funcao) in modulo.funcoes {
            programa
                .funcoes
                .insert(format!("{caminho_modulo}::{nome}"), funcao);
        }
    }
    let execucao = nebula_ir::executar(&programa, 1_000_000)?;
    Ok(execucao.saida)
}

fn resolver_arquivo_modulo(origem: &Path, caminho: &str) -> Result<PathBuf, String> {
    let base = origem.parent().unwrap_or_else(|| Path::new("."));
    let mut relativo = PathBuf::new();
    for componente in caminho.split("::") {
        relativo.push(componente);
    }
    if relativo.extension().is_none() {
        relativo.set_extension("nbx");
    }
    let candidato = base.join(relativo);
    if candidato.is_file() {
        return Ok(candidato);
    }
    Err(format!(
        "Módulo '{caminho}' não encontrado. Esperado arquivo relativo a '{}'.",
        base.display()
    ))
}

fn tipo_assinatura_modulo(nome: &str) -> Option<Tipo> {
    Some(match nome {
        "vazio" => Tipo::Vazio,
        "logico" => Tipo::Logico,
        "inteiro" => Tipo::Inteiro,
        "inteiro8" => Tipo::Inteiro8,
        "inteiro16" => Tipo::Inteiro16,
        "inteiro32" => Tipo::Inteiro32,
        "inteiro64" => Tipo::Inteiro64,
        "inteiro128" => Tipo::Inteiro128,
        "decimal" => Tipo::Decimal,
        "decimal32" => Tipo::Decimal32,
        "decimal64" => Tipo::Decimal64,
        "texto" => Tipo::Texto,
        "caractere" => Tipo::Caractere,
        "byte" => Tipo::Byte,
        lista if lista.starts_with('[') && lista.ends_with(']') => Tipo::Lista(Box::new(
            tipo_assinatura_modulo(&lista[1..lista.len() - 1])?,
        )),
        _ => return None,
    })
}

fn carregar_modulos(
    origem: &Path,
    programa: &Programa,
) -> Result<(ModulosDisponiveis, Vec<(String, ProgramaIr)>), String> {
    let mut assinaturas = ModulosDisponiveis::new();
    let mut programas = Vec::new();
    for item in &programa.itens {
        let NoAst::Incluir { caminho, .. } = item else {
            continue;
        };
        if assinaturas.contains_key(caminho) {
            continue;
        }
        let arquivo = resolver_arquivo_modulo(origem, caminho)?;
        let bytes = std::fs::read(&arquivo).map_err(|erro| {
            format!(
                "Não foi possível ler módulo '{}': {erro}",
                arquivo.display()
            )
        })?;
        let codigo = String::from_utf8(bytes)
            .map_err(|erro| format!("Módulo '{}' não é UTF-8 válido: {erro}", arquivo.display()))?;
        let fonte = ArquivoFonte::novo(&arquivo, codigo);
        let sintaxe = analisar(&fonte);
        if !sintaxe.diagnosticos.is_empty() {
            return Err(format!(
                "Erro de sintaxe no módulo '{}': {}",
                caminho,
                sintaxe
                    .diagnosticos
                    .iter()
                    .map(|d| format!("{} — {}", d.codigo, d.mensagem))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
        if sintaxe
            .programa
            .itens
            .iter()
            .any(|item| matches!(item, NoAst::Incluir { .. }))
        {
            return Err(format!(
                "O módulo '{caminho}' inclui outros módulos; dependências aninhadas ainda não estão ligadas pelo interpretador."
            ));
        }
        if sintaxe
            .programa
            .itens
            .iter()
            .any(|item| !matches!(item, NoAst::Funcao { .. } | NoAst::Tipo { .. }))
        {
            return Err(format!(
                "O módulo '{caminho}' contém inicializadores globais; módulos carregados pelo interpretador devem conter funções e aliases de tipo."
            ));
        }

        let mut exportadas = std::collections::HashMap::new();
        for item in &sintaxe.programa.itens {
            if let NoAst::Funcao {
                visibilidade: Some(Visibilidade::Publico),
                nome,
                parametros,
                retorno,
                ..
            } = item
            {
                let parametros = parametros
                    .iter()
                    .map(|(_, tipo)| tipo_assinatura_modulo(tipo)
                        .ok_or_else(|| format!("Tipo '{tipo}' não suportado na assinatura pública '{caminho}::{nome}'.")))
                    .collect::<Result<Vec<_>, _>>()?;
                let retorno = retorno.as_deref().unwrap_or("vazio");
                let retorno = tipo_assinatura_modulo(retorno).ok_or_else(|| {
                    format!("Tipo de retorno '{retorno}' não suportado em '{caminho}::{nome}'.")
                })?;
                exportadas.insert(
                    nome.clone(),
                    AssinaturaFuncao {
                        parametros,
                        retorno,
                    },
                );
            }
        }
        if exportadas.is_empty() {
            return Err(format!(
                "O módulo '{caminho}' não exporta nenhuma função pública."
            ));
        }

        let resultado_ir = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        if !resultado_ir.diagnosticos.is_empty() {
            return Err(format!(
                "Erro semântico no módulo '{caminho}': {}",
                resultado_ir.diagnosticos.join("\n")
            ));
        }
        let mut modulo_ir = resultado_ir.programa.expect("módulo validado");
        let nomes = modulo_ir.funcoes.keys().cloned().collect::<HashSet<_>>();
        for funcao in modulo_ir.funcoes.values_mut() {
            for instrucao in &mut funcao.corpo {
                qualificar_chamadas_modulo(instrucao, caminho, &nomes);
            }
        }
        assinaturas.insert(caminho.clone(), exportadas);
        programas.push((caminho.clone(), modulo_ir));
    }
    Ok((assinaturas, programas))
}

fn qualificar_chamadas_modulo(
    instrucao: &mut InstrucaoIr,
    caminho: &str,
    funcoes: &HashSet<String>,
) {
    match instrucao {
        InstrucaoIr::Declarar { valor, .. } | InstrucaoIr::Expressao(valor) => {
            qualificar_expressao_modulo(valor, caminho, funcoes);
        }
        InstrucaoIr::Bloco(itens) => {
            for item in itens {
                qualificar_chamadas_modulo(item, caminho, funcoes);
            }
        }
        InstrucaoIr::Se {
            condicao,
            entao,
            senao,
        } => {
            qualificar_expressao_modulo(condicao, caminho, funcoes);
            for item in entao.iter_mut().chain(senao.iter_mut()) {
                qualificar_chamadas_modulo(item, caminho, funcoes);
            }
        }
        InstrucaoIr::Enquanto { condicao, corpo } => {
            qualificar_expressao_modulo(condicao, caminho, funcoes);
            for item in corpo {
                qualificar_chamadas_modulo(item, caminho, funcoes);
            }
        }
        InstrucaoIr::Para {
            inicial,
            condicao,
            atualizacao,
            corpo,
        } => {
            if let Some(inicial) = inicial {
                qualificar_chamadas_modulo(inicial, caminho, funcoes);
            }
            if let Some(condicao) = condicao {
                qualificar_expressao_modulo(condicao, caminho, funcoes);
            }
            if let Some(atualizacao) = atualizacao {
                qualificar_expressao_modulo(atualizacao, caminho, funcoes);
            }
            for item in corpo {
                qualificar_chamadas_modulo(item, caminho, funcoes);
            }
        }
        InstrucaoIr::Devolver(valor) => {
            if let Some(valor) = valor {
                qualificar_expressao_modulo(valor, caminho, funcoes);
            }
        }
        InstrucaoIr::Parar | InstrucaoIr::Seguir => {}
    }
}

fn qualificar_expressao_modulo(
    expressao: &mut ExpressaoIr,
    caminho: &str,
    funcoes: &HashSet<String>,
) {
    let mut chamada_local = None;
    match expressao {
        ExpressaoIr::Lista(itens) => {
            for item in itens {
                qualificar_expressao_modulo(item, caminho, funcoes);
            }
        }
        ExpressaoIr::Unaria { valor, .. } => {
            qualificar_expressao_modulo(valor, caminho, funcoes);
        }
        ExpressaoIr::Binaria {
            esquerda, direita, ..
        } => {
            qualificar_expressao_modulo(esquerda, caminho, funcoes);
            qualificar_expressao_modulo(direita, caminho, funcoes);
        }
        ExpressaoIr::Atribuicao { valor, .. } => {
            qualificar_expressao_modulo(valor, caminho, funcoes);
        }
        ExpressaoIr::Chamada { nome, argumentos } => {
            for argumento in argumentos.iter_mut() {
                qualificar_expressao_modulo(argumento, caminho, funcoes);
            }
            if funcoes.contains(nome) {
                chamada_local = Some(nome.clone());
            }
        }
        ExpressaoIr::ChamadaQualificada { argumentos, .. } => {
            for argumento in argumentos {
                qualificar_expressao_modulo(argumento, caminho, funcoes);
            }
        }
        ExpressaoIr::Indexacao { alvo, indice } => {
            qualificar_expressao_modulo(alvo, caminho, funcoes);
            qualificar_expressao_modulo(indice, caminho, funcoes);
        }
        ExpressaoIr::Constante(_) | ExpressaoIr::Variavel(_) => {}
    }
    if let Some(nome) = chamada_local {
        let ExpressaoIr::Chamada { argumentos, .. } = expressao else {
            unreachable!("expressão identificada como chamada local");
        };
        let argumentos = std::mem::take(argumentos);
        *expressao = ExpressaoIr::ChamadaQualificada {
            modulo: caminho.to_owned(),
            nome,
            argumentos,
        };
    }
}

#[rustfmt::skip]
fn compilar_arquivo(caminho: &Path, saida: &Path) -> Result<(), String> {
    compilar_arquivo_com_nivel(caminho, saida, NivelOtimizacao::O1)
}

#[rustfmt::skip]
fn compilar_arquivo_com_nivel(
    caminho: &Path,
    saida: &Path,
    nivel: NivelOtimizacao,
) -> Result<(), String> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err("O backend inicial gera executáveis somente para Linux x86-64.".into());
    }
    validar_caminhos_compilacao(caminho, saida)?;
    let bytes = std::fs::read(caminho)
        .map_err(|e| format!("não foi possível ler '{}': {e}", caminho.display()))?;
    let conteudo = String::from_utf8(bytes)
        .map_err(|e| format!("arquivo não contém UTF-8 válido no byte {}", e.utf8_error().valid_up_to()))?;
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let sintaxe = analisar(&fonte);
    if !sintaxe.diagnosticos.is_empty() {
        return Err(sintaxe.diagnosticos.iter()
            .map(|d| format!("{} — {}", d.codigo, d.mensagem))
            .collect::<Vec<_>>().join("\n"));
    }
    let ir = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
    if !ir.diagnosticos.is_empty() {
        return Err(ir.diagnosticos.join("\n"));
    }
    let mut programa = ir.programa.expect("IR validada");
    nebula_ir::otimizador::otimizar(&mut programa, nivel)?;
    let binario = nebula_backend::gerar_executavel(&programa)?;
    gravar_executavel_atomico(saida, &binario)
}

fn nivel_otimizacao(valor: &str) -> Result<NivelOtimizacao, String> {
    match valor.to_ascii_lowercase().as_str() {
        "o0" | "0" => Ok(NivelOtimizacao::O0),
        "o1" | "1" => Ok(NivelOtimizacao::O1),
        "o2" | "2" => Ok(NivelOtimizacao::O2),
        "o3" | "3" => Ok(NivelOtimizacao::O3),
        "os" | "s" => Ok(NivelOtimizacao::Os),
        _ => Err(format!(
            "Nível de otimização inválido: '{valor}'. Use O0, O1, O2, O3 ou Os."
        )),
    }
}

fn validar_caminhos_compilacao(entrada: &Path, saida: &Path) -> Result<(), String> {
    let entrada_real = std::fs::canonicalize(entrada).map_err(|erro| {
        format!(
            "não foi possível resolver o arquivo de entrada '{}': {erro}",
            entrada.display()
        )
    })?;
    let nome_saida = saida
        .file_name()
        .ok_or_else(|| format!("caminho de saída inválido: '{}'", saida.display()))?;
    let diretorio_saida = saida
        .parent()
        .filter(|diretorio| !diretorio.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let diretorio_real = std::fs::canonicalize(diretorio_saida).map_err(|erro| {
        format!(
            "não foi possível resolver o diretório de saída '{}': {erro}",
            diretorio_saida.display()
        )
    })?;
    let saida_real = if saida.exists() {
        std::fs::canonicalize(saida).map_err(|erro| {
            format!(
                "não foi possível resolver a saída '{}': {erro}",
                saida.display()
            )
        })?
    } else {
        diretorio_real.join(nome_saida)
    };
    if entrada_real == saida_real {
        return Err("O executável de saída não pode sobrescrever o arquivo-fonte.".into());
    }
    Ok(())
}

#[rustfmt::skip]
fn gravar_executavel_atomico(saida: &Path, binario: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let nome = saida.file_name().ok_or_else(|| {
        format!("caminho de saída inválido: '{}'", saida.display())
    })?;
    let temporario = saida.with_file_name(format!(
        ".{}.{}.nebula-tmp",
        nome.to_string_lossy(),
        std::process::id()
    ));

    let resultado = (|| {
        // Um processo anterior pode ter sido encerrado antes da publicação.
        let _ = std::fs::remove_file(&temporario);
        let mut arquivo = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporario)
            .map_err(|e| format!("não foi possível criar arquivo temporário '{}': {e}", temporario.display()))?;
        arquivo.write_all(binario)
            .map_err(|e| format!("não foi possível gravar executável temporário: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            arquivo.set_permissions(std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("não foi possível marcar o executável: {e}"))?;
        }
        arquivo.sync_all()
            .map_err(|e| format!("não foi possível sincronizar o executável: {e}"))?;
        drop(arquivo);
        std::fs::rename(&temporario, saida)
            .map_err(|e| format!("não foi possível publicar o executável '{}': {e}", saida.display()))?;
        Ok(())
    })();

    if resultado.is_err() {
        let _ = std::fs::remove_file(&temporario);
    }
    resultado
}

fn ajuda() {
    println!("Nebula — linguagem de programação");
    println!();
    println!("Uso: nebula <comando>");
    println!();
    println!("Comandos disponíveis:");
    println!("  ajuda                 Exibe esta mensagem");
    println!("  versao                Exibe a versão da ferramenta");
    println!("  verificar <arquivo>   Analisa léxica e sintaticamente um arquivo .nbx");
    println!("  analisar <arquivo>    Verifica nomes, escopos e tipos estáticos");
    println!("  ir <arquivo>          Gera e exibe a representação intermediária");
    println!(
        "  compilar <arquivo>    Gera executável ELF nativo Linux x86-64 (-O0/-O1/-O2/-O3/-Os)"
    );
    println!("  interpretar <arquivo> Executa a IR pelo runtime Rust de referência");
    println!("  executar <arquivo>    Compila e executa um programa nativo");
    println!("  testar <arquivo>      Compila e executa como teste de fumaça");
    println!();
    println!("A verificação sintática não realiza análise semântica nem gera executáveis.");
    println!("O backend gera diretamente executáveis ELF x86-64 em Rust, sem compilador externo.");
    println!("'interpretar' usa o runtime de referência; 'executar' usa o backend nativo.");
    println!("O backend ELF nativo ainda não liga o runtime nem a biblioteca padrão.");
}

fn mostrar_ir(caminho: &Path) {
    let bytes = std::fs::read(caminho).unwrap_or_else(|erro| {
        eprintln!(
            "Erro N0001: não foi possível ler '{}': {erro}",
            caminho.display()
        );
        std::process::exit(1);
    });
    let conteudo = String::from_utf8(bytes).unwrap_or_else(|erro| {
        eprintln!(
            "Erro N1000: arquivo '{}' não contém UTF-8 válido; sequência inválida a partir do byte {}.",
            caminho.display(),
            erro.utf8_error().valid_up_to()
        );
        std::process::exit(1);
    });
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let sintaxe = analisar(&fonte);
    if !sintaxe.diagnosticos.is_empty() {
        for diagnostico in &sintaxe.diagnosticos {
            eprintln!("{} — {}", diagnostico.codigo, diagnostico.mensagem);
        }
        std::process::exit(1);
    }
    let resultado = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
    if !resultado.diagnosticos.is_empty() {
        for diagnostico in &resultado.diagnosticos {
            eprintln!("{diagnostico}");
        }
        std::process::exit(1);
    }
    println!("{:#?}", resultado.programa.expect("baixamento validado"));
}

fn analisar_arquivo(caminho: &Path) {
    let bytes = std::fs::read(caminho).unwrap_or_else(|erro| {
        eprintln!(
            "Erro N0001: não foi possível ler '{}': {erro}",
            caminho.display()
        );
        std::process::exit(1);
    });
    let conteudo = String::from_utf8(bytes).unwrap_or_else(|erro| {
        eprintln!("Erro N1000: arquivo '{}' não contém UTF-8 válido; sequência inválida a partir do byte {}.", caminho.display(), erro.utf8_error().valid_up_to());
        std::process::exit(1);
    });
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let sintaxe = analisar(&fonte);
    if !sintaxe.diagnosticos.is_empty() {
        for diagnostico in &sintaxe.diagnosticos {
            let local = diagnostico
                .intervalo
                .and_then(|i| fonte.linha_coluna(i.inicio));
            if let Some((linha, coluna)) = local {
                eprintln!(
                    "{} {}:{}:{} — {}",
                    diagnostico.codigo,
                    caminho.display(),
                    linha,
                    coluna,
                    diagnostico.mensagem
                );
            } else {
                eprintln!("{} — {}", diagnostico.codigo, diagnostico.mensagem);
            }
        }
        std::process::exit(1);
    }
    let resultado = analisar_semantica(&sintaxe.programa);
    for diagnostico in &resultado.diagnosticos {
        eprintln!("{} — {}", diagnostico.codigo, diagnostico.mensagem);
    }
    if resultado.diagnosticos.is_empty() {
        println!("Arquivo válido semanticamente: {}", caminho.display());
    } else {
        std::process::exit(1);
    }
}

fn verificar(caminho: &Path) {
    let bytes = match std::fs::read(caminho) {
        Ok(bytes) => bytes,
        Err(erro) => {
            eprintln!(
                "Erro N0001: não foi possível ler '{}': {erro}",
                caminho.display()
            );
            std::process::exit(1);
        }
    };
    let conteudo = match String::from_utf8(bytes) {
        Ok(conteudo) => conteudo,
        Err(erro) => {
            eprintln!(
                "Erro N1000: arquivo '{}' não contém UTF-8 válido; sequência inválida a partir do byte {}.",
                caminho.display(),
                erro.utf8_error().valid_up_to()
            );
            std::process::exit(1);
        }
    };
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let resultado = analisar(&fonte);
    for diagnostico in &resultado.diagnosticos {
        let local = diagnostico
            .intervalo
            .and_then(|i| fonte.linha_coluna(i.inicio));
        if let Some((linha, coluna)) = local {
            eprintln!(
                "{} {}:{}:{} — {}",
                diagnostico.codigo,
                caminho.display(),
                linha,
                coluna,
                diagnostico.mensagem
            );
        } else {
            eprintln!("{} — {}", diagnostico.codigo, diagnostico.mensagem);
        }
    }
    if resultado.diagnosticos.is_empty() {
        println!("Arquivo válido sintaticamente: {}", caminho.display());
    } else {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn compila_e_executa_bloco_otimizado_em_o2() {
        let origem = std::env::temp_dir().join(format!("nebula-o2-{}.nbx", std::process::id()));
        let saida = std::env::temp_dir().join(format!("nebula-o2-{}", std::process::id()));
        std::fs::write(
            &origem,
            "funcao principal(): inteiro { quando verdadeiro { devolver 7 } senao { devolver 9 } }",
        )
        .unwrap();
        compilar_arquivo_com_nivel(&origem, &saida, NivelOtimizacao::O2)
            .expect("compilação O2 válida");
        let status = Command::new(&saida).status().expect("executável gerado");
        assert_eq!(status.code(), Some(7));
        let _ = std::fs::remove_file(origem);
        let _ = std::fs::remove_file(saida);
    }

    #[test]
    fn interpreta_modulo_local_com_funcao_publica_e_auxiliar_privada() {
        let diretorio = std::env::temp_dir().join(format!("nebula-modulo-{}", std::process::id()));
        std::fs::create_dir_all(&diretorio).unwrap();
        let modulo = diretorio.join("Matematica.nbx");
        let origem = diretorio.join("principal.nbx");
        std::fs::write(
            &modulo,
            "funcao somar(a: inteiro, b: inteiro): inteiro { devolver a + b }\npublico funcao dobro(valor: inteiro): inteiro { devolver somar(valor, valor) }",
        )
        .unwrap();
        std::fs::write(&origem, "incluir Matematica como Mat\nexibir(Mat::dobro(21))").unwrap();

        let saida = interpretar_arquivo(&origem).expect("módulo local válido");
        assert_eq!(saida, vec!["42"]);
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[test]
    fn interpreta_programa_com_texto_pelo_runtime() {
        let caminho =
            std::env::temp_dir().join(format!("nebula-runtime-{}.nbx", std::process::id()));
        std::fs::write(
            &caminho,
            "variavel nome = \"Nebula\"\nexibir(\"Olá, \" + nome)",
        )
        .unwrap();
        let saida = interpretar_arquivo(&caminho).expect("interpretação válida");
        assert_eq!(saida, vec!["Olá, Nebula"]);
        let _ = std::fs::remove_file(caminho);
    }

    #[test]
    fn reconhece_todos_os_niveis_de_otimizacao() {
        assert_eq!(nivel_otimizacao("O0"), Ok(NivelOtimizacao::O0));
        assert_eq!(nivel_otimizacao("O1"), Ok(NivelOtimizacao::O1));
        assert_eq!(nivel_otimizacao("O2"), Ok(NivelOtimizacao::O2));
        assert_eq!(nivel_otimizacao("O3"), Ok(NivelOtimizacao::O3));
        assert_eq!(nivel_otimizacao("Os"), Ok(NivelOtimizacao::Os));
        assert!(nivel_otimizacao("O4").is_err());
    }

    #[test]
    fn publica_executavel_atomico_e_substitui_saida_anterior() {
        let caminho = std::env::temp_dir().join(format!("nebula-atomic-{}", std::process::id()));
        let _ = std::fs::remove_file(&caminho);
        gravar_executavel_atomico(&caminho, b"primeira versao").unwrap();
        gravar_executavel_atomico(&caminho, b"segunda versao").unwrap();
        assert_eq!(std::fs::read(&caminho).unwrap(), b"segunda versao");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&caminho).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
        let _ = std::fs::remove_file(caminho);
    }

    #[test]
    fn falha_de_compilacao_preserva_executavel_anterior() {
        let origem =
            std::env::temp_dir().join(format!("nebula-invalido-{}.nbx", std::process::id()));
        let saida = std::env::temp_dir().join(format!("nebula-preservado-{}", std::process::id()));
        std::fs::write(&origem, "funcao principal(): vazio { exibir(42) }").unwrap();
        std::fs::write(&saida, b"executavel anterior").unwrap();
        assert!(compilar_arquivo(&origem, &saida).is_err());
        assert_eq!(std::fs::read(&saida).unwrap(), b"executavel anterior");
        let _ = std::fs::remove_file(origem);
        let _ = std::fs::remove_file(saida);
    }

    #[test]
    fn falha_de_publicacao_remove_temporario_e_preserva_destino() {
        let destino =
            std::env::temp_dir().join(format!("nebula-destino-diretorio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&destino);
        std::fs::create_dir(&destino).unwrap();
        let erro = gravar_executavel_atomico(&destino, b"binario").unwrap_err();
        assert!(erro.contains("não foi possível publicar"));
        assert!(destino.is_dir());
        let nome = destino.file_name().unwrap().to_string_lossy();
        let temporario =
            destino.with_file_name(format!(".{nome}.{}.nebula-tmp", std::process::id()));
        assert!(!temporario.exists());
        std::fs::remove_dir(destino).unwrap();
    }

    #[test]
    fn compilacao_nao_sobrescreve_o_arquivo_fonte() {
        let origem =
            std::env::temp_dir().join(format!("nebula-fonte-protegido-{}.nbx", std::process::id()));
        let conteudo = "funcao principal(): inteiro { devolver 42 }";
        std::fs::write(&origem, conteudo).unwrap();
        let erro = validar_caminhos_compilacao(&origem, &origem).unwrap_err();
        assert!(erro.contains("não pode sobrescrever"));
        assert_eq!(std::fs::read_to_string(&origem).unwrap(), conteudo);
        let _ = std::fs::remove_file(origem);
    }

    #[test]
    fn verifica_arquivo_nebula_valido() {
        let caminho = std::env::temp_dir().join(format!("nebula-{}.nbx", std::process::id()));
        std::fs::write(&caminho, "variavel total = 1 + 2").expect("grava arquivo");
        let texto = std::fs::read_to_string(&caminho).expect("lê arquivo");
        assert!(
            analisar(&ArquivoFonte::novo(&caminho, texto))
                .diagnosticos
                .is_empty()
        );
        let _ = std::fs::remove_file(caminho);
    }
}
