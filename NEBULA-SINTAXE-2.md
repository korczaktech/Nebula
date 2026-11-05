# Nebula Nativo — Especificação de Sistemas e Hardware

**Documento:** extensão técnica de `NEBULA-SINTAXE.md`  
**Linguagem:** Nebula (`.nbx`)  
**Camada:** Nebula Nativo  
**Compilador de referência:** Nebula Compiler, implementado em Rust  
**Objetivo:** programação de sistemas, kernels, drivers, runtimes e controle de hardware  
**Status:** proposta técnica formal para implementação; não implica que os recursos já existam no compilador.

> Este documento complementa a especificação principal. Em caso de conflito, as regras léxicas e sintáticas gerais de `NEBULA-SINTAXE.md` continuam válidas. As assinaturas aqui descritas definem a API-alvo, não uma afirmação de implementação concluída.

---

# 1. Objetivos e princípios

Nebula Nativo permite programar desde aplicações comuns até kernels e drivers, mantendo uma única linguagem e uma sintaxe progressiva.

Princípios obrigatórios:

1. **Português claro:** nomes públicos em português brasileiro, sem acentos.
2. **Aprendizado gradual:** recursos de hardware não são necessários para programas comuns.
3. **Abstração de custo zero:** abstrações devem desaparecer na compilação quando não adicionarem comportamento observável.
4. **Controle explícito:** acesso perigoso, privilegiado ou dependente de arquitetura deve ser visível no código.
5. **Segurança por padrão:** verificações de tipos, limites, alinhamento e permissões sempre que forem possíveis.
6. **Desempenho nativo:** gerar instruções adequadas à arquitetura de destino, sem camada interpretadora obrigatória.
7. **Multiarquitetura:** separar API portátil de operações específicas de x86-64, ARM64 e RISC-V.
8. **Sem promessas falsas:** uma API só será considerada implementada quando tiver código, testes e documentação compatíveis.

Nebula Nativo não deve transformar cada operação em uma chamada dinâmica de biblioteca. Operações fundamentais podem ser intrínsecos reconhecidos pelo compilador.

# 2. Níveis de acesso

A linguagem distingue quatro contextos. Eles não são palavras mágicas que concedem permissões ao programa; são declarações verificadas pelo compilador e pelo ambiente de execução.

| Contexto | Finalidade |
|---|---|
| `seguro` | Código submetido às verificações normais de segurança |
| `critico` | Região em que o programador assume responsabilidade por invariantes explicitamente documentados |
| `privilegio sistema` | Código que só pode ser compilado para um alvo e ambiente com privilégios adequados |
| `montagem arquitetura` | Instruções Assembly específicas, com contrato de entradas, saídas e efeitos |

## 2.1. Região crítica

```nbx
critico:
    variavel valor = memoria.ler<u32>(endereco)
    memoria.escrever<u32>(endereco, valor + 1)
fim
```

A região crítica não desativa automaticamente todas as verificações. Ela autoriza somente operações que a especificação classificar como inseguras, e exige que os invariantes violados sejam responsabilidade do autor.

## 2.2. Código privilegiado

```nbx
privilegio sistema:
    cpu.interrupcoes.desabilitar()
    cpu.parar()
fim
```

O compilador deve rejeitar esse código em alvos sem suporte ou em programas sem perfil de privilégio compatível. A palavra `privilegio` nunca substitui permissões do processador, do firmware ou do sistema operacional.

# 3. Tipos fundamentais de baixo nível

Os tipos de largura fixa têm tamanho e representação definidos independentemente da arquitetura.

| Tipo | Largura | Significado |
|---|---:|---|
| `u8`, `u16`, `u32`, `u64`, `u128` | 8–128 bits | Inteiros sem sinal |
| `i8`, `i16`, `i32`, `i64`, `i128` | 8–128 bits | Inteiros com sinal |
| `f32`, `f64` | 32/64 bits | Ponto flutuante |
| `bit` | 1 bit lógico | Valor binário lógico |
| `endereco` | dependente do alvo | Endereço virtual representável |
| `enderecoFisico` | dependente do alvo | Endereço físico, quando o alvo o expõe |
| `tamanho` | dependente do alvo | Tamanho de objeto ou região |
| `deslocamento` | dependente do alvo | Distância em bytes |
| `ponteiro<T>` | dependente do alvo | Ponteiro para T |
| `ponteiroBruto<T>` | dependente do alvo | Ponteiro sem garantias de validade |
| `vetor<T, N>` | tamanho fixo | N elementos contíguos |
| `bloco<T, N>` | tamanho fixo | Região contígua de N elementos |

Tipos inteiros de largura não convencional, como `u24` e `u48`, podem ser oferecidos quando o backend os representar corretamente. O compilador não deve fingir que a CPU possui registradores nativos dessas larguras.

## 3.1. Regras numéricas

