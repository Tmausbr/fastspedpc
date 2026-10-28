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
