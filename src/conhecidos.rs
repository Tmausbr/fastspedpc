//! Base de "inimigos conhecidos": servicos e programas que costumam causar
//! travamentos, uso excessivo de recursos, riscos de seguranca ou sao adware.

use crate::diagnostico::Gravidade::{self, *};

pub struct Conhecido {
    /// minusculo; termina com '*' para prefixo
    pub padrao: &'static str,
    /// 'S' = servico, 'P' = processo/programa
    pub tipo: char,
    pub grav: Gravidade,
    pub titulo: &'static str,
    pub porque: &'static str,
}

macro_rules! k {
    ($p:expr, $t:expr, $g:expr, $ti:expr, $pq:expr) => {
        Conhecido { padrao: $p, tipo: $t, grav: $g, titulo: $ti, porque: $pq }
    };
}

pub static CONHECIDOS: &[Conhecido] = &[
    // ---------------------------------------------------------- servicos do Windows
    k!("sysmain", 'S', Medio, "SysMain (Superfetch)", "Pré-carrega programas na RAM. Em muitos PCs causa disco a 100% e travadas curtas durante jogos."),
    k!("wsearch", 'S', Medio, "Windows Search (indexação)", "Varre e indexa arquivos continuamente, gerando picos de disco e CPU logo após ligar o PC."),
    k!("diagtrack", 'S', Medio, "Telemetria do Windows", "Coleta e envia dados de uso em segundo plano."),
    k!("dmwappushservice", 'S', Baixo, "Roteamento de telemetria (WAP Push)", "Auxiliar da telemetria; não é necessário para uso doméstico."),
    k!("remoteregistry", 'S', Alto, "Registro Remoto", "Permite alterar o registro do Windows pela rede. Não deveria estar ligado em PC pessoal."),
    k!("tlntsvr", 'S', Alto, "Servidor Telnet", "Acesso remoto sem criptografia. Risco de segurança."),
    k!("sshd", 'S', Medio, "Servidor SSH", "Deixa o PC aceitando conexões remotas por SSH. Desligue se não usa."),
    k!("wmpnetworksvc", 'S', Baixo, "Compartilhamento do Windows Media Player", "Compartilha mídia na rede; raramente usado."),
    k!("retaildemo", 'S', Baixo, "Modo de demonstração de loja", "Não tem uso fora de lojas."),
    k!("fax", 'S', Baixo, "Fax", "Não tem uso na maioria dos PCs."),
    // ---------------------------------------------------------- fabricantes
    k!("supportassistagent", 'S', Alto, "Dell SupportAssist", "Faz varreduras de hardware periódicas; conhecido por causar stutter em jogos e usar muita RAM."),
    k!("dell supportassist remediation", 'S', Medio, "Dell SupportAssist Remediation", "Cria pontos de recuperação e monitora o sistema em segundo plano."),
    k!("delltechhub", 'S', Alto, "Dell TechHub", "Agentes de analytics/instrumentação que consomem RAM e CPU continuamente."),
    k!("dellclientmanagementservice", 'S', Medio, "Dell Client Management", "Gerenciamento e atualizações Dell em segundo plano."),
    k!("dellconnectedservicedelivery", 'S', Medio, "Dell Connected Service Delivery", "Telemetria e entrega de serviços Dell."),
    k!("hptouchpointanalyticsservice", 'S', Alto, "HP Touchpoint Analytics", "Telemetria da HP conhecida por uso alto de disco e CPU."),
    k!("hp*cap", 'S', Medio, "Agentes HP (App Helper/Diags/SysInfo)", "Coletores de diagnóstico e telemetria da HP."),
    k!("lightingservice", 'S', Alto, "ASUS Aura LightingService", "Serviço de RGB da ASUS com histórico de uso alto e constante de CPU."),
    k!("armourycrateservice", 'S', Medio, "ASUS Armoury Crate", "Pesado; mantém vários processos em segundo plano."),
    k!("asusoptimization", 'S', Baixo, "ASUS Optimization", "Serviço auxiliar ASUS."),
    k!("lenovovantageservice", 'S', Baixo, "Lenovo Vantage", "Útil para bateria/atualizações, mas roda tarefas periódicas."),
    k!("nahimicservice", 'S', Alto, "Nahimic (áudio)", "Driver de áudio conhecido por causar stutter, travamentos e quedas de FPS em jogos."),
    k!("nvtelemetrycontainer", 'S', Medio, "Telemetria NVIDIA", "Coleta de dados da NVIDIA; não afeta o driver."),
    k!("rzsurroundvadstreamingservice", 'S', Baixo, "Razer Surround", "Serviço de áudio virtual Razer."),
    k!("corsairservice", 'S', Medio, "Corsair iCUE", "Conhecido por uso alto de CPU/RAM."),
    // ---------------------------------------------------------- atualizadores
    k!("adobearmservice", 'S', Medio, "Atualizador Adobe", "Verifica atualizações da Adobe em segundo plano."),
    k!("agsservice", 'S', Medio, "Adobe Genuine Software", "Verifica licença da Adobe periodicamente."),
    k!("agmservice", 'S', Baixo, "Adobe Genuine Monitor", "Monitoramento de licença Adobe."),
    k!("gupdate", 'S', Baixo, "Google Update", "Atualizador do Google."),
    k!("gupdatem", 'S', Baixo, "Google Update (manual)", "Atualizador do Google."),
    k!("edgeupdate", 'S', Baixo, "Microsoft Edge Update", "Atualizador do Edge."),
    k!("bravem*", 'S', Baixo, "Brave Update", "Atualizador do Brave."),
    k!("clicktorunsvc", 'S', Info, "Office Click-to-Run", "Mantém o Office atualizado; normal se você usa Office."),
    // ---------------------------------------------------------- acesso remoto
    k!("teamviewer", 'S', Medio, "TeamViewer (acesso remoto)", "Permite controle remoto do PC a qualquer momento. Se não usa, desligue: é alvo comum de golpes."),
    k!("anydesk", 'S', Medio, "AnyDesk (acesso remoto)", "Permite controle remoto do PC. Muito usado em golpes; desligue se não usa."),
    k!("rustdesk", 'S', Medio, "RustDesk (acesso remoto)", "Permite controle remoto do PC."),
    k!("tightvnc", 'S', Alto, "Servidor VNC", "Acesso remoto, frequentemente sem senha forte."),
    k!("uvnc_service", 'S', Alto, "UltraVNC", "Acesso remoto, frequentemente sem senha forte."),
    // ---------------------------------------------------------- bancos de dados (ERP/PDV)
    k!("firebird*", 'S', Info, "Firebird (banco de dados)", "Usado por ERPs/PDVs. Consome RAM; normal se você usa o sistema neste PC."),
    k!("mongodb*", 'S', Info, "MongoDB (banco de dados)", "Usado por sistemas comerciais. Consome RAM."),
    k!("mssql*", 'S', Info, "SQL Server", "Banco de dados que reserva bastante RAM."),
    k!("mysql*", 'S', Info, "MySQL", "Banco de dados local."),
    k!("postgresql*", 'S', Info, "PostgreSQL", "Banco de dados local."),
    // ---------------------------------------------------------- adware / PUP / scareware
    k!("wavebrowser", 'P', Critico, "Wave Browser (adware)", "Navegador sequestrador: injeta anúncios, muda buscas e se reinstala sozinho. Desinstale."),
    k!("swvupdater", 'P', Critico, "Wave Browser Updater (adware)", "Componente do Wave Browser que o reinstala."),
    k!("onelaunch*", 'P', Critico, "OneLaunch (adware)", "Barra/navegador indesejado que roda na inicialização e coleta dados."),
    k!("webcompanion", 'P', Alto, "Web Companion (Adaware)", "Programa potencialmente indesejado que altera buscas."),
    k!("bytefence*", 'P', Alto, "ByteFence (PUP)", "Antimalware enganoso instalado junto com outros programas."),
    k!("segurazo*", 'P', Critico, "Segurazo/SAntivirus (PUP)", "Falso antivírus difícil de remover."),
    k!("santivirus*", 'P', Critico, "SAntivirus (PUP)", "Falso antivírus difícil de remover."),
    k!("reimage*", 'P', Alto, "Reimage (scareware)", "Otimizador que exibe falsos problemas para vender licença."),
    k!("restoro*", 'P', Alto, "Restoro (scareware)", "Otimizador que exibe falsos problemas para vender licença."),
    k!("pcaccelerate*", 'P', Alto, "PC Accelerate (PUP)", "Otimizador indesejado."),
    k!("pcoptimizerpro", 'P', Alto, "PC Optimizer Pro (PUP)", "Otimizador indesejado."),
    k!("advancedsystemcare*", 'P', Medio, "IObit Advanced SystemCare", "Otimizador que fica em segundo plano e costuma instalar outros apps."),
    k!("asc*tray", 'P', Medio, "IObit SystemCare (bandeja)", "Fica rodando em segundo plano."),
    k!("driverbooster*", 'P', Medio, "IObit Driver Booster", "Pode instalar drivers inadequados; não precisa ficar aberto."),
    k!("ccleaner*", 'P', Baixo, "CCleaner (monitoramento)", "Monitor em segundo plano não é necessário."),
    k!("mcuicnt", 'P', Medio, "McAfee (interface)", "Antivírus pré-instalado; com o Kaspersky gera conflito."),
    k!("kmspico*", 'P', Critico, "KMSPico (ativador pirata)", "Ativadores piratas frequentemente trazem mineradores e trojans."),
    k!("autokms*", 'P', Critico, "AutoKMS (ativador pirata)", "Ativadores piratas frequentemente trazem mineradores e trojans."),
    k!("kmsauto*", 'P', Critico, "KMSAuto (ativador pirata)", "Ativadores piratas frequentemente trazem mineradores e trojans."),
    k!("kmsss", 'P', Critico, "Serviço KMS pirata", "Ativadores piratas frequentemente trazem mineradores e trojans."),
    k!("utorrent", 'P', Medio, "uTorrent", "Cliente torrent em segundo plano consome rede e disco; versões antigas traziam minerador e anúncios."),
    k!("bittorrent", 'P', Medio, "BitTorrent", "Cliente torrent em segundo plano consome rede e disco."),
    k!("nahimic*", 'P', Alto, "Nahimic (áudio)", "Conhecido por causar stutter e travamentos em jogos."),
    k!("icue", 'P', Medio, "Corsair iCUE", "Conhecido por uso alto de CPU/RAM."),
    k!("razer synapse*", 'P', Medio, "Razer Synapse", "Pesado em segundo plano."),
    k!("armourycrate*", 'P', Medio, "ASUS Armoury Crate", "Vários processos pesados em segundo plano."),
    k!("lightingservice", 'P', Alto, "ASUS Aura", "Uso alto de CPU constante."),
    k!("dell.techhub*", 'P', Alto, "Dell TechHub (agentes)", "Analytics e instrumentação Dell; consomem RAM continuamente."),
    k!("supportassistagent", 'P', Alto, "Dell SupportAssist", "Varreduras periódicas causam stutter."),
    k!("hpsysinfo*", 'P', Medio, "HP System Info", "Coletor de informações HP."),
    k!("touchpointanalytics*", 'P', Alto, "HP Touchpoint Analytics", "Telemetria HP pesada."),
];