- Operações inteiras devem ter comportamento definido para overflow.
- O modo padrão de inteiros de largura fixa é verificado: overflow detectável produz erro de execução ou resultado de tipo de erro, conforme a operação definida pela biblioteca.
- Operações explicitamente marcadas como wrapping podem operar módulo 2 elevado à largura do tipo.
- Conversões que perdem bits ou sinal exigem conversão explícita.
- Deslocamentos com quantidade negativa ou maior/igual à largura do operando são inválidos.
- Divisão por zero é sempre erro.
- O compilador não pode assumir que overflow assinado é impossível.

Exemplos:

```nbx
variavel pequeno: u8 = 255
variavel proximo = pequeno.somarVerificado(1)
variavel circular = pequeno.somarCircular(1)
```

Os nomes acima representam operações propostas; a API final deve escolher uma convenção única para métodos verificados e circulares.

# 4. Alinhamento, tamanho e layout

O layout deve ser previsível quando declarado explicitamente.

Atributos propostos:

| Atributo | Efeito |
|---|---|
| `@alinhamento(N)` | Exige alinhamento N |
| `@representacao(C)` | Usa layout compatível com C, sujeito ao ABI |
| `@empacotado` | Remove padding entre campos quando permitido |
| `@tamanho(N)` | Exige tamanho total N |
| `@secao("nome")` | Solicita seção específica no objeto |
| `@volatil` | Marca acesso volátil a um objeto ou campo compatível |
| `@semCopia` | Impede cópia implícita do tipo |
| `@semDescarte` | Impede descarte automático; exige consumo explícito |

```nbx
@alinhamento(8)
@representacao(C)
estrutura Cabecalho:
    codigo: u32
    tamanho: u16
    estado: u8
fim
```

O compilador deve disponibilizar intrínsecos de consulta:

- `tipo.tamanhoDe<T>()`
- `tipo.alinhamentoDe<T>()`
- `tipo.deslocamentoDe<T>("campo")`

Essas consultas devem ser resolvidas em compilação quando os tipos forem conhecidos.

# 5. Memória

Módulo: `memoria`.

## 5.1. API proposta

| Operação | Assinatura conceitual | Descrição |
|---|---|---|
| Reservar | `memoria.reservar(tamanho, alinhamento)` | Obtém uma região de memória |
| Alocar | `memoria.alocar<T>(quantidade)` | Aloca espaço para elementos |
| Realocar | `memoria.realocar(regiao, novoTamanho)` | Redimensiona uma região |
| Liberar | `memoria.liberar(regiao)` | Libera uma alocação compatível |
| Ler | `memoria.ler<T>(regiao, deslocamento)` | Lê um valor |
| Escrever | `memoria.escrever<T>(regiao, deslocamento, valor)` | Escreve um valor |
| Copiar | `memoria.copiar(destino, origem, tamanho)` | Copia regiões sem sobreposição |
| Mover | `memoria.mover(destino, origem, tamanho)` | Move regiões com sobreposição |
| Preencher | `memoria.preencher(regiao, byte, tamanho)` | Preenche bytes |
| Comparar | `memoria.comparar(a, b, tamanho)` | Compara bytes |
| Mapear | `memoria.mapear(configuracao)` | Cria mapeamento virtual/físico autorizado |
| Desmapear | `memoria.desmapear(mapa)` | Remove mapeamento |
| Proteger | `memoria.proteger(mapa, permissao)` | Altera permissões |
| Sincronizar | `memoria.sincronizar(regiao)` | Aplica sincronização exigida pelo alvo |

A API real deve usar tipos de região que carreguem tamanho, alinhamento, permissões e origem da alocação. Evitar funções que recebam apenas inteiros crus quando for possível usar um tipo seguro.

## 5.2. Exemplo

```nbx
constante TAMANHO_PAGINA: tamanho = 4096

variavel bloco = memoria.reservar(
    tamanho: TAMANHO_PAGINA,
    alinhamento: 8
)?

memoria.preencher(bloco, 0, TAMANHO_PAGINA)
memoria.escrever<u32>(bloco, 0, 12345)

variavel valor = memoria.ler<u32>(bloco, 0)?

memoria.liberar(bloco)
```

O operador `?` só poderá propagar erros se a semântica de resultado/erro estiver definida pela especificação principal. Até lá, o exemplo é ilustrativo.

## 5.3. Memória física e virtual

- Endereços físicos e virtuais são tipos distintos.
- Converter um endereço virtual em físico exige suporte do kernel e contexto válido.
- Mapear memória física requer privilégio e validação de faixa.
- Um endereço numérico não se torna automaticamente um ponteiro válido.
- Acesso a memória de dispositivo deve ser distinguido de RAM comum.
- O compilador deve impedir aritmética de ponteiro que ultrapasse a região conhecida em contextos seguros.

# 6. Ponteiros e referências

Tipos:

- `referencia<T>`: referência válida, com regras de vida útil.
- `ponteiro<T>`: ponteiro tipado cuja validade depende do contrato da API.
- `ponteiroBruto<T>`: ponteiro cru; dereferência exige contexto crítico.
- `ponteiroVolatil<T>`: ponteiro para objeto com acesso volátil.

