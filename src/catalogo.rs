//! Catalogo de itens que o WinOpti pode pausar temporariamente.
//! Padroes de processo (sempre minusculos, sem ".exe"):
//!   "nome"        -> nome exato do executavel
//!   "prefixo*"    -> nome comeca com o prefixo
//!   "path:texto"  -> caminho do executavel contem o texto (nao vai para o .bat)

pub struct Item {
    pub id: &'static str,
    pub nome: &'static str,
    pub desc: &'static str,
    pub cat: &'static str,
    pub servicos: &'static [&'static str],
    pub processos: &'static [&'static str],
    pub energia: bool,
    /// Faz parte do perfil Otimizado (trabalho + uso geral)
    pub otimizado: bool,
    /// Faz parte do perfil Gamer
    pub gamer: bool,
    /// Reabrir o app ao restaurar (se estava aberto)
    pub reabrir: bool,
}

pub const CATEGORIAS: &[&str] = &[
    "Telemetria e diagnóstico do Windows",
    "Recursos do Windows",
    "Atualizações em segundo plano",
    "Fabricante do notebook",
    "Comunicação",
    "Nuvem e sincronização",
    "Celular e IA",
    "Utilitários",
    "Navegadores",
    "Trabalho (bancos de dados, ERP, PDV)",
    "Energia",
];

macro_rules! item {
    ($id:expr, $nome:expr, $desc:expr, $cat:expr, s=$s:expr, p=$p:expr, o=$o:expr, g=$g:expr, r=$r:expr) => {
        Item { id: $id, nome: $nome, desc: $desc, cat: $cat, servicos: $s, processos: $p,
               energia: false, otimizado: $o, gamer: $g, reabrir: $r }
    };
}