/// Antivirus: mais de um fornecedor ativo ao mesmo tempo = conflito.
pub static ANTIVIRUS: &[(&str, &[&str])] = &[
    ("Kaspersky", &["avp*", "ksde*", "kavfs*"]),
    ("McAfee", &["mfefire", "mfemms", "mfevtp", "mcafeeframework", "mcapexe", "mcafee*"]),
    ("Norton", &["norton*", "nortonsecurity", "nllservice"]),
    ("Avast", &["avast*", "aswbidsagent"]),
    ("AVG", &["avg antivirus", "avgsvc", "avgwd"]),
    ("ESET", &["ekrn"]),
    ("Bitdefender", &["vsserv", "bdredline*", "bdagent"]),
    ("Avira", &["avira*", "avguard"]),
    ("Panda", &["pavsrv*", "psanhost"]),
    ("Trend Micro", &["tmbmserver", "amsp"]),
    ("Malwarebytes (tempo real)", &["mbamservice"]),
    ("Sophos", &["sophos*", "savservice"]),
    ("Webroot", &["wrsvc"]),
];

pub fn casa_padrao(nome: &str, padrao: &str) -> bool {
    let n = nome.to_lowercase();
    if let Some(p) = padrao.strip_suffix('*') {
        n.starts_with(p)
    } else if padrao.contains('*') {
        let mut partes = padrao.split('*');
        let a = partes.next().unwrap_or("");
        let b = partes.next().unwrap_or("");
        n.starts_with(a) && n.ends_with(b)
    } else {
        n == padrao
    }
}

pub fn buscar(nome: &str, tipo: char) -> Option<&'static Conhecido> {
    CONHECIDOS.iter().find(|k| k.tipo == tipo && casa_padrao(nome, k.padrao))
}

/// Nomes com erros de digitacao imitando processos do Windows.
pub const IMITACOES: &[&str] = &[
    "svhost", "scvhost", "svch0st", "svchosts", "svchost32", "svcchost", "lsasss", "lsas", "isass", "csrsss",
    "csrs", "winlogin", "wininit32", "expiorer", "explore", "explorar", "dllhst", "taskhost32", "rundl32",
    "spoolsvc", "smsss", "conhosts", "runtimebroker32", "winservice", "windowsupdate", "wuauclt32",
];

pub const PORTAS_MINERACAO: &[u16] = &[
    1111, 2222, 3032, 3333, 3334, 3335, 3357, 4444, 5555, 5730, 6666, 7777, 8008, 8888, 9999, 10128, 10300,
    13333, 14433, 14444, 15555, 20535, 45560, 45700,
];