Operações propostas:

| Operação | Descrição |
|---|---|
| `ponteiro.de(&valor)` | Cria referência/ponteiro a partir de valor elegível |
| `ponteiro.nulo<T>()` | Produz ponteiro nulo tipado |
| `ponteiro.endereco(p)` | Consulta endereço |
| `ponteiro.avancar(p, n)` | Avança N elementos dentro da região |
| `ponteiro.recuar(p, n)` | Retrocede N elementos |
| `ponteiro.ler(p)` | Lê valor apontado |
| `ponteiro.escrever(p, valor)` | Escreve valor apontado |
| `ponteiro.validar(p)` | Valida propriedades verificáveis |

```nbx
variavel numero: u32 = 50
variavel referencia = ponteiro.de(&numero)

*referencia = 100
exibir(numero)
```

A sintaxe de dereferência acima é proposta e precisa ser confirmada na gramática. Não se deve introduzi-la no parser sem definir precedência e regras de segurança.

# 7. Operações de bits

Módulo: `bits`.

## 7.1. Operadores

| Operador | Operação |
|---|---|
| `&` | E bit a bit |
| `|` | OU bit a bit |
| `^` | OU exclusivo |
| `~` | Inversão de bits |
| `<<` | Deslocamento à esquerda |
| `>>` | Deslocamento à direita, conforme tipo |
| `>>> ` | Deslocamento lógico à direita, se adotado pela gramática |

## 7.2. Funções

| Função | Descrição |
|---|---|
| `bits.ler(valor, indice)` | Lê um bit |
| `bits.definir(valor, indice)` | Define bit como 1 |
| `bits.zerar(valor, indice)` | Define bit como 0 |
| `bits.inverter(valor, indice)` | Inverte bit |
| `bits.extrair(valor, inicio, largura)` | Extrai campo |
| `bits.inserir(valor, campo, inicio, largura)` | Insere campo |
| `bits.contar(valor)` | Conta bits ativos |
| `bits.primeiro(valor)` | Localiza primeiro bit ativo |
| `bits.mascarar(valor, mascara)` | Aplica máscara |
| `bits.rotacionarEsquerda(valor, n)` | Rotaciona à esquerda |
| `bits.rotacionarDireita(valor, n)` | Rotaciona à direita |

```nbx
variavel estado: u8 = 0b00000000

estado = bits.definir(estado, 3)
estado = bits.definir(estado, 5)
estado = bits.inverter(estado, 3)

se bits.ler(estado, 5)
    exibir("Bit 5 ativo")
fim
```

As funções que retornam um valor devem deixar claro se modificam a variável original ou produzem uma nova. A forma preferida é retornar o valor transformado e exigir atribuição explícita.

# 8. CPU e registradores

Módulo: `cpu`.

## 8.1. Informações portáteis

| Operação | Descrição |
|---|---|
| `cpu.arquitetura()` | Identifica arquitetura de destino/execução |
| `cpu.fabricante()` | Consulta fabricante, quando exposto |
| `cpu.identificar()` | Consulta identificação |
| `cpu.nucleos()` | Consulta núcleos lógicos disponíveis |
| `cpu.capacidades()` | Consulta recursos disponíveis |
| `cpu.suporta(recurso)` | Verifica recurso |
| `cpu.cache.consultar()` | Consulta informações de cache |
| `cpu.simd.disponivel()` | Consulta recursos SIMD |

Informações como frequência, temperatura e fabricante podem não estar disponíveis em todos os ambientes. A API deve retornar ausência/erro tipado, nunca inventar valores.

## 8.2. Registradores

Módulo: `cpu.registro`.

Operações propostas:

- `cpu.registro.ler(registro)`
- `cpu.registro.escrever(registro, valor)`
- `cpu.registro.consultar(registro)`
- `cpu.registro.salvar(conjunto)`
- `cpu.registro.restaurar(estado)`

O nome de registrador deve ser um símbolo conhecido pelo backend, e não uma string arbitrária.

Exemplo ilustrativo para x86-64:

```nbx
se cpu.arquitetura() == "x86_64"
    variavel valor = cpu.registro.ler(cpu.registro.rax)
fim
```

O compilador deve rejeitar registradores que não existam no alvo. Registradores de controle, estado e privilégio exigem contexto privilegiado e contrato de segurança específico.

## 8.3. Registradores por arquitetura

- **x86-64:** registradores gerais, ponteiro de pilha, registradores de flags, SIMD e registradores de controle, conforme modo e privilégio.
- **ARM64:** registradores gerais X/W, SP, PC, PSTATE, SIMD/FP e registradores de sistema permitidos.
- **RISC-V:** registradores inteiros, ponto flutuante, PC e registradores de controle/estado conforme extensão e privilégio.

O conjunto exposto deve vir de metadados de arquitetura versionados no compilador.

