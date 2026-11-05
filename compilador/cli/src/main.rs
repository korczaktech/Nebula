//! Interface de linha de comando da Nebula.

#![forbid(unsafe_code)]

use nebula_backend::{alvos_suportados, validar_alvo};
use nebula_cache::CacheCompilacao;
use nebula_diagnosticos::{Diagnostico, Severidade};
use nebula_fonte::ArquivoFonte;
use nebula_ir::otimizador::NivelOtimizacao;
use nebula_ir::{ExpressaoIr, InstrucaoIr, ProgramaIr, baixar};
use nebula_semantica::{AssinaturaFuncao, ModulosDisponiveis, Tipo, analisar_com_modulos};
use nebula_sintaxe::{NoAst, Programa, Visibilidade, analisar};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[rustfmt::skip]
mod debugger;
mod formatador;

type ProgramaModuloCarregado = (String, ProgramaIr);
type FonteModuloCarregada = (PathBuf, Vec<u8>);
type ResultadoCarregamentoModulos = Result<
    (
        ModulosDisponiveis,
        Vec<ProgramaModuloCarregado>,
        Vec<FonteModuloCarregada>,
    ),
    String,
>;

#[rustfmt::skip]
fn main() {
    let mut argumentos = std::env::args().skip(1);
    match argumentos.next().as_deref() {
        Some("versao" | "--version" | "-V") => println!(
            "Nebula {} — compilador nativo inicial",
            env!("CARGO_PKG_VERSION")
        ),
        Some("ajuda" | "--help" | "-h") | None => ajuda(),
        Some("alvos") => {
            if argumentos.next().is_some() {
                eprintln!("Uso: nebula alvos");
                std::process::exit(2);
            }
            println!("Alvos com backend implementado:");
            for alvo in alvos_suportados() {
                println!("  {alvo}");
            }
            println!("Outros alvos permanecem indisponíveis até possuírem backend e testes próprios.");
        },
        Some("criar") => {
            let Some(diretorio) = argumentos.next() else {
                eprintln!("Uso: nebula criar <diretorio> [--nome <nome>]");
                std::process::exit(2);
            };
            let nome = match argumentos.next().as_deref() {
                None => None,
                Some("--nome") => match argumentos.next() {
                    Some(nome) => Some(nome),
                    None => {
                        eprintln!("Erro: --nome exige um identificador.");
                        std::process::exit(2);
                    }
                },
                Some(opcao) => {
                    eprintln!("Opção desconhecida: {opcao}");
                    std::process::exit(2);
                }
            };
            if argumentos.next().is_some() {
                eprintln!("Uso: nebula criar <diretorio> [--nome <nome>]");
                std::process::exit(2);
            }
            match criar_projeto(Path::new(&diretorio), nome.as_deref()) {
                Ok(()) => println!("Projeto Nebula criado: {}", diretorio),
                Err(erro) => {
                    eprintln!("Não foi possível criar o projeto: {erro}");
                    std::process::exit(1);
                }
            }
        }
        Some("limpar") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula limpar <arquivo.nbx>");
                std::process::exit(2);
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: limpar aceita somente um arquivo .nbx.");
                std::process::exit(2);
            }
            match limpar_artefato(Path::new(&caminho)) {
                Ok(true) => println!("Artefato removido: {}", Path::new(&caminho).with_extension("out").display()),
                Ok(false) => println!("Nenhum artefato padrão encontrado para: {caminho}"),
                Err(erro) => {
                    eprintln!("Não foi possível limpar o artefato: {erro}");
                    std::process::exit(1);
                }
            }
        }
        Some("limpar-cache") => {
            let Some(projeto) = argumentos.next() else {
                eprintln!("Uso: nebula limpar-cache <diretorio-do-projeto>");
                std::process::exit(2);
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: limpar-cache aceita somente um diretório.");
                std::process::exit(2);
            }
            let cache = CacheCompilacao::abrir(Path::new(&projeto)).unwrap_or_else(|erro| {
                eprintln!("Não foi possível abrir o cache: {erro}");
                std::process::exit(1);
            });
            match cache.limpar() {
                Ok(resultado) => println!(
                    "Cache removido: {} arquivo(s), {} byte(s) liberado(s).",
                    resultado.removidos,
                    resultado.bytes_liberados
                ),
                Err(erro) => {
                    eprintln!("Não foi possível limpar o cache: {erro}");
                    std::process::exit(1);
                }
            }
        }
        Some("compilar") => {
            let Some(caminho) = argumentos.next() else { eprintln!("Uso: nebula compilar <arquivo.nbx> [--alvo <alvo>] [-o <saida>] [--otimizacao O0|O1|O2|O3|Os]"); std::process::exit(2); };
            let mut saida = PathBuf::from(&caminho).with_extension("out");
            let mut nivel = NivelOtimizacao::O1;
            let mut alvo = "linux-x86_64".to_owned();
            while let Some(arg) = argumentos.next() {
                if arg == "-o" {
                    let Some(valor) = argumentos.next() else { eprintln!("Erro: -o exige um caminho."); std::process::exit(2); };
                    saida = PathBuf::from(valor);
                } else if arg == "--otimizacao" {
                    let Some(valor) = argumentos.next() else { eprintln!("Erro: --otimizacao exige O0, O1, O2, O3 ou Os."); std::process::exit(2); };
                    nivel = nivel_otimizacao(&valor).unwrap_or_else(|erro| { eprintln!("{erro}"); std::process::exit(2); });
                } else if arg == "--alvo" {
                    let Some(valor) = argumentos.next() else { eprintln!("Erro: --alvo exige um identificador, por exemplo linux-x86_64."); std::process::exit(2); };
                    alvo = valor;
                } else if arg.starts_with("-O") {
                    nivel = nivel_otimizacao(&arg[1..]).unwrap_or_else(|erro| { eprintln!("{erro}"); std::process::exit(2); });
                } else {
                    eprintln!("Argumento desconhecido: {arg}");
                    std::process::exit(2);
                }
            }
            if let Err(erro) = validar_alvo(&alvo) {
                eprintln!("Erro de alvo: {erro}");
                std::process::exit(2);
            }
            if let Err(erro) = compilar_arquivo_com_nivel(Path::new(&caminho), &saida, nivel) {
                eprintln!("Erro de compilação: {erro}");
                std::process::exit(1);
            }
            println!("Executável gerado: {}", saida.display());
        }
        Some("executar" | "testar") => {
            if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
                eprintln!("O comando executar/testar exige um host Linux x86-64; use compilar para gerar o ELF em outro host.");
                std::process::exit(2);
            }
            let Some(caminho) = argumentos.next() else { eprintln!("Uso: nebula executar|testar <arquivo.nbx>"); std::process::exit(2); };
            if argumentos.next().is_some() { eprintln!("Erro: executar aceita somente um arquivo."); std::process::exit(2); }
            let temporario = std::env::temp_dir().join(format!("nebula-{}{}", std::process::id(), if cfg!(windows) { ".exe" } else { "" }));
            if let Err(erro) = compilar_arquivo(Path::new(&caminho), &temporario) { eprintln!("Erro de compilação: {erro}"); std::process::exit(1); }
            let status = Command::new(&temporario).status();
            let _ = std::fs::remove_file(&temporario);
            match status { Ok(s) if s.success() => {}, Ok(s) => std::process::exit(s.code().unwrap_or(1)), Err(e) => { eprintln!("Não foi possível executar o programa: {e}"); std::process::exit(1); } }
        }
        Some("depurar") => {
            if argumentos.next().as_deref() != Some("--dap") {
                eprintln!("Uso: nebula depurar --dap <arquivo.nbx> [--continuar-inicializacao]");
                std::process::exit(2);
            }
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula depurar --dap <arquivo.nbx> [--continuar-inicializacao]");
                std::process::exit(2);
            };
            let continuar = match argumentos.next().as_deref() {
                None => false,
                Some("--continuar-inicializacao") => true,
                Some(opcao) => {
                    eprintln!("Opção desconhecida para depurar: {opcao}");
                    std::process::exit(2);
                }
            };
            if argumentos.next().is_some() {
                eprintln!("Uso: nebula depurar --dap <arquivo.nbx> [--continuar-inicializacao]");
                std::process::exit(2);
            }
            if let Err(erro) = debugger::iniciar(Path::new(&caminho), continuar) {
                eprintln!("Falha no depurador Nebula: {erro}");
                std::process::exit(1);
            }
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
        Some("dependencias") => {
            let Some(acao) = argumentos.next() else {
                eprintln!("Uso: nebula dependencias <resolver|verificar> [diretorio]");
                std::process::exit(2);
            };
            if !matches!(acao.as_str(), "resolver" | "verificar") || argumentos.len() > 1 {
                eprintln!("Uso: nebula dependencias <resolver|verificar> [diretorio]");
                std::process::exit(2);
            }
            let diretorio = argumentos.next().map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            if acao == "resolver" {
                match nebula_pacotes::resolver(&diretorio) {
                    Ok(resolucao) => match nebula_pacotes::gravar_lockfile(&diretorio, &resolucao) {
                        Ok(destino) => {
                            println!("Dependências resolvidas: {}", resolucao.pacotes.len());
                            println!("Lockfile: {}", destino.display());
                        }
                        Err(erro) => {
                            eprintln!("Não foi possível gravar lockfile: {erro}");
                            std::process::exit(1);
                        }
                    },
                    Err(erro) => {
                        eprintln!("Erro ao resolver dependências: {erro}");
                        std::process::exit(1);
                    }
                }
            } else {
                match nebula_pacotes::verificar_lockfile(&diretorio) {
                    Ok(resolucao) => println!(
                        "Lockfile válido: {} dependência(s) verificada(s).",
                        resolucao.pacotes.len()
                    ),
                    Err(erro) => {
                        eprintln!("Falha na verificação do lockfile: {erro}");
                        std::process::exit(1);
                    }
                }
            }
        }
        Some("formatar") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula formatar <arquivo.nbx> [--check]");
                std::process::exit(2);
            };
            let check = match argumentos.next().as_deref() {
                None => false,
                Some("--check") => true,
                Some(opcao) => {
                    eprintln!("Opção desconhecida para formatar: {opcao}");
                    std::process::exit(2);
                }
            };
            if argumentos.next().is_some() {
                eprintln!("Uso: nebula formatar <arquivo.nbx> [--check]");
                std::process::exit(2);
            }
            match formatador::format_file(Path::new(&caminho), check) {
                Ok(true) if check => {
                    eprintln!("O arquivo precisa ser formatado: {caminho}");
                    std::process::exit(1);
                }
                Ok(true) => println!("Arquivo formatado: {caminho}"),
                Ok(false) => println!("Arquivo já está formatado: {caminho}"),
                Err(erro) => {
                    eprintln!("Erro ao formatar: {erro}");
                    std::process::exit(1);
                }
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
                eprintln!("Uso: nebula analisar <arquivo.nbx> [--json]");
                std::process::exit(2);
            };
            let json = match argumentos.next().as_deref() {
                None => false,
                Some("--json") => true,
                Some(opcao) => {
                    eprintln!("Erro: analisar aceita somente um arquivo e a opção --json; opção desconhecida: {opcao}");
                    std::process::exit(2);
                }
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: o comando 'analisar' aceita somente --json como opção.");
                std::process::exit(2);
            }
            analisar_arquivo(Path::new(&caminho), json);
        }
        Some("verificar" | "check") => {
            let Some(caminho) = argumentos.next() else {
                eprintln!("Uso: nebula verificar <arquivo.nbx> [--json]");
                std::process::exit(2);
            };
            let json = match argumentos.next().as_deref() {
                None => false,
                Some("--json") => true,
                Some(opcao) => {
                    eprintln!("Erro: verificar aceita somente um arquivo e a opção --json; opção desconhecida: {opcao}");
                    std::process::exit(2);
                }
            };
            if argumentos.next().is_some() {
                eprintln!("Erro: o comando 'verificar' aceita somente um arquivo e a opção --json.");
                eprintln!("Uso: nebula verificar <arquivo.nbx> [--json]");
                std::process::exit(2);
            }
            verificar(Path::new(&caminho), json);
        }
        Some(comando) => {
            eprintln!("Comando desconhecido: {comando}");
            eprintln!("Execute 'nebula ajuda' para consultar os comandos disponíveis.");
            std::process::exit(2);
        }
    }
}

