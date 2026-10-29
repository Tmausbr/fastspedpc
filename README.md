# WinOpti (fastspedpc)

Otimizador de desempenho do Windows feito em **Rust** (interface com egui).
Pausa serviços e aplicativos desnecessários **somente durante a sessão**: ao reiniciar o computador, ou ao clicar em **Restaurar**, tudo volta exatamente como era.
É pensado para quem usa o mesmo PC para jogar e para trabalhar.

## Perfis

| Perfil | Para quê | O que faz |
|---|---|---|
| **Padrão** | Windows normal | Desfaz tudo: inicia os serviços parados, volta o plano de energia e reabre os apps fechados. |
| **Otimizado** | Trabalho e uso diário | Corta telemetria, agentes do fabricante (Dell/HP/ASUS), atualizadores, widgets/Copilot e Phone Link. Mantém WhatsApp, Teams, nuvem e impressão. |
| **Gamer** | Jogos | Tudo do Otimizado mais WhatsApp, Teams, OneDrive/Drive, ChatGPT/Claude, indexação, SysMain, spooler, Windows Update e plano **Alto desempenho**. Discord, Spotify, Steam e launchers continuam abertos. |
| **Personalizado** | Você escolhe | Marque item por item na aba *Serviços e Aplicativos*. |

## Diagnóstico profundo (mapa dos inimigos)

Roda sozinho ao abrir o app, **antes** de qualquer otimização, e é somente leitura. Ele dá uma nota de 0 a 100 e organiza o que encontrou em 9 categorias:

| Categoria | O que verifica |
|---|---|
| ⛏ Mineradores e malware | Nomes e linhas de comando de mineradores; portas de pools (stratum); GPU alta em programas sem assinatura; processos imitando o Windows (svchost fora do System32, nomes com erro de digitação); serviços rodando de AppData/Temp. |
| 🛡 Segurança e sequestros | Adware/PUP conhecidos (Wave Browser, OneLaunch, Segurazo…); ativadores piratas; **dois antivírus ativos**; Winlogon (Shell/Userinit) alterado; IFEO (redirecionamento de programas); AppInit_DLLs; proxy/PAC; exclusões do Defender; arquivo hosts; serviços com caminho sem aspas; acesso remoto (TeamViewer, AnyDesk, VNC, SSH, Registro Remoto). |
| 📈 CPU, memória e disco | Duas amostras com CPU, RAM, **GPU** e **disco** por processo; o que consome em repouso; RAM quase cheia; assinatura digital de cada programa. |
| 🚀 Iniciando com o Windows | Run/RunOnce (usuário, máquina, 32 bits) e pastas Inicializar, com scripts invisíveis (.vbs/.js/mshta), entradas quebradas e programas sem assinatura. |
| ⏰ Tarefas agendadas ocultas | Lê todas as tarefas, inclusive as **ocultas**. Aponta PowerShell codificado, downloads, tarefas camufladas na pasta da Microsoft e tarefas que repetem a cada poucos minutos. |
| ⚙ Serviços problemáticos | Uma base de serviços conhecidos por travar ou pesar (SysMain, Search, Dell SupportAssist/TechHub, HP Touchpoint, ASUS Aura, Nahimic…), além de serviços quebrados ou sem assinatura. |
| 🌐 Rede | Conexões em portas de mineração e processos com conexões em excesso. |
| 🖥 Configuração do Windows | Tempo sem reiniciar, Inicialização Rápida, reinício pendente, disco cheio, temporários, Game DVR, HAGS, Modo de Jogo e arquivo de paginação. |
| 🔋 Bateria (notebook) | Saúde (capacidade atual comparada à original), ciclos, autonomia estimada, uso na bateria e dicas de conservação. |

Cada problema vem com a explicação de **por que** ele atrapalha e com uma ação:
- **Fechar agora** e **Parar até reiniciar:** temporárias.
- **Não iniciar com o Windows** e **Desativar tarefa:** reversíveis na aba *Desativados*.
- **Incluir na otimização**, **Abrir local** e **Pesquisar na web**.

O relatório completo pode ser salvo em .txt.

## Recursos

- **Painel** com uso de CPU, memória e disco em tempo real.
- **Varredura e segurança:**
  - Procura mineradores (por nome e por linha de comando), processos imitando o Windows, executáveis rodando de pastas temporárias e bloqueios no arquivo `hosts`.
  - Lista os programas que mais pesam, e você pode marcá-los para fechar também.
- **Gerar Script (.bat):** cria `WinOpti-<Perfil>.bat` e `WinOpti-Restaurar.bat` para usar sem abrir o app.
- **Ações rápidas:** restaurar, reiniciar o PC, limpar temporários e gerenciar os apps de inicialização.

## Por que é temporário

- Os serviços são apenas **parados** (`net stop`). O tipo de inicialização **nunca** é alterado.
- Os aplicativos são **fechados**, não desinstalados nem bloqueados.
- Se o plano de energia for trocado, a volta do plano original é agendada para o próximo login (`RunOnce`).
- O app detecta quando o PC foi reiniciado e descarta o estado antigo automaticamente.
- Nunca fecha processos do Windows, antivírus (Kaspersky, Defender), drivers ou launchers de jogos.
- Bancos de dados de ERP/PDV (Firebird, SQL Server, MongoDB) só param se você marcar.

## Baixar

Abra **Actions → Build WinOpti → execução mais recente → Artifacts → WinOpti**.
Se houver uma tag `v*`, o executável também fica disponível em **Releases**.

O app pede permissão de administrador ao abrir, porque precisa disso para parar serviços.

## Compilar

```powershell
# Windows, com Rust instalado (https://rustup.rs)
cargo build --release
# resultado: target\release\winopti.exe
```

## Estrutura

- `src/catalogo.rs`: lista de serviços e processos, perfis e processos protegidos.
- `src/sistema.rs`: parar e iniciar serviços, fechar e reabrir apps, energia, varredura e estado.
- `src/bat.rs`: gerador dos scripts .bat.
- `src/main.rs`: interface gráfica.