# 9. Instruções e recursos da CPU

Operações intrínsecas propostas:

| Intrínseco | Semântica |
|---|---|
| `cpu.nop()` | Instrução sem efeito arquitetural observável além do avanço |
| `cpu.pausar()` | Dica de espera ativa adequada à arquitetura |
| `cpu.barreira()` | Barreira com ordenação explicitamente definida |
| `cpu.esperarEvento()` | Espera por evento, quando suportada |
| `cpu.parar()` | Suspende execução conforme privilégio e arquitetura |
| `cpu.tique()` | Lê contador de ciclos/ticks disponível |
| `cpu.simd...` | Operações vetoriais |

`cpu.pausar()` não é sinônimo de suspensão da CPU. `cpu.parar()` não deve ser chamado em aplicação comum sem suporte e privilégio.

## 9.1. SIMD

A biblioteca deve oferecer tipos vetoriais independentes de arquitetura, por exemplo `vetorSimd<u32, 4>`, e permitir que o backend selecione instruções disponíveis.

```nbx
se cpu.simd.disponivel("vetorial_128")
    variavel resultado = simd.somar(vetorA, vetorB)
senao
    variavel resultado = vetorA.somarElemento(vetorB)
fim
```

O nome de recurso acima é conceitual. A especificação final deve definir nomes canônicos para conjuntos de instruções, largura e alinhamento.

# 10. Barreira e ordenação de memória

Módulo: `memoria.ordem`.

A linguagem deve distinguir:

- barreira de compilador;
- barreira de leitura;
- barreira de escrita;
- barreira completa;
- ordenação de operações atômicas;
- sincronização de cache para dispositivos não coerentes.

Uma barreira de compilador impede determinadas reorganizações pelo otimizador, mas não necessariamente emite instrução de CPU. Uma barreira de CPU impõe a ordenação arquitetural definida. A documentação deve explicar essa diferença.

Nenhuma API genérica pode prometer a mesma instrução em todas as arquiteturas.

# 11. Acesso a dispositivos e MMIO

Módulo: `hardware`.

## 11.1. Descoberta e ciclo de vida

| Operação | Descrição |
|---|---|
| `hardware.detectar()` | Descobre dispositivos enumeráveis |
| `hardware.consultar(id)` | Consulta propriedades |
| `hardware.conectar(id)` | Adquire dispositivo |
| `hardware.desconectar(dispositivo)` | Libera dispositivo |
| `hardware.iniciar(dispositivo)` | Inicializa |
| `hardware.parar(dispositivo)` | Para |
| `hardware.estado(dispositivo)` | Consulta estado |
| `hardware.capacidades(dispositivo)` | Consulta recursos |

## 11.2. Regiões mapeadas em memória

Uma região MMIO deve ser representada por tipo próprio, com tamanho, permissões, largura de acesso e atributo volátil.

```nbx
variavel controlador = hardware.mapear(
    endereco: 0xF0000000,
    tamanho: 4096,
    acesso: leitura | escrita,
    largura: [8, 16, 32]
)?

hardware.escreverVolatil<u32>(controlador, 0, 1)
variavel estado = hardware.lerVolatil<u32>(controlador, 4)?
```

Regras obrigatórias:

1. Acesso MMIO não pode ser tratado como RAM comum.
2. Leituras e escritas voláteis não podem ser removidas como código morto.
3. O compilador deve preservar a ordem exigida pelo contrato do dispositivo.
4. Largura, alinhamento e permissões devem ser verificados.
5. O mapeamento precisa respeitar atributos de cache e coerência.
6. O endereço físico só pode ser mapeado em ambiente autorizado.
7. A API deve impedir que uma região seja usada depois de desmapeada.

# 12. Portas de entrada e saída

Módulo: `porta`.

Disponível somente em arquiteturas/ambientes que implementem espaço de I/O por portas, como determinados modos de x86.

| Operação | Largura |
|---|---:|
| `porta.ler8(endereco)` | 8 bits |
| `porta.ler16(endereco)` | 16 bits |
| `porta.ler32(endereco)` | 32 bits |
| `porta.escrever8(endereco, valor)` | 8 bits |
| `porta.escrever16(endereco, valor)` | 16 bits |
| `porta.escrever32(endereco, valor)` | 32 bits |

```nbx
privilegio sistema:
    variavel codigo = porta.ler8(0x60)
    porta.escrever8(0x20, 0x01)
fim
```

O exemplo é ilustrativo e não configura um controlador real por si só. Em ARM64 e em muitos dispositivos RISC-V, o acesso a periféricos é feito por MMIO ou interfaces específicas, não por essas portas.

# 13. Interrupções e exceções

Módulo: `interrupcao`.

Operações propostas:

- `interrupcao.registrar(vetor, manipulador)`
- `interrupcao.remover(vetor)`
- `interrupcao.habilitar()`
- `interrupcao.desabilitar()`
- `interrupcao.aguardar()`
- `interrupcao.identificar(contexto)`
- `interrupcao.confirmar(controlador, vetor)`
- `interrupcao.prioridade(vetor, valor)`

