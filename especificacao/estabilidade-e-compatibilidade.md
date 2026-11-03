# Estabilidade e compatibilidade

## Estado atual

Nebula permanece em desenvolvimento e a versão 1.0 ainda não foi publicada. A versão Cargo atual não constitui, por si só, promessa de estabilidade da linguagem, ABI, runtime, biblioteca padrão ou pacotes.

## Contratos separados

A política de estabilidade deverá versionar separadamente:

- **Linguagem:** tokens, gramática, semântica, tipos e regras de avaliação.
- **CLI:** nomes de comandos, argumentos, códigos de saída e formatos de diagnóstico.
- **ABI e backend:** convenções de chamada, layout, símbolos e formatos de artefato.
- **Runtime e biblioteca padrão:** assinaturas públicas, erros e garantias.
- **Pacotes:** manifesto, lockfile, resolução e integridade.

## Política planejada para 1.0

Após a publicação de 1.0, mudanças incompatíveis em contratos estáveis exigirão nova versão majoritária, salvo correção de vulnerabilidade com procedimento de segurança documentado. Depreciações deverão ser anunciadas no changelog, permanecer disponíveis por pelo menos uma versão minoritária quando tecnicamente viável e indicar substituição.

Antes de 1.0, mudanças incompatíveis são permitidas, mas devem atualizar especificação, exemplos, testes e changelog. Formatos internos ainda não documentados não são contratos públicos.

## Compatibilidade atual

- Alvo nativo executável validado: Linux x86-64 / ELF64.
- Outros alvos não são declarados suportados.
- Não existe ABI 1.0 congelada.
- Não existe garantia de compatibilidade entre lockfiles de versões futuras.
- O cache incremental de artefatos ELF existe em `.nebula/cache`, com chave SHA-256, checksum do artefato e invalidação por fonte, módulos, versão do compilador, alvo e otimização. O cache não é um contrato estável 1.0.

## Critério de congelamento

A equipe deverá publicar uma tabela de suporte por plataforma, arquitetura e versão mínima do sistema operacional antes de congelar ABI e gerar instaladores estáveis.
