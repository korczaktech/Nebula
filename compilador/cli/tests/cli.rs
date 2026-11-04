//! Testes de integração dos comandos da CLI Nebula.

use std::process::{Command, Output};

fn executar(argumentos: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nebula"))
        .args(argumentos)
        .output()
        .expect("a CLI deve iniciar")
}

#[test]
fn ajuda_explicita_retorna_sucesso_e_instrucoes() {
    let saida = executar(&["ajuda"]);
    assert!(saida.status.success());
    let stdout = String::from_utf8_lossy(&saida.stdout);
    assert!(stdout.contains("Uso: nebula <comando>"));
    assert!(stdout.contains("verificar <arquivo>"));
    assert!(stdout.contains("não realiza análise semântica"));
}

#[test]
fn lista_somente_alvos_com_backend_real() {
    let saida = executar(&["alvos"]);
    assert!(saida.status.success());
    let stdout = String::from_utf8_lossy(&saida.stdout);
    assert!(stdout.contains("linux-x86_64"));
    assert!(!stdout.contains("wasm32"));
}

#[test]
fn rejeita_alvo_sem_backend_antes_de_compilar() {
    let saida = executar(&["compilar", "arquivo-inexistente.nbx", "--alvo", "wasm32"]);
    assert_eq!(saida.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&saida.stderr).contains("ainda não possui backend implementado")
    );
}

#[test]
fn ausencia_de_comando_exibe_ajuda() {
    let saida = executar(&[]);
    assert!(saida.status.success());
    assert!(String::from_utf8_lossy(&saida.stdout).contains("Uso: nebula <comando>"));
}