Exemplo:

```nbx
funcao tratarTeclado(evento: interrupcao.Evento):
    variavel tecla = hardware.teclado.ler()
    teclado.processar(tecla)
fim

interrupcao.registrar(33, tratarTeclado)
```

O tipo de manipulador deve impor uma convenção de chamada compatível com a arquitetura, regras de pilha e retorno apropriado. O compilador deve impedir operações bloqueantes ou alocações não permitidas em contextos de interrupção, conforme o perfil configurado.

Interrupções, exceções síncronas, traps e chamadas de sistema são conceitos relacionados, mas não intercambiáveis. A API deve distingui-los.

# 14. DMA

Módulo: `dma`.

| Operação | Descrição |
|---|---|
| `dma.criar(configuracao)` | Prepara transferência |
| `dma.iniciar(transferencia)` | Inicia |
| `dma.parar(transferencia)` | Interrompe |
| `dma.estado(transferencia)` | Consulta estado |
| `dma.aguardar(transferencia)` | Aguarda conclusão |
| `dma.sincronizar(transferencia)` | Sincroniza CPU/dispositivo |
| `dma.endereco(transferencia)` | Consulta endereço acessível ao dispositivo |
| `dma.liberar(transferencia)` | Libera recursos |

Direções canônicas:

- `memoria_para_dispositivo`
- `dispositivo_para_memoria`
- `bidirecional`

Exemplo:

```nbx
variavel transferencia = dma.criar(
    origem: buffer,
    tamanho: 4096,
    direcao: memoria_para_dispositivo
)?

dma.sincronizar(transferencia)?
dma.iniciar(transferencia)?
dma.aguardar(transferencia)?
dma.liberar(transferencia)
```

O runtime/kernel deve validar endereço DMA, limites, alinhamento, mapeamento IOMMU quando disponível, coerência de cache e vida útil do buffer. DMA não pode receber um ponteiro arbitrário sem validação.

# 15. Operações atômicas e sincronização

Módulo: `atomico`.

Operações:

- `atomico.ler(valor, ordem)`
- `atomico.escrever(valor, novo, ordem)`
- `atomico.trocar(valor, novo, ordem)`
- `atomico.somar(valor, incremento, ordem)`
- `atomico.subtrair(valor, decremento, ordem)`
- `atomico.compararTrocar(valor, esperado, novo, sucesso, falha)`
- `atomico.adquirir()`
- `atomico.liberar()`

Ordenações:

| Nome | Significado |
|---|---|
| `relaxado` | Atomicidade sem ordenação adicional |
| `adquirir` | Ordenação de aquisição |
| `liberar` | Ordenação de liberação |
| `adquirirLiberar` | Aquisição e liberação |
| `sequencial` | Ordem sequencial consistente |

```nbx
variavel contador: atomico<u32> = 0
atomico.somar(contador, 1, ordem: relaxado)
```

A operação deve ser atômica para o tipo e alvo em questão. Se não houver instrução nativa, o compilador pode usar mecanismo equivalente permitido ou rejeitar o alvo quando o contrato exigir lock-free.

# 16. Cache e coerência

Módulo: `cache`.

Operações propostas:

- `cache.linha()`
- `cache.precarregar(endereco)`
- `cache.limpar(regiao)`
- `cache.invalidar(regiao)`
- `cache.escreverVolta(regiao)`
- `cache.sincronizar(regiao)`

Essas operações variam significativamente por arquitetura. Algumas são privilegiadas, outras exigem alinhamento por linha, e outras não estão disponíveis em modo usuário. O backend deve validar suporte e emitir instruções corretas ou retornar recurso não suportado.

Não confundir:
- limpar cache com zerar memória;
- invalidar cache com liberar memória;
- write-back com barreira de memória;
- coerência de cache de CPU com coerência de DMA.

# 17. Temporizadores e contadores

Módulo: `tempo`.

Operações:

- `tempo.ciclos()`
- `tempo.tique()`
- `tempo.nanosegundos()`
- `tempo.microssegundos()`
- `tempo.milissegundos()`
- `tempo.aguardar(duracao)`
- `tempo.cronometro()`
- `tempo.frequencia()`

O valor de ciclos não equivale necessariamente a tempo de parede nem a ciclos constantes por segundo. A API deve informar origem, unidade, resolução e limitações do contador.

# 18. Energia e estados de execução

Módulo: `energia`.

Operações:

- `energia.estado()`
- `energia.dormir()`
- `energia.acordar()`
- `energia.desligar()`
- `energia.reiniciar()`

Essas operações dependem de firmware, kernel, plataforma e privilégio. A biblioteca deve retornar resultado explícito e não prometer que o hardware executará a solicitação.

# 19. Controladores e subsistemas de hardware

A biblioteca padrão deve oferecer interfaces de alto nível sobre drivers, sem remover a possibilidade de acesso nativo quando necessário.