pub static ITENS: &[Item] = &[
    // ---------------- Telemetria
    item!("telemetria", "Telemetria do Windows", "Envio de dados de uso para a Microsoft (DiagTrack).",
        "Telemetria e diagnóstico do Windows", s=&["DiagTrack", "dmwappushservice"], p=&[], o=true, g=true, r=false),
    item!("erros", "Relatório de erros", "Coleta e envio de relatórios de falhas (WER).",
        "Telemetria e diagnóstico do Windows", s=&["WerSvc"], p=&[], o=true, g=true, r=false),
    item!("compat", "Assistente de compatibilidade", "Monitora programas antigos em segundo plano.",
        "Telemetria e diagnóstico do Windows", s=&["PcaSvc"], p=&[], o=true, g=true, r=false),
    item!("diag", "Política de diagnóstico", "Detecção automática de problemas do Windows.",
        "Telemetria e diagnóstico do Windows", s=&["DPS", "WdiServiceHost", "WdiSystemHost"], p=&[], o=false, g=true, r=false),
    // ---------------- Recursos do Windows
    item!("busca", "Indexação da pesquisa", "Indexa arquivos no disco. A busca do Iniciar fica mais lenta até restaurar.",
        "Recursos do Windows", s=&["WSearch"], p=&[], o=false, g=true, r=false),
    item!("sysmain", "SysMain (Superfetch)", "Pré-carrega apps na memória; pode gerar uso de disco durante jogos.",
        "Recursos do Windows", s=&["SysMain"], p=&[], o=false, g=true, r=false),
    item!("impressao", "Spooler de impressão", "Não será possível imprimir até restaurar.",
        "Recursos do Windows", s=&["Spooler"], p=&[], o=false, g=true, r=false),
    item!("extras_win", "Fax, Mapas, Demo e Localização", "Serviços raramente usados.",
        "Recursos do Windows", s=&["Fax", "MapsBroker", "RetailDemo", "lfsvc"], p=&[], o=true, g=true, r=false),
    item!("widgets", "Widgets e Copilot", "Painel de widgets e Copilot em segundo plano.",
        "Recursos do Windows", s=&[], p=&["widgets", "widgetservice", "copilot", "m365copilot"], o=true, g=true, r=false),
    // ---------------- Atualizacoes
    item!("winupdate", "Windows Update", "Pausa downloads de atualizações durante a sessão.",
        "Atualizações em segundo plano", s=&["wuauserv", "UsoSvc", "BITS", "DoSvc"], p=&[], o=false, g=true, r=false),
    item!("updaters", "Atualizadores de programas", "Google, Edge, Firefox, Adobe, MiniTool, Lightshot e outros.",
        "Atualizações em segundo plano",
        s=&["gupdate", "gupdatem", "edgeupdate", "edgeupdatem", "MozillaMaintenance", "AdobeARMservice", "AdobeUpdateService", "AGSService", "AGMService"],
        p=&["updatechecker", "adobearm", "jusched", "path:\\skillbrains\\updater\\"], o=true, g=true, r=false),
    // ---------------- Fabricante
    item!("dell", "Dell SupportAssist e TechHub", "Agentes de suporte, analytics e telemetria da Dell (pesados).",
        "Fabricante do notebook",
        s=&["SupportAssistAgent", "Dell SupportAssist Remediation", "DellClientManagementService", "DellConnectedServiceDelivery", "DellTechHub"],
        p=&["supportassistagent", "dell.coreservices.client", "dell.techhub*"], o=true, g=true, r=false),
    item!("hp_asus", "Agentes HP / ASUS", "Telemetria e diagnóstico de HP e ASUS (não mexe em Omen/Armoury).",
        "Fabricante do notebook",
        s=&["HPAppHelperCap", "HPDiagsCap", "HPNetworkCap", "HPSysInfoCap", "HpTouchpointAnalyticsService", "AsusUpdateCheck"],
        p=&[], o=true, g=true, r=false),
    item!("alienfx", "Iluminação RGB (AlienFX)", "As luzes do teclado podem mudar até restaurar.",
        "Fabricante do notebook", s=&[], p=&["alienfxsubagent"], o=false, g=false, r=false),
    // ---------------- Comunicacao
    item!("whatsapp", "WhatsApp", "Fecha o WhatsApp Desktop (reabre ao restaurar).",
        "Comunicação", s=&[], p=&["whatsapp.root", "whatsapp"], o=false, g=true, r=true),
    item!("teams", "Microsoft Teams", "Fecha o Teams (reabre ao restaurar).",
        "Comunicação", s=&[], p=&["ms-teams", "teams"], o=false, g=true, r=true),
    item!("telegram", "Telegram, Zoom e Slack", "Fecha apps de mensagens e reuniões.",
        "Comunicação", s=&[], p=&["telegram", "zoom", "slack"], o=false, g=true, r=true),
    item!("discord", "Discord", "Muitos jogadores usam durante o jogo, por isso fica desmarcado.",
        "Comunicação", s=&[], p=&["discord"], o=false, g=false, r=true),
    // ---------------- Nuvem
    item!("onedrive", "OneDrive", "Pausa a sincronização (reabre ao restaurar).",
        "Nuvem e sincronização", s=&[], p=&["onedrive"], o=false, g=true, r=true),
    item!("gdrive", "Google Drive", "Pausa a sincronização (reabre ao restaurar).",
        "Nuvem e sincronização", s=&[], p=&["googledrivefs"], o=false, g=true, r=true),
    item!("dropbox", "Dropbox", "Pausa a sincronização (reabre ao restaurar).",
        "Nuvem e sincronização", s=&[], p=&["dropbox"], o=false, g=true, r=true),
    // ---------------- Celular e IA
    item!("phonelink", "Vincular ao Celular", "Phone Link / CrossDevice em segundo plano.",
        "Celular e IA", s=&[], p=&["phoneexperiencehost", "crossdeviceservice", "yourphone"], o=true, g=true, r=false),
    item!("chatgpt", "ChatGPT (app)", "Fecha o app do ChatGPT/Codex.",
        "Celular e IA", s=&[], p=&["chatgpt", "path:\\openai.codex_", "path:\\openai.chatgpt"], o=false, g=true, r=true),
    item!("claude", "Claude (app)", "Fecha o app Claude e o serviço de VM (CoworkVMService).",
        "Celular e IA", s=&["CoworkVMService"], p=&["path:\\claude_"], o=false, g=true, r=true),
    // ---------------- Utilitarios
    item!("lightshot", "Lightshot", "Captura de tela em segundo plano.",
        "Utilitários", s=&[], p=&["lightshot"], o=false, g=true, r=true),
    item!("intel", "Painel gráfico Intel", "Painel da Intel; o driver continua funcionando.",
        "Utilitários", s=&[], p=&["intelgraphicssoftware"], o=true, g=true, r=false),
    item!("downloads", "Gerenciadores de download", "IDM e Free Download Manager.",
        "Utilitários", s=&[], p=&["idman", "fdm"], o=false, g=true, r=true),
    item!("pdf", "PDF24 e Adobe em 2o plano", "PDF24, Creative Cloud e processos auxiliares da Adobe.",
        "Utilitários", s=&[], p=&["pdf24", "creative cloud", "ccxprocess", "adobecollabsync", "acrotray", "cclibrary"], o=true, g=true, r=false),
    item!("spotify", "Spotify", "Desmarcado: muita gente ouve música jogando.",
        "Utilitários", s=&[], p=&["spotify"], o=false, g=false, r=true),
    item!("voicemod", "Voicemod", "Modificador de voz.",
        "Utilitários", s=&[], p=&["voicemod"], o=false, g=false, r=true),
    // ---------------- Navegadores
    item!("edge", "Microsoft Edge", "Fecha o Edge (abas não salvas podem ser perdidas).",
        "Navegadores", s=&[], p=&["msedge"], o=false, g=false, r=true),
    item!("chrome", "Google Chrome", "Fecha o Chrome.",
        "Navegadores", s=&[], p=&["chrome"], o=false, g=false, r=true),
    item!("firefox", "Mozilla Firefox", "Fecha o Firefox.",
        "Navegadores", s=&[], p=&["firefox"], o=false, g=false, r=true),
    // ---------------- Trabalho
    item!("firebird", "Firebird", "Banco usado por ERPs e PDVs (Hiper e outros). Pare só se não usa neste PC.",
        "Trabalho (bancos de dados, ERP, PDV)", s=&["FirebirdGuardianDefaultInstance", "FirebirdServerDefaultInstance"], p=&[], o=false, g=false, r=false),
    item!("bancos", "SQL Server, MySQL, MongoDB", "Bancos de dados locais de sistemas comerciais.",
        "Trabalho (bancos de dados, ERP, PDV)",
        s=&["MSSQL$SQLEXPRESS", "SQLBrowser", "SQLWriter", "MySQL80", "MySQL57", "MongoDB", "MongoDBDigisat"], p=&[], o=false, g=false, r=false),
    item!("erp", "Sincronizadores de ERP/PDV", "Digisat, CPServer, Hiper Sync.",
        "Trabalho (bancos de dados, ERP, PDV)", s=&["SincronizadorDigisat", "CPServerService"],
        p=&["hiperpdvsync", "hiper.sync.loja", "cpserver"], o=false, g=false, r=true),
    // ---------------- Energia
    Item { id: "energia", nome: "Plano de energia Alto desempenho",
        desc: "Troca o plano de energia. O original volta sozinho no próximo login.",
        cat: "Energia", servicos: &[], processos: &[], energia: true, otimizado: false, gamer: true, reabrir: false },
];