fn criar_projeto(diretorio: &Path, nome: Option<&str>) -> Result<(), String> {
    let nome_projeto = match nome {
        Some(nome) => nome.to_owned(),
        None => nome_padrao_projeto(diretorio)?,
    };
    if !nome_valido_projeto(&nome_projeto) {
        return Err("o nome deve conter somente letras ASCII, números, '-' ou '_'".to_owned());
    }
    if diretorio.exists() {
        return Err(format!(
            "o diretório '{}' já existe; nenhum arquivo foi alterado",
            diretorio.display()
        ));
    }
    let pai = diretorio
        .parent()
        .filter(|pai| !pai.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(pai)
        .map_err(|erro| format!("não foi possível criar o diretório pai: {erro}"))?;
    std::fs::create_dir(diretorio)
        .map_err(|erro| format!("não foi possível criar '{}': {erro}", diretorio.display()))?;

    let manifesto = format!("[pacote]\nnome = \"{nome_projeto}\"\nversao = \"0.1.0\"\n");
    let fonte = "funcao principal(): vazio {\n}\n";
    let resultado = (|| {
        escrever_novo(
            diretorio.join("nebula.toml").as_path(),
            manifesto.as_bytes(),
        )?;
        escrever_novo(diretorio.join("principal.nbx").as_path(), fonte.as_bytes())?;
        Ok(())
    })();
    if resultado.is_err() {
        let _ = std::fs::remove_dir_all(diretorio);
    }
    resultado
}

fn nome_padrao_projeto(diretorio: &Path) -> Result<String, String> {
    let nome = diretorio
        .file_name()
        .and_then(|nome| nome.to_str())
        .ok_or_else(|| "o diretório precisa ter um nome UTF-8".to_owned())?;
    let mut saida = String::new();
    for caractere in nome.chars() {
        if caractere.is_ascii_alphanumeric() || caractere == '_' || caractere == '-' {
            saida.push(caractere.to_ascii_lowercase());
        } else if !saida.ends_with('-') {
            saida.push('-');
        }
    }
    let saida = saida.trim_matches(['-', '_']).to_owned();
    if saida.is_empty() {
        return Err("não foi possível derivar um nome válido do diretório; use --nome".to_owned());
    }
    Ok(saida)
}

fn nome_valido_projeto(nome: &str) -> bool {
    !nome.is_empty()
        && nome
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn escrever_novo(caminho: &Path, conteudo: &[u8]) -> Result<(), String> {
    use std::io::Write as _;
    let mut arquivo = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(caminho)
        .map_err(|erro| format!("não foi possível criar '{}': {erro}", caminho.display()))?;
    arquivo
        .write_all(conteudo)
        .map_err(|erro| format!("não foi possível gravar '{}': {erro}", caminho.display()))
}

fn limpar_artefato(fonte: &Path) -> Result<bool, String> {
    let metadados_fonte = std::fs::symlink_metadata(fonte)
        .map_err(|erro| format!("não foi possível inspecionar '{}': {erro}", fonte.display()))?;
    if !metadados_fonte.is_file() || metadados_fonte.file_type().is_symlink() {
        return Err(
            "a origem precisa ser um arquivo regular .nbx, não um link simbólico".to_owned(),
        );
    }
    if fonte.extension().and_then(|extensao| extensao.to_str()) != Some("nbx") {
        return Err("a origem precisa usar a extensão .nbx".to_owned());
    }
    let artefato = fonte.with_extension("out");
    let metadados = match std::fs::symlink_metadata(&artefato) {
        Ok(metadados) => metadados,
        Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(erro) => {
            return Err(format!(
                "não foi possível inspecionar '{}': {erro}",
                artefato.display()
            ));
        }
    };
    if !metadados.is_file() || metadados.file_type().is_symlink() {
        return Err("o artefato não é um arquivo regular; nada foi removido".to_owned());
    }
    use std::io::Read as _;
    let mut arquivo = std::fs::File::open(&artefato)
        .map_err(|erro| format!("não foi possível abrir '{}': {erro}", artefato.display()))?;
    let mut cabecalho = [0_u8; 20];
    let quantidade = arquivo
        .read(&mut cabecalho)
        .map_err(|erro| format!("não foi possível ler o cabeçalho do artefato: {erro}"))?;
    let elf64_linux_x86_64 = quantidade == cabecalho.len()
        && cabecalho[0..4] == [0x7f, b'E', b'L', b'F']
        && cabecalho[4] == 2
        && cabecalho[5] == 1
        && cabecalho[18..20] == [0x3e, 0x00];
    if !elf64_linux_x86_64 {
        return Err("o arquivo .out não corresponde ao formato ELF64 Linux x86-64 suportado; nada foi removido".to_owned());
    }
    std::fs::remove_file(&artefato)
        .map_err(|erro| format!("não foi possível remover '{}': {erro}", artefato.display()))?;
    Ok(true)
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
    let (modulos, programas_modulos, _) = carregar_modulos(caminho, &sintaxe.programa)?;
    let resultado = baixar(&sintaxe.programa, &modulos);
    if !resultado.diagnosticos.is_empty() {
        return Err(resultado.diagnosticos.join("\n"));
    }
    let mut programa = resultado.programa.expect("IR validada");
    ligar_modulos(&mut programa, programas_modulos);
    let execucao = nebula_ir::executar(&programa, 1_000_000)?;
    Ok(execucao.saida)
}

fn ligar_modulos(programa: &mut ProgramaIr, programas_modulos: Vec<(String, ProgramaIr)>) {
    for (caminho_modulo, modulo) in programas_modulos {
        for (nome, mut funcao) in modulo.funcoes {
            let nome_completo = format!("{caminho_modulo}::{nome}");
            funcao.nome = nome_completo.clone();
            programa.funcoes.insert(nome_completo, funcao);
        }
    }
}

fn resolver_arquivo_modulo(origem: &Path, caminho: &str) -> Result<PathBuf, String> {
    let componentes = caminho.split("::").collect::<Vec<_>>();
    if componentes.is_empty()
        || componentes.iter().any(|componente| {
            componente.is_empty()
                || !componente.chars().enumerate().all(|(indice, caractere)| {
                    if indice == 0 {
                        caractere == '_' || caractere.is_alphabetic()
                    } else {
                        caractere == '_' || caractere.is_alphanumeric()
                    }
                })
        })
    {
        return Err(format!(
            "Caminho de módulo inválido: '{caminho}'. Use identificadores separados por '::'."
        ));
    }
    let base = origem.parent().unwrap_or_else(|| Path::new("."));
    let base_canonica = std::fs::canonicalize(base).map_err(|erro| {
        format!(
            "Não foi possível resolver diretório de módulos '{}': {erro}",
            base.display()
        )
    })?;
    let mut relativo = PathBuf::new();
    for componente in componentes {
        relativo.push(componente);
    }
    if relativo.extension().is_none() {
        relativo.set_extension("nbx");
    }
    let candidato = base_canonica.join(relativo);
    if !candidato.is_file() {
        return Err(format!(
            "Módulo '{caminho}' não encontrado. Esperado arquivo relativo a '{}'.",
            base_canonica.display()
        ));
    }
    let arquivo_canonico = std::fs::canonicalize(&candidato).map_err(|erro| {
        format!(
            "Não foi possível resolver módulo '{}': {erro}",
            candidato.display()
        )
    })?;
    if !arquivo_canonico.starts_with(&base_canonica) {
        return Err(format!(
            "O módulo '{caminho}' aponta para fora do diretório permitido '{}'.",
            base_canonica.display()
        ));
    }
    Ok(arquivo_canonico)
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
        "inteiro_u8" => Tipo::InteiroU8,
        "inteiro_u16" => Tipo::InteiroU16,
        "inteiro_u32" => Tipo::InteiroU32,
        "inteiro_u64" => Tipo::InteiroU64,
        "inteiro_u128" => Tipo::InteiroU128,
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

#[rustfmt::skip]
fn carregar_modulos(
    origem: &Path,
    programa: &Programa,
) -> ResultadoCarregamentoModulos {
    let mut assinaturas = ModulosDisponiveis::new();
    let mut programas = Vec::new();
    let mut fontes_lidas = Vec::new();
    let mut cache = std::collections::HashMap::new();
    let mut ativos = HashSet::new();
    for item in &programa.itens {
        let NoAst::Incluir { caminho, .. } = item else {
            continue;
        };
        let exportadas = carregar_modulo_recursivo(
            origem,
            caminho,
            caminho,
            &mut cache,
            &mut ativos,
            &mut programas,
            &mut fontes_lidas,
        )?;
        assinaturas.insert(caminho.clone(), exportadas);
    }
    Ok((assinaturas, programas, fontes_lidas))
}

#[rustfmt::skip]
fn carregar_modulo_recursivo(
    origem: &Path,
    caminho: &str,
    identificador: &str,
    cache: &mut HashMap<String, std::collections::HashMap<String, AssinaturaFuncao>>,
    ativos: &mut HashSet<PathBuf>,
    programas: &mut Vec<(String, ProgramaIr)>,
    fontes_lidas: &mut Vec<(PathBuf, Vec<u8>)>,
) -> Result<std::collections::HashMap<String, AssinaturaFuncao>, String> {
    if let Some(assinaturas) = cache.get(identificador) {
        return Ok(assinaturas.clone());
    }
    let arquivo = resolver_arquivo_modulo(origem, caminho)?;
    let arquivo_canonico = std::fs::canonicalize(&arquivo)
        .map_err(|erro| format!("Não foi possível resolver '{}': {erro}", arquivo.display()))?;
    if !ativos.insert(arquivo_canonico.clone()) {
        return Err(format!(
            "Ciclo de dependências detectado ao carregar o módulo '{identificador}'."
        ));
    }

    let resultado = (|| {
        let bytes = std::fs::read(&arquivo).map_err(|erro| {
            format!(
                "Não foi possível ler módulo '{}': {erro}",
                arquivo.display()
            )
        })?;
        fontes_lidas.push((arquivo_canonico.clone(), bytes.clone()));
        let codigo = String::from_utf8(bytes)
            .map_err(|erro| format!("Módulo '{}' não é UTF-8 válido: {erro}", arquivo.display()))?;
        let fonte = ArquivoFonte::novo(&arquivo, codigo);
        let sintaxe = analisar(&fonte);
        if !sintaxe.diagnosticos.is_empty() {
            return Err(format!(
                "Erro de sintaxe no módulo '{}': {}",
                identificador,
                sintaxe
                    .diagnosticos
                    .iter()
                    .map(|d| format!("{} — {}", d.codigo, d.mensagem))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
        if sintaxe.programa.itens.iter().any(|item| {
            !matches!(item, NoAst::Funcao { .. } | NoAst::Tipo { .. } | NoAst::Estrutura { .. } | NoAst::Enumeracao { .. } | NoAst::Incluir { .. })
        }) {
            return Err(format!(
                "O módulo '{identificador}' contém inicializadores globais; módulos importados devem conter funções, aliases, estruturas, enumerações e imports."
            ));
        }

        let mut modulos_disponiveis = ModulosDisponiveis::new();
        let mut caminhos_canonicos = HashMap::new();
        for item in &sintaxe.programa.itens {
            let NoAst::Incluir {
                caminho: caminho_filho,
                ..
            } = item
            else {
                continue;
            };
            let id_filho = format!("{identificador}::{caminho_filho}");
            let exportadas = carregar_modulo_recursivo(
                &arquivo,
                caminho_filho,
                &id_filho,
                cache,
                ativos,
                programas,
                fontes_lidas,
            )?;
            modulos_disponiveis.insert(caminho_filho.clone(), exportadas);
            caminhos_canonicos.insert(caminho_filho.clone(), id_filho);
        }

        let mut exportadas = HashMap::new();
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
                    .map(|(_, tipo)| {
                        tipo_assinatura_modulo(tipo).ok_or_else(|| {
                            format!("Tipo '{tipo}' não suportado na assinatura pública '{identificador}::{nome}'.")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let retorno_nome = retorno.as_deref().unwrap_or("vazio");
                let retorno = tipo_assinatura_modulo(retorno_nome).ok_or_else(|| {
                    format!("Tipo de retorno '{retorno_nome}' não suportado em '{identificador}::{nome}'.")
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
                "O módulo '{identificador}' não exporta nenhuma função pública."
            ));
        }

        let resultado_ir = baixar(&sintaxe.programa, &modulos_disponiveis);
        if !resultado_ir.diagnosticos.is_empty() {
            return Err(format!(
                "Erro semântico no módulo '{identificador}': {}",
                resultado_ir.diagnosticos.join("\n")
            ));
        }
        let mut modulo_ir = resultado_ir.programa.expect("módulo validado");
        let nomes = modulo_ir.funcoes.keys().cloned().collect::<HashSet<_>>();
        for funcao in modulo_ir.funcoes.values_mut() {
            for instrucao in &mut funcao.corpo {
                qualificar_chamadas_modulo(instrucao, identificador, &nomes, &caminhos_canonicos);
            }
        }
        programas.push((identificador.to_owned(), modulo_ir));
        cache.insert(identificador.to_owned(), exportadas.clone());
        Ok(exportadas)
    })();
    ativos.remove(&arquivo_canonico);
    resultado
}

#[rustfmt::skip]
fn qualificar_chamadas_modulo(
    instrucao: &mut InstrucaoIr,
    caminho: &str,
    funcoes: &HashSet<String>,
    caminhos_canonicos: &HashMap<String, String>,
) {
    match instrucao {
        InstrucaoIr::Localizada { instrucao, .. } => {
            qualificar_chamadas_modulo(instrucao, caminho, funcoes, caminhos_canonicos);
        }
        InstrucaoIr::Declarar { valor, .. } | InstrucaoIr::Expressao(valor) => {
            qualificar_expressao_modulo(valor, caminho, funcoes, caminhos_canonicos);
        }
        InstrucaoIr::Bloco(itens) => {
            for item in itens {
                qualificar_chamadas_modulo(item, caminho, funcoes, caminhos_canonicos);
            }
        }
        InstrucaoIr::Escolha { valor, casos } => {
            qualificar_expressao_modulo(valor, caminho, funcoes, caminhos_canonicos);
            for caso in casos {
                for item in &mut caso.corpo {
                    qualificar_chamadas_modulo(item, caminho, funcoes, caminhos_canonicos);
                }
            }
        }
        InstrucaoIr::Se {
            condicao,
            entao,
            senao,
        } => {
            qualificar_expressao_modulo(condicao, caminho, funcoes, caminhos_canonicos);
            for item in entao.iter_mut().chain(senao.iter_mut()) {
                qualificar_chamadas_modulo(item, caminho, funcoes, caminhos_canonicos);
            }
        }
        InstrucaoIr::Enquanto { condicao, corpo } => {
            qualificar_expressao_modulo(condicao, caminho, funcoes, caminhos_canonicos);
            for item in corpo {
                qualificar_chamadas_modulo(item, caminho, funcoes, caminhos_canonicos);
            }
        }
        InstrucaoIr::Cada { iteravel, corpo, .. } => {
            qualificar_expressao_modulo(iteravel, caminho, funcoes, caminhos_canonicos);
            for item in corpo {
                qualificar_chamadas_modulo(item, caminho, funcoes, caminhos_canonicos);
            }
        }
        InstrucaoIr::Pulso { intervalo, corpo } => {
            qualificar_expressao_modulo(intervalo, caminho, funcoes, caminhos_canonicos);
            for item in corpo {
                qualificar_chamadas_modulo(item, caminho, funcoes, caminhos_canonicos);
            }
        }
        InstrucaoIr::Para {
            inicial,
            condicao,
            atualizacao,
            corpo,
        } => {
            if let Some(inicial) = inicial {
                qualificar_chamadas_modulo(inicial, caminho, funcoes, caminhos_canonicos);
            }
            if let Some(condicao) = condicao {
                qualificar_expressao_modulo(condicao, caminho, funcoes, caminhos_canonicos);
            }
            if let Some(atualizacao) = atualizacao {
                qualificar_expressao_modulo(atualizacao, caminho, funcoes, caminhos_canonicos);
            }
            for item in corpo {
                qualificar_chamadas_modulo(item, caminho, funcoes, caminhos_canonicos);
            }
        }
        InstrucaoIr::Devolver(valor) => {
            if let Some(valor) = valor {
                qualificar_expressao_modulo(valor, caminho, funcoes, caminhos_canonicos);
            }
        }
        InstrucaoIr::Parar | InstrucaoIr::Seguir => {}
    }
}

#[rustfmt::skip]
fn qualificar_expressao_modulo(
    expressao: &mut ExpressaoIr,
    caminho: &str,
    funcoes: &HashSet<String>,
    caminhos_canonicos: &HashMap<String, String>,
) {
    let mut chamada_local = None;
    match expressao {
        ExpressaoIr::Lista(itens) | ExpressaoIr::Tupla(itens) => {
            for item in itens {
                qualificar_expressao_modulo(item, caminho, funcoes, caminhos_canonicos);
            }
        }
        ExpressaoIr::Normalizar { valor, .. } | ExpressaoIr::Conversao { valor, .. } | ExpressaoIr::Unaria { valor, .. } => {
            qualificar_expressao_modulo(valor, caminho, funcoes, caminhos_canonicos);
        }
        ExpressaoIr::ChamadaIndireta { alvo, argumentos } => {
            qualificar_expressao_modulo(alvo, caminho, funcoes, caminhos_canonicos);
            for argumento in argumentos {
                qualificar_expressao_modulo(argumento, caminho, funcoes, caminhos_canonicos);
            }
        }
        ExpressaoIr::ReferenciaFuncao { nome, tipo } => {
            if funcoes.contains(nome) {
                *expressao = ExpressaoIr::ReferenciaFuncaoQualificada {
                    modulo: caminho.to_owned(),
                    nome: nome.clone(),
                    tipo: tipo.clone(),
                };
            }
        }
        ExpressaoIr::Binaria {
            esquerda, direita, ..
        } => {
            qualificar_expressao_modulo(esquerda, caminho, funcoes, caminhos_canonicos);
            qualificar_expressao_modulo(direita, caminho, funcoes, caminhos_canonicos);
        }
        ExpressaoIr::Atribuicao { valor, .. } | ExpressaoIr::AtribuicaoCampo { valor, .. } => {
            qualificar_expressao_modulo(valor, caminho, funcoes, caminhos_canonicos);
        }
        ExpressaoIr::Chamada { nome, argumentos } => {
            for argumento in argumentos.iter_mut() {
                qualificar_expressao_modulo(argumento, caminho, funcoes, caminhos_canonicos);
            }
            if funcoes.contains(nome) {
                chamada_local = Some(nome.clone());
            }
        }
        ExpressaoIr::ChamadaQualificada {
            modulo, argumentos, ..
        } => {
            if let Some(canonico) = caminhos_canonicos.get(modulo) {
                *modulo = canonico.clone();
            }
            for argumento in argumentos {
                qualificar_expressao_modulo(argumento, caminho, funcoes, caminhos_canonicos);
            }
        }
        ExpressaoIr::Indexacao { alvo, indice } => {
            qualificar_expressao_modulo(alvo, caminho, funcoes, caminhos_canonicos);
            qualificar_expressao_modulo(indice, caminho, funcoes, caminhos_canonicos);
        }
        ExpressaoIr::Membro { alvo, .. } => {
            qualificar_expressao_modulo(alvo, caminho, funcoes, caminhos_canonicos);
        }
        ExpressaoIr::ConstrucaoEstrutura { campos, .. } => {
            for (_, valor) in campos {
                qualificar_expressao_modulo(valor, caminho, funcoes, caminhos_canonicos);
            }
        }
        ExpressaoIr::ConstrucaoEnumeracao { valores, .. } => {
            for valor in valores { qualificar_expressao_modulo(valor, caminho, funcoes, caminhos_canonicos); }
        }
        ExpressaoIr::Constante(_) | ExpressaoIr::Variavel(_) | ExpressaoIr::ReferenciaFuncaoQualificada { .. } => {}
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
    let conteudo = String::from_utf8(bytes.clone())
        .map_err(|e| format!("arquivo não contém UTF-8 válido no byte {}", e.utf8_error().valid_up_to()))?;
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let sintaxe = analisar(&fonte);
    if !sintaxe.diagnosticos.is_empty() {
        return Err(sintaxe.diagnosticos.iter()
            .map(|d| format!("{} — {}", d.codigo, d.mensagem))
            .collect::<Vec<_>>().join("\n"));
    }
    let (modulos, programas_modulos, mut fontes_modulos) =
        carregar_modulos(caminho, &sintaxe.programa)?;
    let nivel_chave = format!("{nivel:?}");
    fontes_modulos.sort_by(|a, b| a.0.cmp(&b.0));
    let mut componentes_cache = vec![
        b"nebula-build-v1".to_vec(),
        env!("CARGO_PKG_VERSION").as_bytes().to_vec(),
        b"linux-x86_64-elf64".to_vec(),
        nivel_chave.into_bytes(),
        bytes.clone(),
    ];
    for (arquivo_modulo, codigo_modulo) in fontes_modulos {
        componentes_cache.push(arquivo_modulo.to_string_lossy().as_bytes().to_vec());
        componentes_cache.push(codigo_modulo);
    }
    let fingerprint_compilador = std::env::current_exe()
        .ok()
        .and_then(|executavel| CacheCompilacao::fingerprint_arquivo(&executavel).ok());
    let chave_cache = fingerprint_compilador.map(|fingerprint| {
        componentes_cache.push(fingerprint.into_bytes());
        let componentes = componentes_cache
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        CacheCompilacao::calcular_chave(&componentes)
    });
    let diretorio_projeto = caminho
        .parent()
        .filter(|diretorio| !diretorio.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let cache = if chave_cache.is_some() {
        CacheCompilacao::abrir(diretorio_projeto).ok()
    } else {
        None
    };
    if let (Some(cache), Some(chave_cache)) = (&cache, &chave_cache) {
        if let Ok(Some(binario)) = cache.obter(chave_cache) {
            if executavel_elf64_valido(&binario) {
                return gravar_executavel_atomico(saida, &binario);
            }
        }
    }
    let ir = baixar(&sintaxe.programa, &modulos);
    if !ir.diagnosticos.is_empty() {
        return Err(ir.diagnosticos.join("\n"));
    }
    let mut programa = ir.programa.expect("IR validada");
    ligar_modulos(&mut programa, programas_modulos);
    nebula_ir::otimizador::otimizar(&mut programa, nivel)?;
    let binario = nebula_backend::gerar_executavel(&programa)?;
    if let (Some(cache), Some(chave_cache)) = (&cache, &chave_cache) {
        let _ = cache.armazenar(chave_cache, &binario);
    }
    gravar_executavel_atomico(saida, &binario)
}

fn executavel_elf64_valido(binario: &[u8]) -> bool {
    binario.len() >= 64
        && binario.get(0..4) == Some(&[0x7f, b'E', b'L', b'F'])
        && binario.get(4) == Some(&2)
        && binario.get(5) == Some(&1)
        && binario.get(18..20) == Some(&[0x3e, 0x00])
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
    println!("  criar <diretório>     Cria um projeto Nebula vazio");
    println!("  limpar <arquivo>      Remove o executável .out padrão do arquivo");
    println!("  limpar-cache <projeto> Remove os artefatos em .nebula/cache");
    println!("  versao                Exibe a versão da ferramenta");
    println!("  alvos                 Lista os alvos com backend implementado");
    println!("  verificar <arquivo>   Analisa sintaxe (aceita --json)");
    println!("  formatar <arquivo>    Formata indentação e espaços sem reescrever tokens");
    println!("  analisar <arquivo>    Verifica nomes, escopos e tipos (aceita --json)");
    println!("  ir <arquivo>          Gera e exibe a representação intermediária");
    println!(
        "  compilar <arquivo>    Gera executável nativo (--alvo linux-x86_64, -O0/-O1/-O2/-O3/-Os)"
    );
    println!("  interpretar <arquivo> Executa a IR pelo runtime Rust de referência");
    println!("  depurar --dap <arquivo> Inicia o adaptador DAP do VS Code");
    println!("  dependencias resolver [diretorio] Resolve dependências e grava nebula.lock");
    println!("  dependencias verificar [diretorio] Confere o lockfile sem alterá-lo");
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
    let (modulos, programas_modulos, _) = carregar_modulos(caminho, &sintaxe.programa)
        .unwrap_or_else(|erro| {
            eprintln!("{erro}");
            std::process::exit(1);
        });
    let resultado = baixar(&sintaxe.programa, &modulos);
    if !resultado.diagnosticos.is_empty() {
        for diagnostico in &resultado.diagnosticos {
            eprintln!("{diagnostico}");
        }
        std::process::exit(1);
    }
    let mut programa = resultado.programa.expect("baixamento validado");
    ligar_modulos(&mut programa, programas_modulos);
    println!("{programa:#?}");
}

fn escapar_json(texto: &str) -> String {
    let mut saida = String::from("\"");
    for caractere in texto.chars() {
        match caractere {
            '"' => saida.push_str("\\\""),
            '\\' => saida.push_str("\\\\"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            c if c <= '\u{001f}' => saida.push_str(&format!("\\u{:04x}", c as u32)),
            c => saida.push(c),
        }
    }
    saida.push('"');
    saida
}

fn diagnosticos_json(
    caminho: &Path,
    diagnosticos: &[Diagnostico],
    fonte: Option<&ArquivoFonte>,
) -> String {
    let itens = diagnosticos.iter().map(|diagnostico| {
        let local = diagnostico.intervalo.and_then(|intervalo| {
            fonte.and_then(|arquivo| arquivo.linha_coluna(intervalo.inicio))
                .map(|(linha, coluna)| (intervalo, linha, coluna))
        });
        let intervalo = diagnostico.intervalo.map_or_else(
            || "null".to_owned(),
            |i| format!("{{\"inicio\":{},\"fim\":{}}}", i.inicio, i.fim),
        );
        let (linha, coluna) = local.map_or(("null".to_owned(), "null".to_owned()), |(_, l, c)| {
            (l.to_string(), c.to_string())
        });
        let severidade = match diagnostico.severidade {
            Severidade::Erro => "erro",
            Severidade::Aviso => "aviso",
            Severidade::Informacao => "informacao",
        };
        format!(
            "{{\"codigo\":{},\"severidade\":{},\"mensagem\":{},\"intervalo\":{},\"linha\":{},\"coluna\":{}}}",
            escapar_json(diagnostico.codigo),
            escapar_json(severidade),
            escapar_json(&diagnostico.mensagem),
            intervalo,
            linha,
            coluna
        )
    }).collect::<Vec<_>>().join(",");
    format!(
        "{{\"arquivo\":{},\"valido\":{},\"diagnosticos\":[{}]}}",
        escapar_json(&caminho.display().to_string()),
        diagnosticos.is_empty(),
        itens
    )
}

fn analisar_arquivo(caminho: &Path, json: bool) {
    let bytes = match std::fs::read(caminho) {
        Ok(bytes) => bytes,
        Err(erro) => {
            let diagnostico = Diagnostico::novo(
                "N0001",
                Severidade::Erro,
                format!("Não foi possível ler o arquivo: {erro}"),
                None,
            );
            if json {
                println!("{}", diagnosticos_json(caminho, &[diagnostico], None));
            } else {
                eprintln!(
                    "Erro N0001: não foi possível ler '{}': {erro}",
                    caminho.display()
                );
            }
            std::process::exit(1);
        }
    };
    let conteudo = match String::from_utf8(bytes) {
        Ok(conteudo) => conteudo,
        Err(erro) => {
            let deslocamento = erro.utf8_error().valid_up_to();
            let diagnostico = Diagnostico::novo(
                "N1000",
                Severidade::Erro,
                format!(
                    "O arquivo não contém UTF-8 válido; sequência inválida a partir do byte {deslocamento}."
                ),
                None,
            );
            if json {
                println!("{}", diagnosticos_json(caminho, &[diagnostico], None));
            } else {
                eprintln!(
                    "Erro N1000: arquivo '{}' não contém UTF-8 válido; sequência inválida a partir do byte {}.",
                    caminho.display(),
                    deslocamento
                );
            }
            std::process::exit(1);
        }
    };
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let sintaxe = analisar(&fonte);
    if !sintaxe.diagnosticos.is_empty() {
        if json {
            println!(
                "{}",
                diagnosticos_json(caminho, &sintaxe.diagnosticos, Some(&fonte))
            );
            std::process::exit(1);
        }
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
    let (modulos, _, _) = match carregar_modulos(caminho, &sintaxe.programa) {
        Ok(carregados) => carregados,
        Err(erro) => {
            if json {
                let diagnostico = Diagnostico::novo("N0001", Severidade::Erro, erro, None);
                println!(
                    "{}",
                    diagnosticos_json(caminho, &[diagnostico], Some(&fonte))
                );
            } else {
                eprintln!("{erro}");
            }
            std::process::exit(1);
        }
    };
    let resultado = analisar_com_modulos(&sintaxe.programa, &modulos);
    if json {
        println!(
            "{}",
            diagnosticos_json(caminho, &resultado.diagnosticos, Some(&fonte))
        );
    } else if resultado.diagnosticos.is_empty() {
        println!("Arquivo válido semanticamente: {}", caminho.display());
    } else {
        for diagnostico in &resultado.diagnosticos {
            eprintln!("{} — {}", diagnostico.codigo, diagnostico.mensagem);
        }
    }
    if !resultado.diagnosticos.is_empty() {
        std::process::exit(1);
    }
}

fn verificar(caminho: &Path, json: bool) {
    let bytes = match std::fs::read(caminho) {
        Ok(bytes) => bytes,
        Err(erro) => {
            let diagnostico = Diagnostico::novo(
                "N0001",
                Severidade::Erro,
                format!("Não foi possível ler o arquivo: {erro}"),
                None,
            );
            if json {
                println!("{}", diagnosticos_json(caminho, &[diagnostico], None));
            } else {
                eprintln!(
                    "Erro N0001: não foi possível ler '{}': {erro}",
                    caminho.display()
                );
            }
            std::process::exit(1);
        }
    };
    let conteudo = match String::from_utf8(bytes) {
        Ok(conteudo) => conteudo,
        Err(erro) => {
            let deslocamento = erro.utf8_error().valid_up_to();
            let diagnostico = Diagnostico::novo(
                "N1000",
                Severidade::Erro,
                format!(
                    "O arquivo não contém UTF-8 válido; sequência inválida a partir do byte {deslocamento}."
                ),
                None,
            );
            if json {
                println!("{}", diagnosticos_json(caminho, &[diagnostico], None));
            } else {
                eprintln!(
                    "Erro N1000: arquivo '{}' não contém UTF-8 válido; sequência inválida a partir do byte {}.",
                    caminho.display(),
                    deslocamento
                );
            }
            std::process::exit(1);
        }
    };
    let fonte = ArquivoFonte::novo(caminho, conteudo);
    let resultado = analisar(&fonte);
    if json {
        println!(
            "{}",
            diagnosticos_json(caminho, &resultado.diagnosticos, Some(&fonte))
        );
        if !resultado.diagnosticos.is_empty() {
            std::process::exit(1);
        }
        return;
    }
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
        let identificador = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio do sistema")
            .as_nanos();
        let prefixo = format!("nebula-o2-{}-{identificador}", std::process::id());
        let origem = std::env::temp_dir().join(format!("{prefixo}.nbx"));
        let saida = std::env::temp_dir().join(&prefixo);
        std::fs::write(
            &origem,
            "funcao principal(): inteiro { quando verdadeiro { devolver 7 } senao { devolver 9 } }",
        )
        .unwrap();
        compilar_arquivo_com_nivel(&origem, &saida, NivelOtimizacao::O2)
            .expect("compilação O2 válida");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável gerado não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(7));
        let _ = std::fs::remove_file(origem);
        let _ = std::fs::remove_file(saida);
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[rustfmt::skip]
    #[test]
    fn compila_e_executa_chamada_de_modulo_no_backend_nativo() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio = std::env::temp_dir().join(format!(
            "nebula-modulo-elf-{}-{sufixo}",
            std::process::id()
        ));
        std::fs::create_dir_all(&diretorio).unwrap();
        std::fs::write(
            diretorio.join("Base.nbx"),
            "publico funcao valor(): inteiro { devolver 40 }",
        )
        .unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            "incluir Base\nfuncao principal(): inteiro { devolver Base::valor() + 2 }",
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação com módulo local");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável gerado não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn compila_e_executa_retorno_e_chamada_indireta_de_funcao() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio = std::env::temp_dir().join(format!(
            "nebula-funcao-indireta-{}-{sufixo}",
            std::process::id()
        ));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            "funcao somar(valor: inteiro): inteiro { devolver valor + 2 }\nfuncao obter(): funcao(inteiro): inteiro { devolver somar }\nfuncao principal(): inteiro { devolver obter()(40) }",
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação com função indireta");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável gerado não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn compila_estrutura_de_dois_campos_no_abi() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio = std::env::temp_dir().join(format!(
            "nebula-abi-estrutura-{}-{sufixo}",
            std::process::id()
        ));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"estrutura Ponto { x: inteiro64, y: inteiro64 }
funcao deslocar(ponto: Ponto): Ponto {
    devolver Ponto { x: ponto.x + 1, y: ponto.y + 2 }
}
funcao principal(): inteiro {
    variavel original = Ponto { x: 39, y: 0 }
    original.x = 40
    variavel resultado = deslocar(original)
    quando resultado.x == 41 e resultado.y == 2 {
        devolver 42
    } senao {
        devolver 0
    }
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação de ABI de estrutura");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável de estrutura não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn compila_opcionais_e_resultados_no_abi() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio = std::env::temp_dir().join(format!(
            "nebula-abi-opcional-resultado-{}-{sufixo}",
            std::process::id()
        ));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"funcao extrair(valor: Opcional<inteiro64>): inteiro {
    escolha valor {
        caso Opcional::Algum(conteudo) { devolver conteudo }
        caso Opcional::Nenhum { devolver 0 }
    }
}
funcao ler(valor: Resultado<inteiro64,inteiro64>): inteiro {
    escolha valor {
        caso Resultado::Sucesso(conteudo) { devolver conteudo }
        caso Resultado::Falha(erro) { devolver erro }
    }
}
funcao principal(): inteiro {
    variavel opcional: Opcional<inteiro64> = Opcional::Algum(40)
    variavel resultado: Resultado<inteiro64,inteiro64> = Resultado::Sucesso(2)
    devolver extrair(opcional) + ler(resultado)
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação de opcionais e resultados");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável opcional/resultado não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn compila_enum_com_payload_e_pattern_matching() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio =
            std::env::temp_dir().join(format!("nebula-abi-enum-{}-{sufixo}", std::process::id()));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"enumeracao Resposta { Sucesso(inteiro64), Falha(inteiro64), Vazio }
funcao transformar(resposta: Resposta): Resposta {
    escolha resposta {
        caso Resposta::Sucesso(valor) { devolver Resposta::Falha(valor + 1) }
        caso Resposta::Falha(erro) { devolver Resposta::Sucesso(erro + 1) }
        caso Resposta::Vazio { devolver Resposta::Vazio }
    }
}
funcao principal(): inteiro {
    variavel resultado = transformar(Resposta::Sucesso(41))
    escolha resultado {
        caso Resposta::Sucesso(valor) { devolver 0 }
        caso Resposta::Falha(erro) { devolver erro }
        caso Resposta::Vazio { devolver 1 }
    }
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação de enumeração com payload");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável de enumeração não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn compila_tuplas_de_dois_elementos_no_abi() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio =
            std::env::temp_dir().join(format!("nebula-abi-tupla-{}-{sufixo}", std::process::id()));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"funcao inverter(par: Tupla<inteiro64,inteiro64>): Tupla<inteiro64,inteiro64> {
    devolver (par[1], par[0])
}
funcao principal(): inteiro {
    variavel original = (40, 2)
    variavel resultado = inverter(original)
    quando resultado[0] == 2 e resultado[1] == 40 {
        devolver 42
    } senao {
        devolver 0
    }
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação de ABI de tupla");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável de tupla não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn converte_inteiros_entre_64_e_128_bits() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio = std::env::temp_dir().join(format!(
            "nebula-conversao-inteiro128-{}-{sufixo}",
            std::process::id()
        ));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"funcao principal(): inteiro {
    variavel negativo: inteiro128 = -42
    variavel estreito: inteiro64 = inteiro64(negativo)
    variavel ampliado: inteiro128 = inteiro128(estreito)
    variavel sem_sinal: inteiro_u128 = inteiro_u128(42)
    variavel de_volta: inteiro_u64 = inteiro_u64(sem_sinal)
    quando estreito == -42 e ampliado == negativo e de_volta == 42 {
        devolver 42
    } senao {
        devolver 0
    }
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação de conversões 64/128");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável de conversão 128 não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn compila_chamadas_e_retorno_com_inteiros128() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio = std::env::temp_dir().join(format!(
            "nebula-abi-inteiro128-{}-{sufixo}",
            std::process::id()
        ));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"variavel global: inteiro128 = 1208925819614629174706176
funcao somar(a: inteiro128, b: inteiro128): inteiro128 {
    devolver a + b
}
funcao identidade(valor: inteiro128): inteiro128 {
    devolver valor
}
funcao identidade_sem_sinal(valor: inteiro_u128): inteiro_u128 {
    devolver valor
}
funcao principal(): inteiro {
    variavel soma = somar(global, global)
    variavel esperado: inteiro128 = 2417851639229258349412352
    variavel negativo: inteiro128 = -42
    variavel positivo: inteiro128 = 42
    variavel maximo: inteiro_u128 = 340282366920938463463374607431768211455
    variavel zero: inteiro_u128 = 0
    quando soma == esperado e identidade(soma) == esperado e negativo < positivo e -positivo == negativo e identidade_sem_sinal(maximo) == maximo e maximo > zero {
        devolver 42
    } senao {
        devolver 0
    }
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação com ABI inteiro128");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável inteiro128 não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn compila_conversao_e_aritmetica_decimal32_com_variaveis() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio =
            std::env::temp_dir().join(format!("nebula-decimal32-{}-{sufixo}", std::process::id()));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"funcao principal(): inteiro {
    variavel origem: inteiro = 16777217
    variavel convertido: decimal32 = decimal32(origem)
    variavel um: decimal32 = 1.0
    variavel esperado: decimal32 = 16777216.0
    variavel soma = convertido + um
    variavel dois: decimal64 = 2.0
    variavel soma_mista = convertido + dois
    variavel esperado_misto: decimal64 = 16777218.0
    quando soma == esperado e soma_mista == esperado_misto {
        devolver 42
    } senao {
        devolver 0
    }
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação decimal32");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável gerado não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn compila_conversao_decimal_para_inteiro_com_faixa() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio = std::env::temp_dir().join(format!(
            "nebula-decimal-inteiro-{}-{sufixo}",
            std::process::id()
        ));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"funcao principal(): inteiro64 {
    variavel decimal: decimal64 = 42.75
    devolver inteiro64(decimal)
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação de conversão decimal");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável gerado não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(&diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn converte_decimal_e_inteiro_u64_acima_de_i64_maximo() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio =
            std::env::temp_dir().join(format!("nebula-u64-alto-{}-{sufixo}", std::process::id()));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"funcao principal(): inteiro {
    variavel valor: decimal64 = 9223372036854775808.0
    variavel convertido: inteiro_u64 = inteiro_u64(valor)
    variavel esperado: inteiro_u64 = 9223372036854775808
    variavel decimal: decimal64 = decimal64(esperado)
    quando convertido == esperado e decimal == valor {
        devolver 42
    } senao {
        devolver 0
    }
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("conversão de u64 acima de i64::MAX");
        let mut tentativas = 0;
        let status = loop {
            match Command::new(&saida).status() {
                Ok(status) => break status,
                Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                    tentativas += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(erro) => panic!("executável de conversão não pôde ser iniciado: {erro}"),
            }
        };
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn rejeita_nan_faixas_e_conversoes_128_invalidas() {
        let programas = [
            r#"funcao principal(): inteiro64 {
    variavel valor: decimal64 = 1e100
    devolver inteiro64(valor)
}"#,
            r#"funcao principal(): inteiro64 {
    variavel valor: decimal64 = 0.0 / 0.0
    devolver inteiro64(valor)
}"#,
            r#"funcao principal(): inteiro_u64 {
    variavel valor: decimal64 = -1.0
    devolver inteiro_u64(valor)
}"#,
            r#"funcao principal(): inteiro_u64 {
    variavel valor: decimal64 = 18446744073709551616.0
    devolver inteiro_u64(valor)
}"#,
            r#"funcao principal(): inteiro64 {
    variavel valor: inteiro128 = 9223372036854775808
    devolver inteiro64(valor)
}"#,
            r#"funcao principal(): inteiro128 {
    variavel valor: inteiro_u128 = 170141183460469231731687303715884105728
    devolver inteiro128(valor)
}"#,
            r#"funcao principal(): inteiro_u128 {
    variavel valor: inteiro128 = -1
    devolver inteiro_u128(valor)
}"#,
        ];
        for (indice, codigo) in programas.into_iter().enumerate() {
            let sufixo = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let diretorio = std::env::temp_dir().join(format!(
                "nebula-decimal-invalido-{}-{indice}-{sufixo}",
                std::process::id()
            ));
            std::fs::create_dir_all(&diretorio).unwrap();
            let origem = diretorio.join("principal.nbx");
            let saida = diretorio.join("programa");
            std::fs::write(&origem, codigo).unwrap();

            compilar_arquivo(&origem, &saida).expect("compilação de conversão decimal inválida");
            let mut tentativas = 0;
            let status = loop {
                match Command::new(&saida).status() {
                    Ok(status) => break status,
                    Err(erro) if erro.raw_os_error() == Some(26) && tentativas < 5 => {
                        tentativas += 1;
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(erro) => panic!("executável gerado não pôde ser iniciado: {erro}"),
                }
            };
            assert_eq!(status.code(), Some(2));
            std::fs::remove_dir_all(diretorio).unwrap();
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn compila_caractere_unicode_fora_do_ascii() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio = std::env::temp_dir().join(format!(
            "nebula-caractere-unicode-{}-{sufixo}",
            std::process::id()
        ));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"funcao principal(): inteiro {
    variavel letra: caractere = '🛰'
    quando letra == '🛰' {
        devolver 42
    } senao {
        devolver 0
    }
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação de caractere Unicode");
        let status = Command::new(&saida).status().expect("executável gerado");
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn executa_decimais_em_agregados_e_payloads_da_abi() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio = std::env::temp_dir().join(format!(
            "nebula-decimais-agregados-{}-{sufixo}",
            std::process::id()
        ));
        std::fs::create_dir_all(&diretorio).unwrap();
        let origem = diretorio.join("principal.nbx");
        let saida = diretorio.join("programa");
        std::fs::write(
            &origem,
            r#"estrutura Medidas { curta: decimal32, longa: decimal64 }
enumeracao Evento { Vazio, Medida(decimal64) }
funcao identidade(valor: Medidas): Medidas {
    devolver valor
}
funcao principal(): inteiro {
    variavel entrada = Medidas { curta: 1.25, longa: 42.5 }
    variavel retorno = identidade(entrada)
    variavel evento = Evento::Medida(42.5)
    escolha evento {
        caso Evento::Medida(valor) {
            quando retorno.curta == decimal32(1.25) e retorno.longa == decimal64(42.5) e valor == decimal64(42.5) {
                devolver 42
            }
        }
        caso Evento::Vazio {}
    }
    devolver 0
}"#,
        )
        .unwrap();

        compilar_arquivo(&origem, &saida).expect("compilação de decimais em agregados");
        let status = Command::new(&saida).status().expect("executável gerado");
        assert_eq!(status.code(), Some(42));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[test]
    fn rejeita_travessia_no_caminho_de_modulo() {
        let origem = std::env::temp_dir().join("principal.nbx");
        assert!(resolver_arquivo_modulo(&origem, "..::segredo").is_err());
        assert!(resolver_arquivo_modulo(&origem, "/tmp/segredo").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejeita_modulo_por_symlink_fora_da_raiz() {
        use std::os::unix::fs::symlink;

        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diretorio = std::env::temp_dir().join(format!(
            "nebula-symlink-modulo-{}-{sufixo}",
            std::process::id()
        ));
        let raiz = diretorio.join("projeto");
        std::fs::create_dir_all(&raiz).unwrap();
        let externo = diretorio.join("externo.nbx");
        std::fs::write(&externo, "publico funcao vazia() {}").unwrap();
        symlink(&externo, raiz.join("Externo.nbx")).unwrap();

        let origem = raiz.join("principal.nbx");
        assert!(resolver_arquivo_modulo(&origem, "Externo").is_err());
        std::fs::remove_dir_all(diretorio).unwrap();
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
        std::fs::write(
            &origem,
            "incluir Matematica como Mat\nexibir(Mat::dobro(21))",
        )
        .unwrap();

        let saida = interpretar_arquivo(&origem).expect("módulo local válido");
        assert_eq!(saida, vec!["42"]);
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[rustfmt::skip]
    #[test]
    fn interpreta_modulo_com_dependencia_aninhada() {
        let diretorio = std::env::temp_dir().join(format!("nebula-modulo-aninhado-{}", std::process::id()));
        std::fs::create_dir_all(&diretorio).unwrap();
        std::fs::write(
            diretorio.join("Base.nbx"),
            "publico funcao valor(): inteiro { devolver 40 }",
        )
        .unwrap();
        std::fs::write(
            diretorio.join("Servico.nbx"),
            "incluir Base\npublico funcao obter(): inteiro { devolver Base::valor() + 2 }",
        )
        .unwrap();
        let origem = diretorio.join("principal.nbx");
        std::fs::write(&origem, "incluir Servico\nexibir(Servico::obter())").unwrap();

        let saida = interpretar_arquivo(&origem).expect("dependência aninhada válida");
        assert_eq!(saida, vec!["42"]);
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[rustfmt::skip]
    #[test]
    fn rejeita_ciclo_de_dependencias_entre_modulos() {
        let diretorio = std::env::temp_dir().join(format!("nebula-modulo-ciclo-{}", std::process::id()));
        std::fs::create_dir_all(&diretorio).unwrap();
        std::fs::write(
            diretorio.join("A.nbx"),
            "incluir B\npublico funcao a(): inteiro { devolver 1 }",
        )
        .unwrap();
        std::fs::write(
            diretorio.join("B.nbx"),
            "incluir A\npublico funcao b(): inteiro { devolver 2 }",
        )
        .unwrap();
        let origem = diretorio.join("principal.nbx");
        std::fs::write(&origem, "incluir A\nexibir(A::a())").unwrap();

        let erro = interpretar_arquivo(&origem).unwrap_err();
        assert!(erro.contains("Ciclo de dependências"));
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[rustfmt::skip]
    #[test]
    fn rejeita_caminhos_de_modulo_com_travessia_de_diretorio() {
        assert!(resolver_arquivo_modulo(Path::new("."), "../segredo").is_err());
        assert!(resolver_arquivo_modulo(Path::new("."), "modulo/subpasta").is_err());
        assert!(resolver_arquivo_modulo(Path::new("."), "modulo::::interno").is_err());
    }

    #[rustfmt::skip]
    #[test]
    fn impede_chamada_externa_a_funcao_privada_de_modulo() {
        let diretorio = std::env::temp_dir().join(format!("nebula-modulo-privado-{}", std::process::id()));
        std::fs::create_dir_all(&diretorio).unwrap();
        std::fs::write(
            diretorio.join("Interno.nbx"),
            "funcao segredo(): inteiro { devolver 42 }\npublico funcao visivel(): inteiro { devolver segredo() }",
        )
        .unwrap();
        let origem = diretorio.join("principal.nbx");
        std::fs::write(&origem, "incluir Interno\nexibir(Interno::segredo())").unwrap();

        let erro = interpretar_arquivo(&origem).unwrap_err();
        assert!(
            erro.contains("não foi encontrada em um módulo importado")
                || erro.contains("não foi declarada"),
            "{erro}"
        );
        std::fs::remove_dir_all(diretorio).unwrap();
    }

    #[cfg(unix)]
    #[rustfmt::skip]
    #[test]
    fn rejeita_symlink_de_modulo_para_fora_da_raiz() {
        use std::os::unix::fs::symlink;

        let raiz = std::env::temp_dir().join(format!("nebula-symlink-raiz-{}", std::process::id()));
        let externo = std::env::temp_dir().join(format!("nebula-symlink-externo-{}", std::process::id()));
        std::fs::create_dir_all(&raiz).unwrap();
        std::fs::create_dir_all(&externo).unwrap();
        std::fs::write(
            externo.join("Segredo.nbx"),
            "publico funcao valor(): inteiro { devolver 42 }",
        )
        .unwrap();
        symlink(externo.join("Segredo.nbx"), raiz.join("Atalho.nbx")).unwrap();
        let origem = raiz.join("principal.nbx");
        std::fs::write(&origem, "incluir Atalho\nexibir(Atalho::valor())").unwrap();

        let erro = interpretar_arquivo(&origem).unwrap_err();
        assert!(erro.contains("fora do diretório permitido"), "{erro}");
        std::fs::remove_dir_all(raiz).unwrap();
        std::fs::remove_dir_all(externo).unwrap();
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
    fn serializa_diagnosticos_em_json_com_escape_correto() {
        let caminho = Path::new(r#"arquivo"teste.nbx"#);
        let diagnosticos = [Diagnostico::novo(
            "N3007",
            Severidade::Erro,
            r#"Nome "valor""#,
            None,
        )];
        let json = diagnosticos_json(caminho, &diagnosticos, None);
        assert!(json.starts_with(r#"{"arquivo":"#));
        assert!(json.contains(r#""valido":false"#));
        assert!(json.contains(r#""codigo":"N3007""#));
        assert!(json.contains(r#""mensagem":"Nome \"valor\"""#));
    }

    #[test]
    fn serializa_lista_vazia_de_diagnosticos_como_valida() {
        let json = diagnosticos_json(Path::new("valido.nbx"), &[], None);
        assert!(json.contains(r#""valido":true"#));
        assert!(json.ends_with(r#""diagnosticos":[]}"#));
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

#[rustfmt::skip]
#[cfg(test)]
mod fixtures_conformidade_tipos {
    use super::*;

    fn validar_fixture(codigo: &str, nome: &str) -> Result<(), String> {
        let fonte = ArquivoFonte::novo(nome, codigo);
        let sintaxe = analisar(&fonte);
        if !sintaxe.diagnosticos.is_empty() {
            return Err(format!("erros sintáticos: {:?}", sintaxe.diagnosticos));
        }
        let semantica = analisar_com_modulos(&sintaxe.programa, &ModulosDisponiveis::new());
        if semantica.diagnosticos.is_empty() {
            Ok(())
        } else {
            Err(format!("erros semânticos: {:?}", semantica.diagnosticos))
        }
    }

    #[test]
    fn valida_fixture_da_matriz_completa_de_tipos_primitivos() {
        let resultado = validar_fixture(
            include_str!("../fixtures/tipos/matriz-primitivos.nbx"),
            "matriz-primitivos.nbx",
        );
        assert!(resultado.is_ok(), "{resultado:?}");
    }

    #[test]
    fn valida_fixture_de_tipos_compostos_e_algebricos() {
        let resultado = validar_fixture(
            include_str!("../fixtures/tipos/compostos.nbx"),
            "compostos.nbx",
        );
        assert!(resultado.is_ok(), "{resultado:?}");
    }

    #[test]
    fn valida_fixture_de_copia_derivada_de_agregados() {
        let resultado = validar_fixture(
            include_str!("../fixtures/tipos/copia-derivada.nbx"),
            "copia-derivada.nbx",
        );
        assert!(resultado.is_ok(), "{resultado:?}");
    }

    #[test]
    fn rejeita_fixture_de_tipos_desconhecidos_aninhados() {
        let resultado = validar_fixture(
            include_str!("../fixtures/tipos/tipos-aninhados-invalidos.nbx"),
            "tipos-aninhados-invalidos.nbx",
        );
        assert!(resultado.is_err(), "tipo desconhecido aninhado foi aceito.");
    }

    #[test]
    fn valida_fixture_de_inferencia_generica_com_tipo_funcao() {
        let resultado = validar_fixture(
            include_str!("../fixtures/tipos/genericos-funcao.nbx"),
            "genericos-funcao.nbx",
        );
        assert!(resultado.is_ok(), "{resultado:?}");
    }

    #[test]
    fn valida_fixture_de_inferencia_generica_com_tipos_aninhados() {
        let resultado = validar_fixture(
            include_str!("../fixtures/tipos/genericos-aninhados.nbx"),
            "genericos-aninhados.nbx",
        );
        assert!(resultado.is_ok(), "{resultado:?}");
    }

    #[test]
    fn valida_fixture_de_movimentacao_parcial() {
        let resultado = validar_fixture(
            include_str!("../fixtures/tipos/movimento-valido.nbx"),
            "movimento-valido.nbx",
        );
        assert!(resultado.is_ok(), "{resultado:?}");
    }

    #[test]
    fn valida_fixture_de_conversoes_numericas() {
        let resultado = validar_fixture(
            include_str!("../fixtures/tipos/conversoes-validas.nbx"),
            "conversoes-validas.nbx",
        );
        assert!(resultado.is_ok(), "{resultado:?}");
    }

    fn rejeita_abi_nativa_pendente(codigo: &str, nome: &str) -> Result<(), String> {
        let fonte = ArquivoFonte::novo(nome, codigo);
        let sintaxe = analisar(&fonte);
        if !sintaxe.diagnosticos.is_empty() {
            return Err(format!("erros sintáticos: {:?}", sintaxe.diagnosticos));
        }
        let semantica = analisar_com_modulos(&sintaxe.programa, &ModulosDisponiveis::new());
        if !semantica.diagnosticos.is_empty() {
            return Err(format!("erros semânticos: {:?}", semantica.diagnosticos));
        }
        let ir = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        if !ir.diagnosticos.is_empty() {
            return Err(format!("erros na IR: {:?}", ir.diagnosticos));
        }
        let programa = ir.programa.ok_or_else(|| "IR não foi produzida".to_owned())?;
        nebula_backend::gerar_executavel(&programa).map(|_| ()).map_err(|erro| erro.to_string())
    }

    #[test]
    fn backend_rejeita_enum_com_multiplos_payloads_pela_pipeline_completa() {
        let resultado = rejeita_abi_nativa_pendente(
            include_str!("../fixtures/tipos/abi-enum-payloads-multiplos.nbx"),
            "abi-enum-payloads-multiplos.nbx",
        );
        assert!(
            resultado.is_err(),
            "A ABI nativa não deve aceitar enumeração com dois payloads escalares enquanto a representação não estiver implementada."
        );
    }

    #[test]
    fn backend_aceita_agregados_grandes_com_retorno_oculto() {
        let estrutura = rejeita_abi_nativa_pendente(
            include_str!("../fixtures/tipos/abi-estrutura-pendente.nbx"),
            "abi-estrutura-pendente.nbx",
        );
        assert!(estrutura.is_ok(), "ABI de agregado grande com retorno oculto falhou: {estrutura:?}");

        let inteiro128 = include_str!("../fixtures/tipos/abi-inteiro128.nbx");
        let fonte = ArquivoFonte::novo("abi-inteiro128.nbx", inteiro128);
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let ir = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        assert!(ir.diagnosticos.is_empty(), "{:?}", ir.diagnosticos);
        assert!(
            nebula_backend::gerar_executavel(&ir.programa.expect("IR de inteiro128")).is_ok(),
            "ABI escalar de inteiro128 deve gerar ELF."
        );
    }

    #[test]
    fn rejeita_fixtures_invalidas_de_inferencia_movimento_e_conversao() {
        assert!(validar_fixture(
            include_str!("../fixtures/tipos/genericos-invalidos.nbx"),
            "genericos-invalidos.nbx",
        ).is_err());

        assert!(validar_fixture(
            include_str!("../fixtures/tipos/movimento-invalido.nbx"),
            "movimento-invalido.nbx",
        ).is_err());

        assert!(validar_fixture(
            include_str!("../fixtures/tipos/conversao-invalida.nbx"),
            "conversao-invalida.nbx",
        ).is_err());
    }
}