Módulos previstos:

| Módulo | Responsabilidade |
|---|---|
| `hardware.processador` | Informações e capacidades da CPU |
| `hardware.memoria` | Regiões e características de memória |
| `hardware.grafico` | GPU e memória de vídeo |
| `hardware.armazenamento` | Discos, controladores e filas |
| `hardware.teclado` | Entrada de teclado |
| `hardware.tela` | Saída de vídeo e modos |
| `hardware.comunicacao` | Barramentos e interfaces |
| `hardware.rede` | Interfaces de rede |
| `hardware.audio` | Dispositivos de áudio |

Esses módulos representam contratos de API. Drivers concretos, enumeradores de barramento e protocolos devem ser implementados separadamente. A linguagem não deve supor que todo hardware possui o mesmo conjunto de recursos.

# 20. Assembly embutido

Assembly é necessário para instruções específicas, transições de privilégio, rotinas de inicialização e integração com ABI.

A forma preferida é estruturada, com entradas, saídas e efeitos declarados:

```nbx
variavel resultado = montagem x86_64<u64>(
    entradas: [10, 20],
    saidas: [registrador.rax],
    efeitos: [registradores, flags],
    instrucoes: [
        "mov rax, entrada0",
        "add rax, entrada1"
    ]
)
```

Regras:

1. Arquitetura deve ser explícita.
2. Entradas e saídas devem ter tipos e restrições.
3. Registradores utilizados devem ser declarados ou inferidos pelo backend.
4. Efeitos colaterais devem ser declarados.
5. O compilador deve validar sintaxe Assembly e operandos.
6. Assembly não pode esconder escrita em memória, alteração de flags ou acesso a registradores especiais.
7. A forma textual é uma interface avançada; macros ou intrínsecos tipados são preferíveis quando disponíveis.

Não aceitar uma string arbitrária como instrução segura. A implementação deve integrar o parser/backend de Assembly do compilador ou uma representação intermediária validada.

# 21. ABI, ligação e símbolos externos

Nebula Nativo deve suportar interoperabilidade controlada com código externo.

Recursos propostos:

- `@representacao(C)` para tipos compatíveis com C.
- `@convencao(C)` para convenção de chamada C quando suportada.
- `@exportar` para símbolos públicos.
- `externa funcao` para declarar símbolo externo.
- `@simbolo("nome")` para nome externo explícito.
- `@secao("nome")` para posicionamento de símbolo.

Exemplo conceitual:

```nbx
@convencao(C)
externa funcao escreverByte(porta: u16, valor: u8) -> vazio
```

A convenção de chamada, layout, alinhamento, tratamento de erro e propriedade de memória devem ser documentados para cada fronteira ABI. Não assumir que ABI C é idêntica entre arquiteturas.

# 22. Seções do executável e inicialização

A linguagem deve permitir que o compilador e o linker posicionem símbolos em seções específicas.

Seções convencionais possíveis:

- `.inicio`
- `.codigo`
- `.dados`
- `.bss`
- `.constantes`
- `.interrupcoes`
- `.nucleo`
- `.drivers`
- `.pilha`

Exemplo:

```nbx
@secao(".nucleo")
funcao iniciarKernel() -> vazio:
    exibir("Kernel iniciado")
fim
```

O nome de seção é uma solicitação ao linker; o formato final do executável e o script de ligação determinam o posicionamento efetivo. O compilador deve emitir erro se a seção não puder ser representada pelo alvo.

# 23. Exemplo integrado: região MMIO

O exemplo abaixo ilustra o estilo desejado. Os nomes de tipos e funções precisam ser implementados conforme o contrato de hardware específico.

```nbx
constante ENDERECO_CONTROLADOR: enderecoFisico = 0xF0000000
constante TAMANHO_CONTROLADOR: tamanho = 4096

funcao iniciarControlador() -> resultado<hardware.Regiao, hardware.Erro>:
    variavel regiao = hardware.mapear(
        endereco: ENDERECO_CONTROLADOR,
        tamanho: TAMANHO_CONTROLADOR,
        acesso: leitura | escrita,
        atributo: naoCacheavel
    )?

    hardware.escreverVolatil<u32>(regiao, 0, 1)?
    retorne sucesso(regiao)
fim

funcao consultarEstado(regiao: hardware.Regiao) -> resultado<u32, hardware.Erro>:
    retorne hardware.lerVolatil<u32>(regiao, 4)
fim
```

# 24. Exemplo integrado: interrupção

```nbx
funcao tratarTeclado(evento: interrupcao.Evento) -> vazio:
    variavel tecla = hardware.teclado.lerSemBloqueio()
    teclado.enfileirar(tecla)
    interrupcao.confirmar(evento)
fim

funcao iniciarTeclado() -> resultado<vazio, sistema.Erro>:
    interrupcao.registrar(33, tratarTeclado)?
    retorne sucesso()
fim
```