/// Processos que nunca sao fechados.
pub const PROTEGIDOS: &[&str] = &[
    "system", "idle", "registry", "smss", "csrss", "wininit", "winlogon", "services", "lsass", "svchost",
    "explorer", "dwm", "fontdrvhost", "sihost", "ctfmon", "audiodg", "conhost", "runtimebroker",
    "startmenuexperiencehost", "shellexperiencehost", "searchhost", "textinputhost", "taskmgr",
    "securityhealthservice", "securityhealthsystray", "msmpeng", "nissrv", "avp", "avpui", "kpm",
    "kpm_service", "ksde", "ksdeui", "winopti", "lsaiso", "memory compression", "wudfhost", "spoolsv",
    "dllhost", "taskhostw", "msedgewebview2", "nvcontainer", "nvdisplay.container", "steam", "steamwebhelper",
    "epicgameslauncher", "eadesktop", "upc", "battle.net", "riotclientservices", "awcc", "rtkauduservice64",
];

pub const MINERADORES: &[&str] = &[
    "xmrig", "xmr-stak", "nanominer", "nbminer", "t-rex", "phoenixminer", "lolminer", "gminer", "teamredminer",
    "ethminer", "cgminer", "bfgminer", "ccminer", "claymore", "minerd", "cpuminer", "srbminer", "nicehash",
    "excavator", "wildrig", "kawpowminer", "bminer", "z-enemy",
];

pub const CMD_MINERACAO: &[&str] = &[
    "stratum+tcp", "stratum+ssl", "stratum2+", "--donate-level", "cryptonight", "randomx", "nicehash",
    "--coin", "-a rx/0", "--algo=rx", "pool.minexmr", "moneroocean", "2miners", "nanopool", "f2pool",
];

pub const NOMES_SISTEMA: &[&str] = &[
    "svchost", "csrss", "lsass", "explorer", "conhost", "dllhost", "winlogon", "services", "smss",
    "rundll32", "taskhostw", "wininit", "spoolsv", "dwm", "runtimebroker",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Perfil {
    Padrao,
    Otimizado,
    Gamer,
    Personalizado,
}

impl Perfil {
    pub fn nome(self) -> &'static str {
        match self {
            Perfil::Padrao => "Padrão",
            Perfil::Otimizado => "Otimizado",
            Perfil::Gamer => "Gamer",
            Perfil::Personalizado => "Personalizado",
        }
    }
    pub fn selecao(self) -> Option<Vec<bool>> {
        match self {
            Perfil::Padrao => Some(vec![false; ITENS.len()]),
            Perfil::Otimizado => Some(ITENS.iter().map(|i| i.otimizado).collect()),
            Perfil::Gamer => Some(ITENS.iter().map(|i| i.gamer).collect()),
            Perfil::Personalizado => None,
        }
    }
}