#[test]
fn criar_projeto_gera_manifesto_e_fonte_vazios() {
    let diretorio = std::env::temp_dir().join(format!(
        "nebula-criar-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let saida = executar(&["criar", diretorio.to_str().unwrap()]);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    let manifesto = std::fs::read_to_string(diretorio.join("nebula.toml")).unwrap();
    assert!(manifesto.contains("[pacote]"));
    assert!(manifesto.contains("versao = \"0.1.0\""));
    let fonte = diretorio.join("principal.nbx");
    assert!(
        std::fs::read_to_string(&fonte)
            .unwrap()
            .contains("funcao principal")
    );
    let verificar = executar(&["analisar", fonte.to_str().unwrap()]);
    assert!(
        verificar.status.success(),
        "{}",
        String::from_utf8_lossy(&verificar.stderr)
    );
    let _ = std::fs::remove_dir_all(diretorio);
}

#[test]
fn criar_nao_sobrescreve_diretorio_existente() {
    let diretorio = std::env::temp_dir().join(format!(
        "nebula-criar-existente-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&diretorio).unwrap();
    std::fs::write(diretorio.join("preservar.txt"), "conteudo").unwrap();
    let saida = executar(&["criar", diretorio.to_str().unwrap()]);
    assert_eq!(saida.status.code(), Some(1));
    assert_eq!(
        std::fs::read_to_string(diretorio.join("preservar.txt")).unwrap(),
        "conteudo"
    );
    let _ = std::fs::remove_dir_all(diretorio);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn limpar_remove_somente_executavel_elf64_padrao() {
    let diretorio = std::env::temp_dir().join(format!(
        "nebula-limpar-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&diretorio).unwrap();
    let fonte = diretorio.join("principal.nbx");
    std::fs::write(&fonte, "funcao principal(): inteiro { devolver 42 }").unwrap();
    let compilar = executar(&["compilar", fonte.to_str().unwrap()]);
    assert!(
        compilar.status.success(),
        "{}",
        String::from_utf8_lossy(&compilar.stderr)
    );
    let artefato = fonte.with_extension("out");
    assert!(artefato.exists());
    let limpar = executar(&["limpar", fonte.to_str().unwrap()]);
    assert!(
        limpar.status.success(),
        "{}",
        String::from_utf8_lossy(&limpar.stderr)
    );
    assert!(!artefato.exists());
    let _ = std::fs::remove_dir_all(diretorio);
}

#[test]
fn limpar_preserva_arquivo_out_que_nao_seja_elf64() {
    let diretorio = std::env::temp_dir().join(format!(
        "nebula-limpar-seguro-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&diretorio).unwrap();
    let fonte = diretorio.join("principal.nbx");
    let artefato = fonte.with_extension("out");
    std::fs::write(&fonte, "funcao principal(): vazio {}").unwrap();
    std::fs::write(&artefato, b"arquivo de usuario").unwrap();
    let limpar = executar(&["limpar", fonte.to_str().unwrap()]);
    assert_eq!(limpar.status.code(), Some(1));
    assert_eq!(std::fs::read(&artefato).unwrap(), b"arquivo de usuario");
    let _ = std::fs::remove_dir_all(diretorio);
}

#[test]
fn versao_exibe_versao_do_pacote() {
    let saida = executar(&["versao"]);
    assert!(saida.status.success());
    assert!(String::from_utf8_lossy(&saida.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn formatar_normaliza_arquivo_validado() {
    let caminho = std::env::temp_dir().join(format!(
        "nebula-format-cli-{}-{}.nbx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&caminho, "variavel total = 42   ").unwrap();
    let saida = executar(&["formatar", caminho.to_str().unwrap()]);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&caminho).unwrap(),
        "variavel total = 42\n"
    );
    let _ = std::fs::remove_file(caminho);
}

#[test]
fn formatar_check_detecta_sem_alterar_arquivo() {
    let caminho = std::env::temp_dir().join(format!(
        "nebula-format-check-cli-{}-{}.nbx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let original = "variavel total = 42   ";
    std::fs::write(&caminho, original).unwrap();
    let saida = executar(&["formatar", caminho.to_str().unwrap(), "--check"]);
    assert_eq!(saida.status.code(), Some(1));
    assert_eq!(std::fs::read_to_string(&caminho).unwrap(), original);
    let _ = std::fs::remove_file(caminho);
}

#[test]
fn comando_desconhecido_retorna_codigo_dois() {
    let saida = executar(&["comando-inexistente"]);
    assert_eq!(saida.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&saida.stderr).contains("Comando desconhecido"));
}

#[test]
fn verificar_aceita_programa_sintaticamente_valido() {
    let caminho = std::env::temp_dir().join(format!("nebula-cli-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel total = 1 + 2").expect("grava fonte");
    let saida = executar(&["verificar", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(String::from_utf8_lossy(&saida.stdout).contains("válido sintaticamente"));
}

#[test]
fn verificar_rejeita_programa_com_sintaxe_invalida() {
    let caminho = std::env::temp_dir().join(format!("nebula-erro-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel =").expect("grava fonte");
    let saida = executar(&["verificar", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    assert_eq!(saida.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&saida.stderr).contains("N2010"));
}

#[test]
fn verificar_sem_caminho_retorna_codigo_dois() {
    let saida = executar(&["verificar"]);
    assert_eq!(saida.status.code(), Some(2));
}

#[test]
fn verificar_emite_diagnosticos_json_para_fonte_valida() {
    let caminho = std::env::temp_dir().join(format!("nebula-json-ok-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel total = 42").unwrap();
    let saida = executar(&["verificar", caminho.to_str().unwrap(), "--json"]);
    let _ = std::fs::remove_file(caminho);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    let json = String::from_utf8_lossy(&saida.stdout);
    assert!(json.contains(r#""valido":true"#), "{json}");
    assert!(json.contains("\"diagnosticos\":[]"), "{json}");
}

#[test]
fn verificar_emite_diagnosticos_json_para_fonte_invalida() {
    let caminho = std::env::temp_dir().join(format!("nebula-json-erro-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel =").unwrap();
    let saida = executar(&["verificar", caminho.to_str().unwrap(), "--json"]);
    let _ = std::fs::remove_file(caminho);
    assert_eq!(saida.status.code(), Some(1));
    let json = String::from_utf8_lossy(&saida.stdout);
    assert!(json.contains(r#""valido":false"#), "{json}");
    assert!(json.contains(r#""codigo":"N2010""#), "{json}");
}

#[test]
fn analisar_emite_diagnosticos_json_semanticos() {
    let caminho = std::env::temp_dir().join(format!("nebula-json-sem-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel total = ausente").unwrap();
    let saida = executar(&["analisar", caminho.to_str().unwrap(), "--json"]);
    let _ = std::fs::remove_file(caminho);
    assert_eq!(saida.status.code(), Some(1));
    let json = String::from_utf8_lossy(&saida.stdout);
    assert!(json.contains(r#""valido":false"#), "{json}");
    assert!(json.contains(r#""codigo":"N3007""#), "{json}");
}

#[test]
fn verificar_rejeita_arquivo_sem_utf8_valido() {
    let caminho = std::env::temp_dir().join(format!("nebula-bytes-{}.nbx", std::process::id()));
    std::fs::write(&caminho, vec![255_u8, 254_u8]).expect("grava bytes de teste");
    let saida = executar(&["verificar", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    assert_eq!(saida.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&saida.stderr).contains("UTF-8 válido"));
}

#[test]
fn verificar_rejeita_argumento_extra() {
    let saida = executar(&["verificar", "programa.nbx", "argumento-extra"]);
    assert_eq!(saida.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&saida.stderr).contains("somente um arquivo"));
}

#[test]
fn verificar_mostra_localizacao_do_erro() {
    let caminho = std::env::temp_dir().join(format!("nebula-local-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel =").expect("grava fonte");
    let saida = executar(&["verificar", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    let stderr = String::from_utf8_lossy(&saida.stderr);
    assert!(stderr.contains(":1:"), "{stderr}");
    assert!(stderr.contains("N2010"), "{stderr}");
}

#[test]
fn analisar_aceita_programa_com_tipos_validos() {
    let caminho = std::env::temp_dir().join(format!("nebula-semantica-{}.nbx", std::process::id()));
    std::fs::write(
        &caminho,
        "funcao soma(a: inteiro, b: inteiro): inteiro { devolver a + b }\nvariavel total = soma(2, 3)",
    )
    .expect("grava arquivo");
    let saida = executar(&["analisar", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(String::from_utf8_lossy(&saida.stdout).contains("válido semanticamente"));
}

#[test]
fn analisar_rejeita_nome_nao_declarado() {
    let caminho =
        std::env::temp_dir().join(format!("nebula-semantica-erro-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel total = ausente").expect("grava arquivo");
    let saida = executar(&["analisar", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    assert_eq!(saida.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&saida.stderr).contains("N3007"));
}

#[test]
fn interpreta_aliases_de_controle_e_operador_de_atribuicao() {
    let caminho = std::env::temp_dir().join(format!("nebula-aliases-{}.nbx", std::process::id()));
    std::fs::write(
        &caminho,
        "definir total = 0\ncada numero em [1, 2, 3] { alterar total <- total + numero }\ndurante falso { exibir(100) }\nrepetir verdadeiro { encerrar }\nalterar total <- total + 1\navaliar total { caso _ { exibir(total) } }",
    )
    .unwrap();
    let saida = executar(&["interpretar", caminho.to_str().unwrap()]);
    let _ = std::fs::remove_file(caminho);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&saida.stdout).trim(), "7");
}

#[test]
fn receber_le_entrada_padrao_e_exibe_prompt() {
    use std::io::Write;
    use std::process::Stdio;
    let caminho = std::env::temp_dir().join(format!("nebula-receber-{}.nbx", std::process::id()));
    std::fs::write(
        &caminho,
        "variavel nome = receber(\"Nome: \")\nexibir(nome)",
    )
    .unwrap();
    let mut processo = Command::new(env!("CARGO_BIN_EXE_nebula"))
        .args(["interpretar", caminho.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("a CLI deve iniciar");
    processo.stdin.take().unwrap().write_all(b"Ada\n").unwrap();
    let saida = processo.wait_with_output().unwrap();
    let _ = std::fs::remove_file(caminho);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&saida.stdout), "Nome: Ada\n");
}

#[test]
fn gerar_constroi_lista_e_percorrer_iteraveis_funciona() {
    let caminho =
        std::env::temp_dir().join(format!("nebula-gerar-cada-{}.nbx", std::process::id()));
    std::fs::write(
        &caminho,
        "variavel repetidos = gerar(7, 3)\nexibir(comprimento(repetidos))\ncada letra em \"ab\" { exibir(letra) }\ncada numero em 1..3 { exibir(numero) }",
    )
    .unwrap();
    let saida = executar(&["interpretar", caminho.to_str().unwrap()]);
    let _ = std::fs::remove_file(caminho);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&saida.stdout), "3\na\nb\n1\n2\n3\n");
}

#[test]
fn pulso_executa_ciclos_ate_encerrar() {
    let caminho = std::env::temp_dir().join(format!("nebula-pulso-{}.nbx", std::process::id()));
    std::fs::write(
        &caminho,
        "variavel batidas = 0\npulso(1) { alterar batidas <- batidas + 1\nquando batidas >= 2 { encerrar } }\nexibir(batidas)",
    )
    .unwrap();
    let saida = executar(&["interpretar", caminho.to_str().unwrap()]);
    let _ = std::fs::remove_file(caminho);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&saida.stdout).trim(), "2");
}

#[test]
fn em_paralelo_executa_funcoes_independentes_e_preserva_ordem() {
    let caminho = std::env::temp_dir().join(format!("nebula-paralelo-{}.nbx", std::process::id()));
    std::fs::write(
        &caminho,
        "funcao um(): inteiro { devolver 11 }\nfuncao dois(): inteiro { devolver 22 }\nvariavel resultados = em_paralelo(um, dois)\nexibir(resultados[0])\nexibir(resultados[1])",
    )
    .unwrap();
    let saida = executar(&["interpretar", caminho.to_str().unwrap()]);
    let _ = std::fs::remove_file(caminho);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&saida.stdout), "11\n22\n");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn cache_incremental_invalida_quando_modulo_transitivo_muda() {
    let diretorio = std::env::temp_dir().join(format!(
        "nebula-cache-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&diretorio).unwrap();
    let fonte = diretorio.join("principal.nbx");
    let modulo = diretorio.join("Base.nbx");
    std::fs::write(&modulo, "publico funcao valor(): inteiro { devolver 40 }").unwrap();
    std::fs::write(
        &fonte,
        "incluir Base\nfuncao principal(): inteiro { devolver Base::valor() + 2 }",
    )
    .unwrap();

    let compilar = || {
        let saida = executar(&["compilar", fonte.to_str().unwrap()]);
        assert!(
            saida.status.success(),
            "{}",
            String::from_utf8_lossy(&saida.stderr)
        );
        std::process::Command::new(fonte.with_extension("out"))
            .status()
            .expect("executável gerado")
            .code()
    };

    assert_eq!(compilar(), Some(42));
    assert_eq!(compilar(), Some(42));
    let cache = diretorio.join(".nebula/cache");
    let entradas_antes = std::fs::read_dir(&cache)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entrada| entrada.path().extension().is_some_and(|ext| ext == "nbc"))
        .count();
    assert_eq!(entradas_antes, 1);

    std::fs::write(&modulo, "publico funcao valor(): inteiro { devolver 39 }").unwrap();
    assert_eq!(compilar(), Some(41));
    let entradas_depois = std::fs::read_dir(&cache)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entrada| entrada.path().extension().is_some_and(|ext| ext == "nbc"))
        .count();
    assert_eq!(entradas_depois, 2);
    let _ = std::fs::remove_dir_all(diretorio);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn limpar_cache_remove_artefatos_sem_remover_fontes() {
    let diretorio = std::env::temp_dir().join(format!(
        "nebula-limpar-cache-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&diretorio).unwrap();
    let fonte = diretorio.join("principal.nbx");
    std::fs::write(&fonte, "funcao principal(): inteiro { devolver 42 }").unwrap();
    let compilar = executar(&["compilar", fonte.to_str().unwrap()]);
    assert!(
        compilar.status.success(),
        "{}",
        String::from_utf8_lossy(&compilar.stderr)
    );
    let limpar = executar(&["limpar-cache", diretorio.to_str().unwrap()]);
    assert!(
        limpar.status.success(),
        "{}",
        String::from_utf8_lossy(&limpar.stderr)
    );
    assert!(fonte.exists());
    let cache = diretorio.join(".nebula/cache");
    assert_eq!(
        std::fs::read_dir(cache)
            .unwrap()
            .filter_map(Result::ok)
            .count(),
        0
    );
    let _ = std::fs::remove_dir_all(diretorio);
}

#[test]
fn dependencias_verificar_aceita_lockfile_atual_e_rejeita_modificado() {
    let raiz = std::env::temp_dir().join(format!(
        "nebula-cli-lock-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(raiz.join("texto")).unwrap();
    std::fs::write(
        raiz.join("nebula.toml"),
        "[pacote]\nnome = \"app\"\nversao = \"1.0.0\"\n[dependencias]\ntexto = \"texto\"",
    )
    .unwrap();
    std::fs::write(
        raiz.join("texto/nebula.toml"),
        "[pacote]\nnome = \"texto\"\nversao = \"1.2.3\"",
    )
    .unwrap();
    let resolver = executar(&["dependencias", "resolver", raiz.to_str().unwrap()]);
    assert!(
        resolver.status.success(),
        "{}",
        String::from_utf8_lossy(&resolver.stderr)
    );
    let verificar = executar(&["dependencias", "verificar", raiz.to_str().unwrap()]);
    assert!(
        verificar.status.success(),
        "{}",
        String::from_utf8_lossy(&verificar.stderr)
    );
    std::fs::write(raiz.join("nebula.lock"), "nebula-lock-v1\nmodificado").unwrap();
    let alterado = executar(&["dependencias", "verificar", raiz.to_str().unwrap()]);
    assert_eq!(alterado.status.code(), Some(1));
    let _ = std::fs::remove_dir_all(raiz);
}

#[test]
fn comando_ir_exibe_representacao_intermediaria() {
    let caminho = std::env::temp_dir().join(format!("nebula-ir-{}.nbx", std::process::id()));
    std::fs::write(&caminho, "variavel total = 1 + 2").expect("grava arquivo");
    let saida = executar(&["ir", caminho.to_str().expect("caminho UTF-8")]);
    let _ = std::fs::remove_file(caminho);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(String::from_utf8_lossy(&saida.stdout).contains("ProgramaIr"));
}