O manipulador deve executar apenas operações permitidas no contexto de interrupção. Processamento pesado deve ser delegado a uma thread ou fila apropriada.

# 25. Exemplo integrado: DMA

```nbx
funcao enviar(buffer: memoria.Regiao<u8>) -> resultado<vazio, dma.Erro>:
    variavel envio = dma.criar(
        buffer: buffer,
        direcao: memoria_para_dispositivo
    )?

    dma.sincronizar(envio)?
    dma.iniciar(envio)?
    dma.aguardar(envio)?
    dma.liberar(envio)

    retorne sucesso()
fim
```

A região deve permanecer válida e inacessível para mutações incompatíveis durante a transferência, de acordo com o contrato do dispositivo.

# 26. Otimização e geração de código

Nebula Nativo deve permitir desempenho de código nativo sem prometer superioridade não medida sobre C, C++ ou Rust.

O compilador deve considerar:

1. Monomorfização de genéricos.
2. Inlining guiado por custo e perfil.
3. Eliminação de código morto.
4. Propagação de constantes.
5. Simplificação algébrica respeitando overflow e efeitos.
6. Eliminação de cópias quando semanticamente válida.
7. Alocação eficiente de registradores.
8. Otimização de loops.
9. Vetorização automática quando segura.
10. Especialização por arquitetura e recursos.
11. Otimização de alinhamento.
12. Eliminação de abstrações sem custo observável.

## 26.1. Restrições do otimizador

O otimizador não pode:

- remover leituras/escritas voláteis;
- reordenar operações atômicas contra sua ordem declarada;
- eliminar barreiras;
- assumir que chamadas externas não têm efeitos sem contrato;
- transformar acesso MMIO em acesso comum;
- presumir que ponteiros crus são válidos;
- assumir que overflow nunca acontece quando a semântica permite overflow;
- remover operações de sincronização necessárias a DMA ou dispositivos.

# 27. Portabilidade e capacidades

A compilação deve distinguir:

- recurso garantido pela linguagem;
- recurso suportado pelo backend;
- recurso suportado pela CPU;
- recurso permitido pelo nível de privilégio;
- recurso fornecido pelo kernel/firmware;
- recurso presente no dispositivo.

Uma verificação como `cpu.suporta("avx2")` não garante, por si só, que o sistema operacional habilitou o estado necessário para usar a extensão. A API de capacidades deve considerar o ambiente de execução.

Código portátil deve usar abstrações comuns. Código específico deve declarar alvo e condição de compilação.

# 28. Perfis de compilação

Perfis propostos:

| Perfil | Uso |
|---|---|
| `aplicacao` | Programas comuns em ambiente hospedado |
| `sistema` | Serviços e componentes de sistema |
| `kernel` | Kernel e drivers privilegiados |
| `baremetal` | Execução sem sistema operacional |
| `biblioteca` | Bibliotecas e ABI externa |

O perfil seleciona regras de ligação, runtime disponível, permissões e bibliotecas permitidas. Ele não deve alterar silenciosamente a semântica dos tipos.

# 29. Diagnósticos obrigatórios

O compilador deve produzir diagnósticos claros para:

- registrador inexistente na arquitetura;
- instrução não suportada;
- instrução privilegiada em perfil sem privilégio;
- ponteiro desalinhado;
- leitura/escrita fora da região;
- acesso a região liberada ou desmapeada;
- largura de MMIO inválida;
- uso de região não volátil como dispositivo;
- operação atômica não suportada;
- barreira incompatível;
- DMA com buffer inválido;
- manipulador com convenção de chamada incorreta;
- símbolo ABI incompatível;
- seção não suportada pelo linker;
- recurso de CPU indisponível;
- uso de API não disponível no perfil.

Cada diagnóstico deve indicar causa, localização, requisito e uma possível correção, sem ocultar o risco real.

# 30. Gramática adicional preliminar

A gramática abaixo complementa a EBNF da especificação principal. É um esboço de construções novas; tipos, expressões e blocos devem reutilizar as regras centrais do parser.

```ebnf
bloco_critico       = "critico", ":", bloco, "fim" ;
bloco_privilegiado  = "privilegio", identificador,
                      ":", bloco, "fim" ;

tipo_primitivo      = "u8" | "u16" | "u32" | "u64" | "u128"
                    | "i8" | "i16" | "i32" | "i64" | "i128"
                    | "f32" | "f64" | "bit"
                    | "endereco" | "enderecoFisico"
                    | "tamanho" | "deslocamento" ;

tipo_ponteiro       = "ponteiro", "<", tipo, ">"
                    | "ponteiroBruto", "<", tipo, ">"
                    | "ponteiroVolatil", "<", tipo, ">" ;

tipo_vetor          = "vetor", "<", tipo, ",", inteiro, ">" ;

atributo            = "@", identificador,
                      [ "(", argumentos, ")" ] ;

declaracao_atributo = { atributo }, declaracao ;

montagem            = "montagem", identificador,
                      [ "<", tipo, ">" ],
                      "(", argumentos_montagem, ")"
                    | "fim" ;
```

A sintaxe de Assembly, atributos e blocos privilegiados precisa ser integrada à gramática principal sem criar ambiguidades com os delimitadores existentes.

# 31. Biblioteca padrão: catálogo inicial

| Módulo | Operações prioritárias |
|---|---|
| `memoria` | reservar, alocar, liberar, ler, escrever, copiar, mover, preencher, mapear, proteger |
| `ponteiro` | endereco, avancar, recuar, ler, escrever, validar |
| `bits` | ler, definir, zerar, inverter, extrair, inserir, contar, rotacionar |
| `cpu` | arquitetura, identificar, nucleos, capacidades, suporta, pausar, nop, barreira |
| `cpu.registro` | ler, escrever, consultar, salvar, restaurar |
| `simd` | somar, subtrair, multiplicar, comparar, carregar, armazenar |
| `hardware` | detectar, consultar, conectar, desconectar, mapear |
| `porta` | ler8/16/32, escrever8/16/32 |
| `interrupcao` | registrar, remover, habilitar, desabilitar, aguardar, confirmar |
| `dma` | criar, iniciar, parar, estado, aguardar, sincronizar, liberar |
| `atomico` | ler, escrever, trocar, somar, subtrair, compararTrocar |
| `cache` | linha, precarregar, limpar, invalidar, escreverVolta, sincronizar |
| `tempo` | ciclos, tique, nanossegundos, microssegundos, milissegundos, aguardar |
| `energia` | estado, dormir, acordar, desligar, reiniciar |
| `nucleo` | recursos de inicialização e execução específicos do kernel |

A biblioteca deve evitar duplicar funções entre módulos. Por exemplo, leitura comum de RAM pertence a `memoria`; leitura volátil de registrador MMIO pertence a `hardware`.

# 32. Estratégia de implementação

A implementação deve ocorrer em etapas verificáveis, sem declarar recursos concluídos apenas por existirem na documentação.

## Etapa 1 — Base de tipos e layout
- Tipos inteiros de largura fixa.
- Tipos de endereço, tamanho e deslocamento.
- Consultas de tamanho e alinhamento.
- Representação de estruturas.
- Testes de layout por arquitetura.

## Etapa 2 — Operações fundamentais
- Operadores de bits.
- Intrínsecos de ponteiros.
- Memória e cópia de blocos.
- Verificações de limites e alinhamento.
- Semântica de overflow e conversões.

## Etapa 3 — Backend e CPU
- Identificação de arquitetura.
- Registradores tipados.
- Intrínsecos básicos.
- Barreiras de compilador e CPU.
- Recursos SIMD.
- Diagnósticos por alvo.

## Etapa 4 — Hardware e kernel
- MMIO e acessos voláteis.
- Portas de I/O aplicáveis.
- Interrupções e convenções de chamada.
- ABI e símbolos externos.
- Seções e integração com linker.

## Etapa 5 — Recursos avançados
- DMA e sincronização.
- Operações atômicas.
- Cache e coerência.
- Temporizadores e energia.
- Assembly embutido validado.
- Perfis kernel e bare-metal.

Cada etapa exige testes unitários, testes de compilação, testes de execução quando seguros, testes de backend e documentação. Testes de hardware real devem ser separados dos testes simulados.

# 33. Critérios de conclusão

Nebula Nativo só poderá ser considerado funcional quando:

1. Os tipos de largura fixa tiverem semântica definida e testes.
2. O compilador rejeitar combinações de tipos e arquiteturas inválidas.
3. Operações de memória tiverem contratos de propriedade, alinhamento e limites.
4. Intrínsecos gerarem código correto nos alvos declarados.
5. Acessos voláteis preservarem os efeitos e a ordem exigida.
6. APIs privilegiadas forem bloqueadas em perfis incompatíveis.
7. Interrupções respeitarem ABI e convenções de chamada.
8. Operações atômicas respeitarem a ordenação declarada.
9. DMA respeitar vida útil, mapeamento e sincronização.
10. Assembly embutido declarar entradas, saídas e efeitos.
11. Houver testes para x86-64, ARM64 e RISC-V na medida em que cada backend for implementado.
12. A documentação diferenciar claramente recursos implementados, experimentais e planejados.

---

# 34. Relação com a especificação principal

Este arquivo não substitui `NEBULA-SINTAXE.md`. Ele detalha a camada de sistemas e hardware da linguagem.

A especificação principal define a identidade, sintaxe geral, palavras reservadas, estruturas de controle, funções, classes e biblioteca de alto nível. Este documento define os tipos, contratos e APIs necessários para programação nativa.

Quando uma construção deste documento exigir uma palavra reservada nova ou uma alteração na gramática central, a mudança deve ser incorporada também à especificação principal antes de ser considerada parte oficial da linguagem.

**Nebula Nativo — especificação técnica complementar.**
