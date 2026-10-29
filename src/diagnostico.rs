//! Diagnostico profundo: mapeia os "inimigos" do PC antes de otimizar.
//! Somente leitura. As acoes (desativar inicializacao/tarefa) so ocorrem
//! quando o usuario clica, e sao reversiveis.

use crate::catalogo::{CMD_MINERACAO, MINERADORES, NOMES_SISTEMA, PROTEGIDOS};
use crate::conhecidos::*;
use crate::sistema::{achar_guid, cmd, pasta_dados, plano_ativo, Shared, GUID_ALTO_DESEMPENHO};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, UpdateKind};
use winreg::enums::*;
use winreg::RegKey;

// =================================================================== tipos
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Gravidade {
    Critico,
    Alto,
    Medio,
    Baixo,
    Info,
}

impl Gravidade {
    pub fn nome(self) -> &'static str {
        match self {
            Gravidade::Critico => "Crítico",
            Gravidade::Alto => "Alto",
            Gravidade::Medio => "Médio",
            Gravidade::Baixo => "Baixo",
            Gravidade::Info => "Info",
        }
    }
    fn peso(self) -> f32 {
        match self {
            Gravidade::Critico => 20.0,
            Gravidade::Alto => 7.0,
            Gravidade::Medio => 2.5,
            Gravidade::Baixo => 0.7,
            Gravidade::Info => 0.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Categoria {
    Mineracao,
    Seguranca,
    Inicializacao,
    Tarefas,
    Servicos,
    Desempenho,
    Rede,
    Sistema,
    Bateria,
}

impl Categoria {
    pub fn nome(self) -> &'static str {
        match self {
            Categoria::Mineracao => "Mineradores e malware",
            Categoria::Seguranca => "Segurança e sequestros",
            Categoria::Inicializacao => "Iniciando com o Windows",
            Categoria::Tarefas => "Tarefas agendadas ocultas",
            Categoria::Servicos => "Serviços problemáticos",
            Categoria::Desempenho => "CPU, memória e disco",
            Categoria::Rede => "Conexões de rede",
            Categoria::Sistema => "Configuração do Windows",
            Categoria::Bateria => "Bateria e energia",
        }
    }
    pub fn icone(self) -> &'static str {
        match self {
            Categoria::Mineracao => "⛏",
            Categoria::Seguranca => "🛡",
            Categoria::Inicializacao => "🚀",
            Categoria::Tarefas => "⏰",
            Categoria::Servicos => "⚙",
            Categoria::Desempenho => "📈",
            Categoria::Rede => "🌐",
            Categoria::Sistema => "🖥",
            Categoria::Bateria => "🔋",
        }
    }
    pub const TODAS: [Categoria; 9] = [
        Categoria::Mineracao,
        Categoria::Seguranca,
        Categoria::Desempenho,
        Categoria::Inicializacao,
        Categoria::Tarefas,
        Categoria::Servicos,
        Categoria::Rede,
        Categoria::Sistema,
        Categoria::Bateria,
    ];
}

#[derive(Clone, Debug, PartialEq)]
pub enum Acao {
    Nenhuma,
    /// stem do processo (sem .exe)
    FecharProcesso(String),
    PararServico(String),
    DesativarInicio(EntradaInicio),
    DesativarTarefa(String),
    AbrirLocal(String),
}

#[derive(Clone, Debug)]
pub struct Achado {
    pub grav: Gravidade,
    pub cat: Categoria,
    pub titulo: String,
    pub detalhe: String,
    pub porque: String,
    pub caminho: String,
    pub acao: Acao,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntradaInicio {
    /// HKCU / HKLM / HKLM32 / PastaUsuario / PastaComum
    pub origem: String,
    pub nome: String,
    pub comando: String,
    pub ativo: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Bateria {
    pub tem: bool,
    pub na_tomada: bool,
    pub carga: u8,
    pub projetada_mwh: u64,
    pub atual_mwh: u64,
    pub ciclos: Option<u64>,
    pub quimica: String,
    pub fabricante: String,
    pub modelo: String,
    pub autonomia_min: Option<u64>,
    pub autonomia_projetada_min: Option<u64>,
}

impl Bateria {
    pub fn saude(&self) -> Option<f32> {
        if self.projetada_mwh > 0 && self.atual_mwh > 0 {
            Some((self.atual_mwh as f32 / self.projetada_mwh as f32 * 100.0).min(100.0))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProcResumo {
    pub nome: String,
    pub stem: String,
    pub qtd: usize,
    pub ram_mb: f64,
    pub cpu: f32,
    pub disco_mbs: f64,
    pub gpu: f32,
    pub caminho: String,
    pub assinado: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct Diagnostico {
    pub achados: Vec<Achado>,
    pub inicio: Vec<EntradaInicio>,
    pub top: Vec<ProcResumo>,
    pub bateria: Bateria,
    pub nota: u8,
    pub segundos: f32,
    pub total_proc: usize,
    pub total_servicos: usize,
    pub total_tarefas: usize,
    pub ram_total_gb: f64,
    pub ram_uso_pct: f64,
}

impl Diagnostico {
    pub fn contar(&self, g: Gravidade) -> usize {
        self.achados.iter().filter(|a| a.grav == g).count()
    }
    pub fn da_categoria(&self, c: Categoria) -> Vec<&Achado> {
        self.achados.iter().filter(|a| a.cat == c).collect()
    }
}

// =================================================================== utilidades
fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn pwstr(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

fn win_dir() -> String {
    std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()).to_lowercase()
}

pub fn expandir(s: &str) -> String {
    let mut out = String::new();
    let mut resto = s;
    while let Some(i) = resto.find('%') {
        out.push_str(&resto[..i]);
        let depois = &resto[i + 1..];
        if let Some(j) = depois.find('%') {
            let var = &depois[..j];
            match std::env::var(var) {
                Ok(v) => out.push_str(&v),
                Err(_) => {
                    out.push('%');
                    out.push_str(var);
                    out.push('%');
                }
            }
            resto = &depois[j + 1..];
        } else {
            out.push('%');
            resto = depois;
        }
    }
    out.push_str(resto);
    out
}

/// Extrai o executavel de uma linha de comando.
pub fn exe_de(comando: &str) -> String {
    let c = expandir(comando.trim());
    let c = c.trim_start_matches("\\??\\");
    if let Some(r) = c.strip_prefix('"') {
        return r.split('"').next().unwrap_or("").to_string();
    }
    let low = c.to_lowercase();
    for ext in [".exe", ".bat", ".cmd", ".vbs", ".js", ".ps1", ".com", ".scr", ".dll", ".lnk"] {
        if let Some(i) = low.find(ext) {
            let fim = i + ext.len();
            if low[fim..].is_empty() || low[fim..].starts_with(' ') || low[fim..].starts_with(',') {
                let mut p = c[..fim].to_string();
                if p.to_lowercase().starts_with("system32\\") {
                    p = format!("{}\\{}", win_dir(), p);
                }
                return p;
            }
        }
    }
    c.split(' ').next().unwrap_or("").to_string()
}

fn pasta_gravavel(caminho: &str) -> bool {
    let l = caminho.to_lowercase();
    l.contains("\\appdata\\local\\temp\\")
        || l.contains("\\windows\\temp\\")
        || l.contains("\\users\\public\\")
        || l.contains("$recycle.bin")
        || l.contains("\\appdata\\roaming\\")
        || (l.contains("\\programdata\\") && l.matches('\\').count() <= 3)
        || l.contains("\\downloads\\")
}

fn pasta_temporaria(caminho: &str) -> bool {
    let l = caminho.to_lowercase();
    l.contains("\\temp\\") || l.contains("\\users\\public\\") || l.contains("$recycle.bin")
}

fn confiavel_por_local(caminho: &str) -> bool {
    let l = caminho.to_lowercase();
    l.starts_with(&format!("{}\\", win_dir())) || l.contains("\\windowsapps\\")
}

// =================================================================== assinatura digital
static CACHE_ASSIN: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();

/// true = assinatura Authenticode valida embutida no arquivo.
pub fn assinado(caminho: &str) -> Option<bool> {
    if caminho.is_empty() || !Path::new(caminho).is_file() {
        return None;
    }
    if confiavel_por_local(caminho) {
        return Some(true);
    }
    let key = caminho.to_lowercase();
    let cache = CACHE_ASSIN.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(v) = cache.lock().unwrap().get(&key) {
        return Some(*v);
    }
    let ok = verificar_assinatura(caminho);
    cache.lock().unwrap().insert(key, ok);
    Some(ok)
}

fn verificar_assinatura(caminho: &str) -> bool {
    use windows_sys::Win32::Security::WinTrust::*;
    let wpath = w(caminho);
    unsafe {
        let mut fi: WINTRUST_FILE_INFO = std::mem::zeroed();
        fi.cbStruct = std::mem::size_of::<WINTRUST_FILE_INFO>() as u32;
        fi.pcwszFilePath = wpath.as_ptr();
        let mut wd: WINTRUST_DATA = std::mem::zeroed();
        wd.cbStruct = std::mem::size_of::<WINTRUST_DATA>() as u32;
        wd.dwUIChoice = WTD_UI_NONE;
        wd.fdwRevocationChecks = WTD_REVOKE_NONE;
        wd.dwUnionChoice = WTD_CHOICE_FILE;
        wd.Anonymous.pFile = &mut fi;
        wd.dwStateAction = WTD_STATEACTION_VERIFY;
        wd.dwProvFlags = WTD_CACHE_ONLY_URL_RETRIEVAL;
        let mut acao = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let r = WinVerifyTrust(std::ptr::null_mut(), &mut acao, &mut wd as *mut _ as *mut _);
        wd.dwStateAction = WTD_STATEACTION_CLOSE;
        WinVerifyTrust(std::ptr::null_mut(), &mut acao, &mut wd as *mut _ as *mut _);
        r == 0
    }
}

// =================================================================== GPU por processo (PDH)
fn gpu_por_pid() -> HashMap<u32, f32> {
    use windows_sys::Win32::System::Performance::*;
    let mut mapa: HashMap<u32, f32> = HashMap::new();
    unsafe {
        let mut q: isize = 0;
        if PdhOpenQueryW(std::ptr::null(), 0, &mut q) != 0 {
            return mapa;
        }
        let mut c: isize = 0;
        let caminho = w("\\GPU Engine(*)\\Utilization Percentage");
        if PdhAddEnglishCounterW(q, caminho.as_ptr(), 0, &mut c) != 0 {
            PdhCloseQuery(q);
            return mapa;
        }
        PdhCollectQueryData(q);
        std::thread::sleep(Duration::from_millis(1200));
        PdhCollectQueryData(q);
        let mut tam: u32 = 0;
        let mut n: u32 = 0;
        let r = PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut tam, &mut n, std::ptr::null_mut());
        if r == PDH_MORE_DATA && tam > 0 {
            let mut buf: Vec<u8> = vec![0; tam as usize + 64];
            let itens = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut tam, &mut n, itens) == 0 {
                for i in 0..n as usize {
                    let it = &*itens.add(i);
                    let nome = pwstr(it.szName);
                    let v = it.FmtValue.Anonymous.doubleValue as f32;
                    if let Some(rest) = nome.strip_prefix("pid_") {
                        if let Some(pid) = rest.split('_').next().and_then(|p| p.parse::<u32>().ok()) {
                            let e = mapa.entry(pid).or_insert(0.0);
                            if v > *e {
                                *e = v; // maior motor (3D, Compute, Video...)
                            }
                        }
                    }
                }
            }
        }
        PdhCloseQuery(q);
    }
    mapa
}

// =================================================================== rede (TCP IPv4)
struct Conexao {
    pid: u32,
    remoto: String,
    porta: u16,
}

fn conexoes_tcp() -> Vec<Conexao> {
    use windows_sys::Win32::NetworkManagement::IpHelper::*;
    let mut v = vec![];
    unsafe {
        let mut tam: u32 = 0;
        GetExtendedTcpTable(std::ptr::null_mut(), &mut tam, 0, 2, TCP_TABLE_OWNER_PID_ALL, 0);
        if tam == 0 {
            return v;
        }
        let mut buf: Vec<u8> = vec![0; tam as usize + 256];
        if GetExtendedTcpTable(buf.as_mut_ptr() as *mut _, &mut tam, 0, 2, TCP_TABLE_OWNER_PID_ALL, 0) != 0 {
            return v;
        }
        let n = *(buf.as_ptr() as *const u32) as usize;
        let base = buf.as_ptr().add(4) as *const MIB_TCPROW_OWNER_PID;
        for i in 0..n {
            let r = &*base.add(i);
            if r.dwState != 5 {
                continue; // somente ESTABLISHED
            }
            let ip = r.dwRemoteAddr.to_le_bytes();
            if ip[0] == 127 || ip == [0, 0, 0, 0] {
                continue;
            }
            let porta = u16::from_be((r.dwRemotePort & 0xffff) as u16);
            v.push(Conexao { pid: r.dwOwningPid, remoto: format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]), porta });
        }
    }
    v
}

// =================================================================== bateria
pub fn ler_bateria() -> Bateria {
    use windows_sys::Win32::System::Power::*;
    let mut b = Bateria::default();
    unsafe {
        let mut st: SYSTEM_POWER_STATUS = std::mem::zeroed();
        if GetSystemPowerStatus(&mut st) != 0 {
            b.tem = st.BatteryFlag != 128 && st.BatteryFlag != 255;
            b.na_tomada = st.ACLineStatus == 1;
            b.carga = if st.BatteryLifePercent <= 100 { st.BatteryLifePercent } else { 0 };
        }
    }
    if !b.tem {
        return b;
    }
    let arq = std::env::temp_dir().join("winopti_bateria.xml");
    let arq_s = arq.to_string_lossy().into_owned();
    let _ = cmd("powercfg", &["/batteryreport", "/xml", "/output", &arq_s]);
    if let Ok(bytes) = std::fs::read(&arq) {
        let xml = decodificar(&bytes);
        if let Some(bat) = trecho(&xml, "<Battery>", "</Battery>") {
            b.projetada_mwh = tag(bat, "DesignCapacity").and_then(|v| v.parse().ok()).unwrap_or(0);
            b.atual_mwh = tag(bat, "FullChargeCapacity").and_then(|v| v.parse().ok()).unwrap_or(0);
            b.ciclos = tag(bat, "CycleCount").and_then(|v| v.parse().ok()).filter(|c: &u64| *c > 0);
            b.quimica = tag(bat, "Chemistry").unwrap_or_default();
            b.fabricante = tag(bat, "Manufacturer").unwrap_or_default();
            b.modelo = tag(bat, "Id").unwrap_or_default();
        }
        if let Some(rt) = trecho(&xml, "<RuntimeEstimates>", "</RuntimeEstimates>") {
            if let Some(fc) = trecho(rt, "<FullChargeCapacity>", "</FullChargeCapacity>") {
                b.autonomia_min = tag(fc, "ActiveRuntime").and_then(|d| duracao_iso(&d));
            }
            if let Some(dc) = trecho(rt, "<DesignCapacity>", "</DesignCapacity>") {
                b.autonomia_projetada_min = tag(dc, "ActiveRuntime").and_then(|d| duracao_iso(&d));
            }
        }
        let _ = std::fs::remove_file(&arq);
    }
    b
}

fn decodificar(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let u: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&u)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn trecho<'a>(s: &'a str, ini: &str, fim: &str) -> Option<&'a str> {
    let i = s.find(ini)? + ini.len();
    let j = s[i..].find(fim)? + i;
    Some(&s[i..j])
}

fn tag(s: &str, nome: &str) -> Option<String> {
    let abre = format!("<{nome}>");
    let fecha = format!("</{nome}>");
    trecho(s, &abre, &fecha).map(|v| v.trim().to_string())
}

fn duracao_iso(d: &str) -> Option<u64> {
    let t = d.split('T').nth(1)?;
    let mut min = 0u64;
    let mut num = String::new();
    for ch in t.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num.push(ch);
        } else {
            let v: f64 = num.parse().unwrap_or(0.0);
            num.clear();
            match ch {
                'H' => min += (v * 60.0) as u64,
                'M' => min += v as u64,
                _ => {}
            }
        }
    }
    if min > 0 { Some(min) } else { None }
}

// =================================================================== registro
fn chave(raiz: &RegKey, caminho: &str, wow32: bool) -> Option<RegKey> {
    let flag = KEY_READ | if wow32 { KEY_WOW64_32KEY } else { KEY_WOW64_64KEY };
    raiz.open_subkey_with_flags(caminho, flag).ok()
}

fn valor_str(k: &RegKey, nome: &str) -> Option<String> {
    k.get_value::<String, _>(nome).ok()
}

fn valor_u32(k: &RegKey, nome: &str) -> Option<u32> {
    k.get_value::<u32, _>(nome).ok()
}

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUNONCE: &str = r"Software\Microsoft\Windows\CurrentVersion\RunOnce";
const APROV: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

fn chave_aprovacao(origem: &str) -> (RegKey, String) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    match origem {
        "HKCU" => (hkcu, format!("{APROV}\\Run")),
        "HKLM" => (hklm, format!("{APROV}\\Run")),
        "HKLM32" => (hklm, format!("{APROV}\\Run32")),
        "PastaUsuario" => (hkcu, format!("{APROV}\\StartupFolder")),
        _ => (hklm, format!("{APROV}\\StartupFolder")),
    }
}

fn aprovado(origem: &str, nome: &str) -> bool {
    let (raiz, cam) = chave_aprovacao(origem);
    if let Some(k) = chave(&raiz, &cam, false) {
        if let Ok(v) = k.get_raw_value(nome) {
            if let Some(b) = v.bytes.first() {
                return b & 1 == 0; // 02/06 = ativo, 03/07 = desativado
            }
        }
    }
    true
}

pub fn listar_inicializacao() -> Vec<EntradaInicio> {
    let mut v = vec![];
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let fontes: [(&RegKey, &str, bool, &str); 5] = [
        (&hkcu, RUN, false, "HKCU"),
        (&hklm, RUN, false, "HKLM"),
        (&hklm, RUN, true, "HKLM32"),
        (&hkcu, RUNONCE, false, "HKCU-RunOnce"),
        (&hklm, RUNONCE, false, "HKLM-RunOnce"),
    ];
    for (raiz, cam, wow, origem) in fontes {
        if let Some(k) = chave(raiz, cam, wow) {
            for (nome, _) in k.enum_values().flatten() {
                if let Some(cmdline) = valor_str(&k, &nome) {
                    if nome.starts_with("WinOpti") {
                        continue;
                    }
                    let ap = if origem.contains("RunOnce") { true } else { aprovado(origem, &nome) };
                    v.push(EntradaInicio { origem: origem.into(), nome, comando: cmdline, ativo: ap });
                }
            }
        }
    }
    let pastas = [
        (std::env::var("APPDATA").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs\Startup")), "PastaUsuario"),
        (std::env::var("ProgramData").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs\StartUp")), "PastaComum"),
    ];
    for (p, origem) in pastas {
        if let Ok(p) = p {
            if let Ok(rd) = std::fs::read_dir(&p) {
                for e in rd.flatten() {
                    let nome = e.file_name().to_string_lossy().into_owned();
                    if nome.eq_ignore_ascii_case("desktop.ini") {
                        continue;
                    }
                    let ap = aprovado(origem, &nome);
                    v.push(EntradaInicio { origem: origem.into(), nome, comando: e.path().to_string_lossy().into_owned(), ativo: ap });
                }
            }
        }
    }
    v
}

/// Desativa/reativa como o Gerenciador de Tarefas faz (StartupApproved). Reversivel.
pub fn definir_inicio(e: &EntradaInicio, ativo: bool) -> bool {
    let (raiz, cam) = chave_aprovacao(&e.origem);
    let k = match raiz.create_subkey_with_flags(&cam, KEY_WRITE | KEY_WOW64_64KEY) {
        Ok((k, _)) => k,
        Err(_) => return false,
    };
    let mut bytes = vec![if ativo { 2u8 } else { 3u8 }, 0, 0, 0];
    if !ativo {
        // horario (FILETIME) da desativacao, como o Windows grava
        let ft = (crate::sistema::agora() + 11_644_473_600) * 10_000_000;
        bytes.extend_from_slice(&ft.to_le_bytes());
    } else {
        bytes.extend_from_slice(&[0u8; 8]);
    }
    k.set_raw_value(&e.nome, &winreg::RegValue { bytes, vtype: REG_BINARY }).is_ok()
}

// =================================================================== tarefas agendadas
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Tarefa {
    pub caminho: String,
    pub comando: String,
    pub argumentos: String,
    pub oculta: bool,
    pub ativa: bool,
    pub ao_logon: bool,
    pub ao_iniciar: bool,
    pub repeticao: Option<String>,
    pub autor: String,
}

fn ler_tarefas() -> Vec<Tarefa> {
    let raiz = PathBuf::from(win_dir()).join("System32\\Tasks");
    let mut v = vec![];
    let mut pilha = vec![raiz.clone()];
    while let Some(d) = pilha.pop() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    pilha.push(p);
                    continue;
                }
                let bytes = match std::fs::read(&p) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                let xml = decodificar(&bytes);
                let rel = p.strip_prefix(&raiz).map(|r| format!("\\{}", r.to_string_lossy())).unwrap_or_default();
                let exec = trecho(&xml, "<Exec>", "</Exec>").unwrap_or("");
                let comando = tag(exec, "Command").unwrap_or_default();
                if comando.is_empty() {
                    continue; // tarefas COM (sem executavel) sao do sistema
                }
                v.push(Tarefa {
                    caminho: rel,
                    comando: desescapar(&comando),
                    argumentos: desescapar(&tag(exec, "Arguments").unwrap_or_default()),
                    oculta: tag(&xml, "Hidden").map(|h| h == "true").unwrap_or(false),
                    ativa: trecho(&xml, "<Settings>", "</Settings>").and_then(|s| tag(s, "Enabled")).map(|h| h != "false").unwrap_or(true),
                    ao_logon: xml.contains("<LogonTrigger"),
                    ao_iniciar: xml.contains("<BootTrigger"),
                    repeticao: trecho(&xml, "<Repetition>", "</Repetition>").and_then(|r| tag(r, "Interval")),
                    autor: tag(&xml, "Author").unwrap_or_default(),
                });
            }
        }
    }
    v
}

fn desescapar(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&apos;", "'")
}

pub fn definir_tarefa(caminho: &str, ativa: bool) -> bool {
    cmd("schtasks", &["/change", "/tn", caminho, if ativa { "/enable" } else { "/disable" }]).0
}

fn comando_suspeito(c: &str) -> Option<&'static str> {
    let l = c.to_lowercase();
    if l.contains("powershell") || l.contains("pwsh") {
        if l.contains("-enc") || l.contains(" -e ") || l.contains("frombase64") {
            return Some("PowerShell com comando codificado (técnica clássica de malware)");
        }
        if l.contains("downloadstring") || l.contains("downloadfile") || l.contains("invoke-webrequest") || l.contains("iwr ") || l.contains("iex") {
            return Some("PowerShell baixando e executando código da internet");
        }
        if l.contains("hidden") {
            return Some("PowerShell executando com janela oculta");
        }
    }
    if l.contains("mshta") {
        return Some("mshta executa scripts HTML/VBScript (muito usado por malware)");
    }
    if (l.contains("wscript") || l.contains("cscript")) && (l.contains(".vbs") || l.contains(".js") || l.contains(".jse")) {
        return Some("Script VBS/JS executado em segundo plano");
    }
    if l.contains("regsvr32") && (l.contains("/i:http") || l.contains("scrobj")) {
        return Some("regsvr32 carregando script remoto (técnica Squiblydoo)");
    }
    if l.contains("certutil") && (l.contains("-urlcache") || l.contains("-decode")) {
        return Some("certutil usado para baixar/decodificar arquivos");
    }
    if l.contains("bitsadmin") && l.contains("/transfer") {
        return Some("bitsadmin baixando arquivos");
    }
    if l.contains("rundll32") && (l.contains("\\appdata\\") || l.contains("\\temp\\") || l.contains("javascript:")) {
        return Some("rundll32 carregando DLL de pasta do usuário");
    }
    if l.contains("http://") || l.contains("https://") {
        if l.contains("cmd") || l.contains("powershell") || l.contains("curl") {
            return Some("Comando acessando endereço da internet");
        }
    }
    None
}

// =================================================================== servicos
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Servico {
    pub nome: String,
    pub exibicao: String,
    pub binario: String,
    pub inicio: u32,
    pub rodando: bool,
    pub pid: u32,
}

fn listar_servicos() -> Vec<Servico> {
    use windows_sys::Win32::System::Services::*;
    let mut v = vec![];
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ENUMERATE_SERVICE | SC_MANAGER_CONNECT);
        if scm.is_null() {
            return v;
        }
        let mut precisa: u32 = 0;
        let mut qtd: u32 = 0;
        let mut resume: u32 = 0;
        EnumServicesStatusExW(scm, SC_ENUM_PROCESS_INFO, SERVICE_WIN32, SERVICE_STATE_ALL, std::ptr::null_mut(), 0, &mut precisa, &mut qtd, &mut resume, std::ptr::null());
        let mut buf: Vec<u8> = vec![0; precisa as usize + 4096];
        resume = 0;
        if EnumServicesStatusExW(scm, SC_ENUM_PROCESS_INFO, SERVICE_WIN32, SERVICE_STATE_ALL, buf.as_mut_ptr(), buf.len() as u32, &mut precisa, &mut qtd, &mut resume, std::ptr::null()) == 0 {
            CloseServiceHandle(scm);
            return v;
        }
        let itens = buf.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW;
        for i in 0..qtd as usize {
            let it = &*itens.add(i);
            let nome = pwstr(it.lpServiceName);
            let exib = pwstr(it.lpDisplayName);
            let st = it.ServiceStatusProcess;
            let mut s = Servico {
                nome: nome.clone(),
                exibicao: exib,
                binario: String::new(),
                inicio: 3,
                rodando: st.dwCurrentState == SERVICE_RUNNING,
                pid: st.dwProcessId,
            };
            let wn = w(&nome);
            let h = OpenServiceW(scm, wn.as_ptr(), SERVICE_QUERY_CONFIG);
            if !h.is_null() {
                let mut need: u32 = 0;
                QueryServiceConfigW(h, std::ptr::null_mut(), 0, &mut need);
                if need > 0 {
                    let mut cb: Vec<u8> = vec![0; need as usize + 16];
                    let cfg = cb.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;
                    if QueryServiceConfigW(h, cfg, need, &mut need) != 0 {
                        s.binario = pwstr((*cfg).lpBinaryPathName);
                        s.inicio = (*cfg).dwStartType;
                    }
                }
                CloseServiceHandle(h);
            }
            v.push(s);
        }
        CloseServiceHandle(scm);
    }
    v
}

// =================================================================== varredura principal
fn etapa(sh: &Shared, s: &str) {
    *sh.etapa.lock().unwrap() = s.to_string();
}

fn protegido_nome(stem: &str) -> bool {
    PROTEGIDOS.contains(&stem) || stem == "system" || stem == "idle"
}

pub fn executar(sh: Arc<Shared>) {
    if sh.diagnosticando.swap(true, Ordering::SeqCst) {
        return;
    }
    let t0 = Instant::now();
    let mut d = Diagnostico::default();
    let mut a: Vec<Achado> = vec![];
    let wd = win_dir();
    let nucleos = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32;

    // ------------------------------------------------------------ processos (2 amostras)
    etapa(&sh, "Medindo CPU, memória e disco de cada processo...");
    let kind = ProcessRefreshKind::new()
        .with_cpu()
        .with_memory()
        .with_disk_usage()
        .with_exe(UpdateKind::OnlyIfNotSet)
        .with_cmd(UpdateKind::OnlyIfNotSet);
    let mut sys = System::new_with_specifics(RefreshKind::new());
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    let ta = Instant::now();
    etapa(&sh, "Medindo uso da placa de vídeo por processo...");
    let gpu = gpu_por_pid();
    let espera = Duration::from_millis(2500).saturating_sub(ta.elapsed());
    std::thread::sleep(espera);
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    let intervalo = ta.elapsed().as_secs_f64().max(0.5);
    sys.refresh_memory();
    d.total_proc = sys.processes().len();
    d.ram_total_gb = sys.total_memory() as f64 / 1073741824.0;
    d.ram_uso_pct = if sys.total_memory() > 0 {
        (sys.total_memory() - sys.available_memory()) as f64 / sys.total_memory() as f64 * 100.0
    } else {
        0.0
    };

    etapa(&sh, "Procurando mineradores, imitações e programas indesejados...");
    let mut agrupado: BTreeMap<String, ProcResumo> = BTreeMap::new();
    let mut ja_avisado: std::collections::HashSet<String> = Default::default();
    let mut pid_nome: HashMap<u32, (String, String)> = HashMap::new();
    for (pid, p) in sys.processes() {
        let nome = p.name().to_string_lossy().into_owned();
        let low = nome.to_lowercase();
        let stem = low.strip_suffix(".exe").unwrap_or(&low).to_string();
        let cam = p.exe().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        let lcam = cam.to_lowercase();
        let cmdl = p.cmd().iter().map(|c| c.to_string_lossy()).collect::<Vec<_>>().join(" ");
        let lcmd = cmdl.to_lowercase();
        let cpu = (p.cpu_usage() / nucleos).min(100.0);
        let du = p.disk_usage();
        let disco = (du.read_bytes + du.written_bytes) as f64 / intervalo / 1048576.0;
        let g = gpu.get(&pid.as_u32()).copied().unwrap_or(0.0);
        pid_nome.insert(pid.as_u32(), (nome.clone(), cam.clone()));

        let e = agrupado.entry(stem.clone()).or_insert(ProcResumo {
            nome: nome.clone(),
            stem: stem.clone(),
            qtd: 0,
            ram_mb: 0.0,
            cpu: 0.0,
            disco_mbs: 0.0,
            gpu: 0.0,
            caminho: cam.clone(),
            assinado: None,
        });
        e.qtd += 1;
        e.ram_mb += p.memory() as f64 / 1048576.0;
        e.cpu = (e.cpu + cpu).min(100.0);
        e.disco_mbs += disco;
        e.gpu = e.gpu.max(g);
        if e.caminho.is_empty() {
            e.caminho = cam.clone();
        }

        if !ja_avisado.insert(format!("{stem}|{lcam}")) {
            continue;
        }
        // mineradores
        if MINERADORES.iter().any(|m| stem.contains(m)) {
            a.push(Achado {
                grav: Gravidade::Critico,
                cat: Categoria::Mineracao,
                titulo: format!("Minerador detectado: {nome}"),
                detalhe: format!("PID {} · {}", pid, cam),
                porque: "Nome de software de mineração de criptomoedas. Usa CPU/GPU ao máximo e derruba o FPS.".into(),
                caminho: cam.clone(),
                acao: Acao::FecharProcesso(stem.clone()),
            });
        } else if CMD_MINERACAO.iter().any(|m| lcmd.contains(m)) {
            a.push(Achado {
                grav: Gravidade::Critico,
                cat: Categoria::Mineracao,
                titulo: format!("Linha de comando de mineração: {nome}"),
                detalhe: format!("PID {} · {}", pid, truncar(&cmdl, 180)),
                porque: "Parâmetros típicos de mineradores (pool stratum, algoritmo RandomX/CryptoNight).".into(),
                caminho: cam.clone(),
                acao: Acao::FecharProcesso(stem.clone()),
            });
        }
        // imitacoes de processos do sistema
        if NOMES_SISTEMA.contains(&stem.as_str()) && !lcam.is_empty() && !lcam.starts_with(&format!("{wd}\\")) {
            a.push(Achado {
                grav: Gravidade::Critico,
                cat: Categoria::Mineracao,
                titulo: format!("Processo imitando o Windows: {nome}"),
                detalhe: cam.clone(),
                porque: "Tem nome de processo do sistema, mas roda fora da pasta do Windows. Técnica comum de mineradores e trojans.".into(),
                caminho: cam.clone(),
                acao: Acao::FecharProcesso(stem.clone()),
            });
        }
        if IMITACOES.contains(&stem.as_str()) {
            a.push(Achado {
                grav: Gravidade::Critico,
                cat: Categoria::Mineracao,
                titulo: format!("Nome falsificado: {nome}"),
                detalhe: cam.clone(),
                porque: "Nome parecido com um processo do Windows, com erro de digitação proposital para enganar.".into(),
                caminho: cam.clone(),
                acao: Acao::FecharProcesso(stem.clone()),
            });
        }
        // GPU alta fora de lugares confiaveis = suspeita de minerador de GPU
        if g >= 25.0 && !protegido_nome(&stem) && !lcam.is_empty() && (pasta_gravavel(&cam) || assinado(&cam) == Some(false)) {
            a.push(Achado {
                grav: Gravidade::Alto,
                cat: Categoria::Mineracao,
                titulo: format!("Uso de GPU suspeito: {nome} ({g:.0}%)"),
                detalhe: cam.clone(),
                porque: "Programa sem assinatura ou em pasta do usuário usando a placa de vídeo em segundo plano. Se não é um jogo seu, pode ser minerador.".into(),
                caminho: cam.clone(),
                acao: Acao::FecharProcesso(stem.clone()),
            });
        }
        // executavel rodando de pasta temporaria
        if pasta_temporaria(&cam) && lcam.ends_with(".exe") {
            let ass = assinado(&cam);
            a.push(Achado {
                grav: if ass == Some(false) { Gravidade::Alto } else { Gravidade::Baixo },
                cat: Categoria::Mineracao,
                titulo: format!("Rodando de pasta temporária: {nome}"),
                detalhe: format!("{cam} · {}", if ass == Some(false) { "sem assinatura digital" } else { "assinado" }),
                porque: "Instaladores rodam daqui normalmente, mas programas que ficam residentes em Temp são suspeitos.".into(),
                caminho: cam.clone(),
                acao: Acao::FecharProcesso(stem.clone()),
            });
        }
        // programas conhecidos (adware, pesados, problemáticos)
        if let Some(k) = buscar(&stem, 'P') {
            a.push(Achado {
                grav: k.grav,
                cat: if ["adware", "PUP", "pirata", "scareware"].iter().any(|t| k.titulo.contains(t)) {
                    Categoria::Seguranca
                } else {
                    Categoria::Desempenho
                },
                titulo: k.titulo.to_string(),
                detalhe: format!("{nome} · {cam}"),
                porque: k.porque.to_string(),
                caminho: cam.clone(),
                acao: Acao::FecharProcesso(stem.clone()),
            });
        }
        if stem == "chromium" && lcam.contains("\\appdata\\") {
            a.push(Achado {
                grav: Gravidade::Alto,
                cat: Categoria::Seguranca,
                titulo: "Chromium instalado na pasta do usuário".into(),
                detalhe: cam.clone(),
                porque: "Vários adwares se instalam como \"Chromium\" no AppData para sequestrar o navegador.".into(),
                caminho: cam.clone(),
                acao: Acao::FecharProcesso(stem.clone()),
            });
        }
    }

    // consumo agregado
    etapa(&sh, "Verificando assinaturas digitais...");
    let mut top: Vec<ProcResumo> = agrupado.into_values().collect();
    for p in top.iter_mut() {
        if !p.caminho.is_empty() && (p.ram_mb > 150.0 || p.cpu > 2.0 || p.gpu > 5.0) {
            p.assinado = assinado(&p.caminho);
        }
    }
    for p in &top {
        if protegido_nome(&p.stem) || p.stem == "winopti" {
            continue;
        }
        let local_win = p.caminho.to_lowercase().starts_with(&format!("{wd}\\"));
        if p.cpu >= 15.0 {
            a.push(Achado {
                grav: if p.cpu >= 40.0 { Gravidade::Alto } else { Gravidade::Medio },
                cat: Categoria::Desempenho,
                titulo: format!("CPU alta em repouso: {} ({:.0}%)", p.nome, p.cpu),
                detalhe: p.caminho.clone(),
                porque: "Consumo contínuo de processador sem você estar usando o programa. Causa queda de FPS e aquecimento.".into(),
                caminho: p.caminho.clone(),
                acao: if local_win { Acao::Nenhuma } else { Acao::FecharProcesso(p.stem.clone()) },
            });
        }
        if p.ram_mb >= 1500.0 {
            a.push(Achado {
                grav: if p.ram_mb >= 3000.0 { Gravidade::Alto } else { Gravidade::Medio },
                cat: Categoria::Desempenho,
                titulo: format!("Memória excessiva: {} ({:.1} GB)", p.nome, p.ram_mb / 1024.0),
                detalhe: format!("{} processo(s) · {}", p.qtd, p.caminho),
                porque: "Programas que acumulam memória deixam o Windows usar o disco como RAM (paginação) e causam travadas.".into(),
                caminho: p.caminho.clone(),
                acao: if local_win { Acao::Nenhuma } else { Acao::FecharProcesso(p.stem.clone()) },
            });
        }
        if p.disco_mbs >= 8.0 {
            a.push(Achado {
                grav: Gravidade::Medio,
                cat: Categoria::Desempenho,
                titulo: format!("Disco intenso: {} ({:.1} MB/s)", p.nome, p.disco_mbs),
                detalhe: p.caminho.clone(),
                porque: "Leitura/gravação contínua deixa o sistema lento e causa travadas em jogos (carregamento de texturas).".into(),
                caminho: p.caminho.clone(),
                acao: if local_win { Acao::Nenhuma } else { Acao::FecharProcesso(p.stem.clone()) },
            });
        }
        if p.assinado == Some(false) && !p.caminho.is_empty() && (p.ram_mb > 150.0 || p.cpu > 2.0) && pasta_gravavel(&p.caminho) {
            a.push(Achado {
                grav: Gravidade::Medio,
                cat: Categoria::Seguranca,
                titulo: format!("Programa sem assinatura em pasta do usuário: {}", p.nome),
                detalhe: p.caminho.clone(),
                porque: "Não tem assinatura digital de um fabricante e roda de uma pasta onde qualquer programa pode gravar. Confirme se você reconhece.".into(),
                caminho: p.caminho.clone(),
                acao: Acao::AbrirLocal(p.caminho.clone()),
            });
        }
    }
    if d.ram_uso_pct >= 85.0 {
        a.push(Achado {
            grav: Gravidade::Alto,
            cat: Categoria::Desempenho,
            titulo: format!("Memória quase cheia: {:.0}% em uso", d.ram_uso_pct),
            detalhe: format!("{:.1} GB no total", d.ram_total_gb),
            porque: "Com a RAM cheia o Windows passa a usar o disco, o que causa travamentos.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    top.sort_by(|x, y| {
        let px = x.ram_mb + x.cpu as f64 * 60.0 + x.gpu as f64 * 30.0 + x.disco_mbs * 40.0;
        let py = y.ram_mb + y.cpu as f64 * 60.0 + y.gpu as f64 * 30.0 + y.disco_mbs * 40.0;
        py.partial_cmp(&px).unwrap_or(std::cmp::Ordering::Equal)
    });
    top.retain(|p| !p.caminho.is_empty() || p.ram_mb > 200.0);
    top.truncate(25);
    d.top = top;

    // ------------------------------------------------------------ rede
    etapa(&sh, "Analisando conexões de rede...");
    let mut por_pid: HashMap<u32, usize> = HashMap::new();
    for c in conexoes_tcp() {
        *por_pid.entry(c.pid).or_default() += 1;
        if PORTAS_MINERACAO.contains(&c.porta) {
            let (n, cam) = pid_nome.get(&c.pid).cloned().unwrap_or_default();
            a.push(Achado {
                grav: Gravidade::Critico,
                cat: Categoria::Mineracao,
                titulo: format!("Conexão em porta de pool de mineração: {n}"),
                detalhe: format!("{}:{} · PID {} · {}", c.remoto, c.porta, c.pid, cam),
                porque: "Essa porta é usada por pools de mineração (protocolo stratum).".into(),
                caminho: cam.clone(),
                acao: Acao::FecharProcesso(n.to_lowercase().trim_end_matches(".exe").to_string()),
            });
        }
    }
    for (pid, n) in por_pid {
        if n >= 60 {
            let (nome, cam) = pid_nome.get(&pid).cloned().unwrap_or_default();
            let stem = nome.to_lowercase().trim_end_matches(".exe").to_string();
            if ["msedge", "chrome", "firefox", "brave", "opera", "steam", "steamwebhelper", "svchost"].contains(&stem.as_str()) {
                continue;
            }
            a.push(Achado {
                grav: Gravidade::Medio,
                cat: Categoria::Rede,
                titulo: format!("Muitas conexões abertas: {nome} ({n})"),
                detalhe: cam.clone(),
                porque: "Grande número de conexões pode ser torrent, sincronização pesada ou malware. Aumenta a latência (ping) em jogos online.".into(),
                caminho: cam.clone(),
                acao: Acao::FecharProcesso(stem),
            });
        }
    }

    // ------------------------------------------------------------ inicializacao
    etapa(&sh, "Mapeando programas que iniciam com o Windows...");
    let inicio = listar_inicializacao();
    for e in &inicio {
        let exe = exe_de(&e.comando);
        let lexe = exe.to_lowercase();
        let existe = exe.is_empty() || Path::new(&exe).exists() || !exe.contains('\\');
        let ass = if lexe.ends_with(".exe") { assinado(&exe) } else { None };
        let base = Path::new(&exe).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        let script = [".vbs", ".js", ".bat", ".cmd", ".ps1", ".hta"].iter().any(|x| lexe.ends_with(x)) || e.comando.to_lowercase().contains("mshta");
        let (grav, porque) = if let Some(s) = comando_suspeito(&e.comando) {
            (Gravidade::Critico, s.to_string())
        } else if let Some(k) = buscar(&base, 'P') {
            (k.grav, k.porque.to_string())
        } else if script {
            (Gravidade::Alto, "Script executado de forma invisível a cada login. Malware usa muito esse truque.".into())
        } else if ass == Some(false) && pasta_gravavel(&exe) {
            (Gravidade::Alto, "Sem assinatura digital e instalado em pasta do usuário. Confirme se você reconhece este programa.".into())
        } else if !existe {
            (Gravidade::Baixo, "O arquivo não existe mais: entrada quebrada que sobrou de um programa desinstalado.".into())
        } else if ass == Some(false) {
            (Gravidade::Baixo, "Programa sem assinatura digital iniciando junto com o Windows.".into())
        } else {
            (Gravidade::Info, "Inicia sozinho, em segundo plano, a cada login. Cada item aumenta o tempo de boot e o uso de memória.".into())
        };
        if !e.ativo && grav >= Gravidade::Medio {
            continue; // ja desativado e sem risco
        }
        a.push(Achado {
            grav: if e.ativo { grav } else { Gravidade::Info },
            cat: Categoria::Inicializacao,
            titulo: format!("{}{}", e.nome, if e.ativo { "" } else { " (desativado)" }),
            detalhe: format!("{} · {}", rotulo_origem(&e.origem), truncar(&e.comando, 160)),
            porque,
            caminho: exe.clone(),
            acao: if e.ativo && !e.origem.contains("RunOnce") { Acao::DesativarInicio(e.clone()) } else { Acao::AbrirLocal(exe.clone()) },
        });
    }
    d.inicio = inicio;

    // ------------------------------------------------------------ tarefas agendadas
    etapa(&sh, "Lendo tarefas agendadas (inclusive ocultas)...");
    let tarefas = ler_tarefas();
    d.total_tarefas = tarefas.len();
    for t in &tarefas {
        if !t.ativa {
            continue;
        }
        let microsoft = t.caminho.to_lowercase().starts_with("\\microsoft\\");
        let exe = exe_de(&t.comando);
        let lexe = exe.to_lowercase();
        let completo = format!("{} {}", t.comando, t.argumentos);
        let fora_windows = !lexe.is_empty() && !lexe.starts_with(&wd) && !lexe.starts_with("%windir%") && !lexe.starts_with("%systemroot%") && exe.contains('\\');
        let mut grav = None;
        let mut porque = String::new();
        if let Some(s) = comando_suspeito(&completo) {
            grav = Some(Gravidade::Critico);
            porque = s.to_string();
        } else if microsoft && fora_windows && !lexe.contains("\\program files") {
            grav = Some(Gravidade::Alto);
            porque = "Tarefa escondida na pasta da Microsoft, mas executa um programa de fora do Windows. Malware costuma se camuflar assim.".into();
        } else if !microsoft {
            let ass = if lexe.ends_with(".exe") { assinado(&exe) } else { None };
            if ass == Some(false) && pasta_gravavel(&exe) {
                grav = Some(Gravidade::Alto);
                porque = "Programa sem assinatura em pasta do usuário, executado automaticamente.".into();
            } else if t.oculta {
                grav = Some(Gravidade::Medio);
                porque = "Tarefa marcada como OCULTA: não aparece no Agendador de Tarefas por padrão.".into();
            } else if let Some(r) = &t.repeticao {
                if let Some(min) = duracao_iso(r).filter(|m| *m <= 15) {
                    grav = Some(Gravidade::Medio);
                    porque = format!("Executa a cada {min} minuto(s) em segundo plano.");
                }
            }
            if grav.is_none() {
                grav = Some(Gravidade::Info);
                porque = "Tarefa de terceiros (normalmente atualizadores).".into();
            }
        }
        if let Some(g) = grav {
            let mut gatilhos = vec![];
            if t.ao_logon {
                gatilhos.push("ao fazer login");
            }
            if t.ao_iniciar {
                gatilhos.push("ao ligar o PC");
            }
            if t.oculta {
                gatilhos.push("oculta");
            }
            a.push(Achado {
                grav: g,
                cat: Categoria::Tarefas,
                titulo: t.caminho.clone(),
                detalhe: format!("{}{}", truncar(&completo, 170), if gatilhos.is_empty() { String::new() } else { format!(" · {}", gatilhos.join(", ")) }),
                porque,
                caminho: exe.clone(),
                acao: Acao::DesativarTarefa(t.caminho.clone()),
            });
        }
    }

    // ------------------------------------------------------------ servicos
    etapa(&sh, "Analisando todos os serviços...");
    let servicos = listar_servicos();
    d.total_servicos = servicos.len();
    let mut fornecedores: Vec<&str> = vec![];
    for (fornecedor, pads) in ANTIVIRUS {
        let ativo = servicos.iter().any(|s| s.rodando && pads.iter().any(|p| casa_padrao(&s.nome, p)))
            || sys.processes().values().any(|p| {
                let n = p.name().to_string_lossy().to_lowercase();
                let n = n.trim_end_matches(".exe");
                pads.iter().any(|pd| casa_padrao(n, pd))
            });
        if ativo {
            fornecedores.push(fornecedor);
        }
    }
    if fornecedores.len() > 1 {
        a.push(Achado {
            grav: Gravidade::Critico,
            cat: Categoria::Seguranca,
            titulo: format!("Mais de um antivírus ativo: {}", fornecedores.join(" + ")),
            detalhe: "Dois antivírus com proteção em tempo real ao mesmo tempo.".into(),
            porque: "Eles verificam os mesmos arquivos em dobro e brigam entre si: lentidão, travamentos e falsos alertas. Mantenha só um (desinstale o outro).".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    for s in &servicos {
        let exe = exe_de(&s.binario);
        let lexe = exe.to_lowercase();
        let windows = lexe.starts_with(&wd) || lexe.is_empty();
        let auto = s.inicio == 2;
        // vulnerabilidade: caminho sem aspas com espacos
        let bin = s.binario.trim();
        if !bin.starts_with('"') && !windows && bin.contains(' ') {
            if let Some(i) = bin.to_lowercase().find(".exe") {
                if bin[..i].contains(' ') {
                    a.push(Achado {
                        grav: Gravidade::Medio,
                        cat: Categoria::Seguranca,
                        titulo: format!("Serviço com caminho sem aspas: {}", s.exibicao),
                        detalhe: s.binario.clone(),
                        porque: "Caminho com espaços sem aspas permite que um programa malicioso seja executado no lugar do serviço (Unquoted Service Path).".into(),
                        caminho: exe.clone(),
                        acao: Acao::AbrirLocal(exe.clone()),
                    });
                }
            }
        }
        if let Some(k) = buscar(&s.nome, 'S') {
            if s.rodando || auto {
                a.push(Achado {
                    grav: if s.rodando { k.grav } else { k.grav.max(Gravidade::Baixo) },
                    cat: if k.titulo.contains("remoto") || k.titulo.contains("Remoto") || k.titulo.contains("SSH") || k.titulo.contains("Telnet") || k.titulo.contains("VNC") { Categoria::Seguranca } else { Categoria::Servicos },
                    titulo: k.titulo.to_string(),
                    detalhe: format!("{} · {} · {}", s.nome, if s.rodando { "rodando" } else { "parado" }, inicio_txt(s.inicio)),
                    porque: k.porque.to_string(),
                    caminho: exe.clone(),
                    acao: if s.rodando { Acao::PararServico(s.nome.clone()) } else { Acao::Nenhuma },
                });
            }
            continue;
        }
        if windows || !(s.rodando || auto) {
            continue;
        }
        let existe = Path::new(&exe).exists();
        let ass = assinado(&exe);
        if !existe {
            a.push(Achado {
                grav: Gravidade::Baixo,
                cat: Categoria::Servicos,
                titulo: format!("Serviço quebrado: {}", s.exibicao),
                detalhe: s.binario.clone(),
                porque: "Configurado para iniciar, mas o arquivo não existe mais (sobra de desinstalação). Gera erros e atraso no boot.".into(),
                caminho: exe.clone(),
                acao: Acao::Nenhuma,
            });
        } else if pasta_gravavel(&exe) {
            a.push(Achado {
                grav: if ass == Some(false) { Gravidade::Critico } else { Gravidade::Alto },
                cat: Categoria::Mineracao,
                titulo: format!("Serviço rodando de pasta do usuário: {}", s.exibicao),
                detalhe: format!("{} · {}", s.nome, exe),
                porque: "Serviços legítimos ficam em Arquivos de Programas. Serviço em AppData/Temp/ProgramData é típico de minerador e malware persistente.".into(),
                caminho: exe.clone(),
                acao: if s.rodando { Acao::PararServico(s.nome.clone()) } else { Acao::AbrirLocal(exe.clone()) },
            });
        } else if ass == Some(false) {
            a.push(Achado {
                grav: Gravidade::Baixo,
                cat: Categoria::Servicos,
                titulo: format!("Serviço sem assinatura: {}", s.exibicao),
                detalhe: format!("{} · {} · {}", s.nome, if s.rodando { "rodando" } else { "parado" }, exe),
                porque: "Sem assinatura digital. Comum em sistemas comerciais brasileiros (ERP/PDV), mas confirme se você reconhece.".into(),
                caminho: exe.clone(),
                acao: if s.rodando { Acao::PararServico(s.nome.clone()) } else { Acao::Nenhuma },
            });
        } else if s.rodando {
            a.push(Achado {
                grav: Gravidade::Info,
                cat: Categoria::Servicos,
                titulo: format!("Serviço de terceiros: {}", s.exibicao),
                detalhe: format!("{} · {}", s.nome, exe),
                porque: "Serviço de outro fabricante rodando em segundo plano.".into(),
                caminho: exe.clone(),
                acao: Acao::PararServico(s.nome.clone()),
            });
        }
    }

    // ------------------------------------------------------------ sequestros do sistema
    etapa(&sh, "Verificando sequestros do sistema (Winlogon, IFEO, proxy, hosts)...");
    verificar_sequestros(&mut a);

    // ------------------------------------------------------------ configuracao do Windows
    etapa(&sh, "Verificando configurações que afetam o desempenho...");
    verificar_sistema(&mut a);

    // ------------------------------------------------------------ bateria
    etapa(&sh, "Lendo saúde da bateria...");
    d.bateria = ler_bateria();
    verificar_bateria(&d.bateria, &mut a);

    // ------------------------------------------------------------ nota
    a.sort_by(|x, y| x.grav.cmp(&y.grav).then(x.cat.cmp(&y.cat)));
    let mut pen = 0.0;
    let mut por_cat: HashMap<Categoria, f32> = HashMap::new();
    for x in &a {
        let e = por_cat.entry(x.cat).or_default();
        *e += x.grav.peso();
    }
    for (_, v) in por_cat {
        pen += v.min(30.0);
    }
    d.nota = (100.0 - pen).clamp(0.0, 100.0) as u8;
    d.achados = a;
    d.segundos = t0.elapsed().as_secs_f32();
    etapa(&sh, "");
    *sh.diag.lock().unwrap() = Some(d);
    sh.diagnosticando.store(false, Ordering::SeqCst);
}

fn rotulo_origem(o: &str) -> &'static str {
    match o {
        "HKCU" => "Registro do usuário (Run)",
        "HKLM" => "Registro da máquina (Run)",
        "HKLM32" => "Registro da máquina 32 bits (Run)",
        "HKCU-RunOnce" => "Executar uma vez (usuário)",
        "HKLM-RunOnce" => "Executar uma vez (máquina)",
        "PastaUsuario" => "Pasta Inicializar do usuário",
        _ => "Pasta Inicializar de todos",
    }
}

fn inicio_txt(t: u32) -> &'static str {
    match t {
        0 => "boot",
        1 => "sistema",
        2 => "automático",
        3 => "manual",
        4 => "desativado",
        _ => "?",
    }
}

pub fn truncar(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

fn verificar_sequestros(a: &mut Vec<Achado>) {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    // Winlogon
    if let Some(k) = chave(&hklm, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon", false) {
        if let Some(sh) = valor_str(&k, "Shell") {
            if !sh.trim().eq_ignore_ascii_case("explorer.exe") {
                a.push(Achado {
                    grav: Gravidade::Critico,
                    cat: Categoria::Seguranca,
                    titulo: "Shell do Windows alterado (Winlogon)".into(),
                    detalhe: format!("Shell = {sh}"),
                    porque: "O valor deveria ser apenas explorer.exe. Malware altera para iniciar junto com a área de trabalho.".into(),
                    caminho: String::new(),
                    acao: Acao::Nenhuma,
                });
            }
        }
        if let Some(ui) = valor_str(&k, "Userinit") {
            let partes: Vec<&str> = ui.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
            if partes.len() != 1 || !partes[0].to_lowercase().ends_with("userinit.exe") {
                a.push(Achado {
                    grav: Gravidade::Critico,
                    cat: Categoria::Seguranca,
                    titulo: "Userinit alterado (Winlogon)".into(),
                    detalhe: format!("Userinit = {ui}"),
                    porque: "Deveria conter só userinit.exe. Programas extras aqui rodam antes da área de trabalho.".into(),
                    caminho: String::new(),
                    acao: Acao::Nenhuma,
                });
            }
        }
    }
    // IFEO debugger
    if let Some(k) = chave(&hklm, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options", false) {
        for sub in k.enum_keys().flatten() {
            if let Ok(sk) = k.open_subkey(&sub) {
                if let Some(dbg) = valor_str(&sk, "Debugger") {
                    if dbg.to_lowercase().contains("vsjitdebugger") {
                        continue;
                    }
                    a.push(Achado {
                        grav: Gravidade::Alto,
                        cat: Categoria::Seguranca,
                        titulo: format!("Execução de {sub} redirecionada (IFEO)"),
                        detalhe: format!("Debugger = {dbg}"),
                        porque: "Ao abrir esse programa o Windows executa outro no lugar. Usado por malware para bloquear antivírus/Gerenciador de Tarefas ou se manter ativo.".into(),
                        caminho: String::new(),
                        acao: Acao::Nenhuma,
                    });
                }
            }
        }
    }
    // AppInit_DLLs
    if let Some(k) = chave(&hklm, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Windows", false) {
        let dlls = valor_str(&k, "AppInit_DLLs").unwrap_or_default();
        if valor_u32(&k, "LoadAppInit_DLLs") == Some(1) && !dlls.trim().is_empty() {
            a.push(Achado {
                grav: Gravidade::Alto,
                cat: Categoria::Seguranca,
                titulo: "AppInit_DLLs ativo".into(),
                detalhe: dlls,
                porque: "DLLs injetadas em todos os programas que usam interface. Técnica antiga de spyware/adware.".into(),
                caminho: String::new(),
                acao: Acao::Nenhuma,
            });
        }
    }
    // Proxy
    if let Some(k) = chave(&hkcu, r"Software\Microsoft\Windows\CurrentVersion\Internet Settings", false) {
        if valor_u32(&k, "ProxyEnable") == Some(1) {
            let srv = valor_str(&k, "ProxyServer").unwrap_or_default();
            a.push(Achado {
                grav: Gravidade::Medio,
                cat: Categoria::Seguranca,
                titulo: "Proxy de internet configurado".into(),
                detalhe: srv,
                porque: "Todo o tráfego passa por esse servidor. Se você não configurou (empresa/VPN), pode ser adware interceptando a navegação.".into(),
                caminho: String::new(),
                acao: Acao::Nenhuma,
            });
        }
        if let Some(pac) = valor_str(&k, "AutoConfigURL").filter(|s| !s.is_empty()) {
            a.push(Achado {
                grav: Gravidade::Medio,
                cat: Categoria::Seguranca,
                titulo: "Script automático de proxy (PAC)".into(),
                detalhe: pac,
                porque: "Redireciona a navegação por regras externas. Confirme se foi a sua empresa que configurou.".into(),
                caminho: String::new(),
                acao: Acao::Nenhuma,
            });
        }
    }
    // Exclusoes do Defender
    for (sub, rot) in [("Paths", "pastas"), ("Processes", "processos"), ("Extensions", "extensões")] {
        if let Some(k) = chave(&hklm, &format!(r"SOFTWARE\Microsoft\Windows Defender\Exclusions\{sub}"), false) {
            let itens: Vec<String> = k.enum_values().flatten().map(|(n, _)| n).collect();
            if !itens.is_empty() {
                let perigoso = itens.iter().any(|i| {
                    let l = i.to_lowercase();
                    l.len() <= 3 || l.contains("\\appdata") || l.contains("\\temp") || l.contains("\\users\\") || l == ".exe" || l == "exe"
                });
                a.push(Achado {
                    grav: if perigoso { Gravidade::Alto } else { Gravidade::Baixo },
                    cat: Categoria::Seguranca,
                    titulo: format!("Exclusões no Windows Defender ({rot})"),
                    detalhe: itens.join(" ; "),
                    porque: "Itens excluídos não são verificados. Mineradores se adicionam aqui para não serem detectados.".into(),
                    caminho: String::new(),
                    acao: Acao::Nenhuma,
                });
            }
        }
    }
    // hosts
    let hosts = PathBuf::from(win_dir()).join("System32\\drivers\\etc\\hosts");
    if let Ok(h) = std::fs::read(&hosts) {
        let h = decodificar(&h);
        let linhas: Vec<&str> = h.lines().map(|l| l.trim()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
        let seg: Vec<&str> = linhas
            .iter()
            .copied()
            .filter(|l| ["kaspersky", "microsoft", "windowsupdate", "eset", "avast", "malwarebytes", "virustotal", "norton", "mcafee", "bitdefender"].iter().any(|k| l.to_lowercase().contains(k)))
            .collect();
        if !seg.is_empty() {
            a.push(Achado {
                grav: Gravidade::Critico,
                cat: Categoria::Seguranca,
                titulo: "Arquivo hosts bloqueando sites de segurança".into(),
                detalhe: truncar(&seg.join(" ; "), 200),
                porque: "Impede antivírus e Windows Update de se atualizarem. Sinal clássico de malware.".into(),
                caminho: hosts.to_string_lossy().into_owned(),
                acao: Acao::AbrirLocal(hosts.to_string_lossy().into_owned()),
            });
        } else if linhas.len() > 20 {
            a.push(Achado {
                grav: Gravidade::Baixo,
                cat: Categoria::Seguranca,
                titulo: format!("Arquivo hosts com {} entradas", linhas.len()),
                detalhe: truncar(&linhas[..linhas.len().min(5)].join(" ; "), 180),
                porque: "Pode ser um bloqueador de anúncios, mas confira se você reconhece.".into(),
                caminho: hosts.to_string_lossy().into_owned(),
                acao: Acao::AbrirLocal(hosts.to_string_lossy().into_owned()),
            });
        }
    }
}

fn verificar_sistema(a: &mut Vec<Achado>) {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    // tempo ligado
    let dias = System::uptime() / 86400;
    let rapida = chave(&hklm, r"SYSTEM\CurrentControlSet\Control\Session Manager\Power", false).and_then(|k| valor_u32(&k, "HiberbootEnabled")) == Some(1);
    if dias >= 3 {
        a.push(Achado {
            grav: if dias >= 10 { Gravidade::Alto } else { Gravidade::Medio },
            cat: Categoria::Sistema,
            titulo: format!("Windows sem reiniciar há {dias} dias"),
            detalhe: if rapida { "Inicialização Rápida ligada: \"Desligar\" NÃO reinicia o Windows de verdade.".into() } else { String::new() },
            porque: "Vazamentos de memória de drivers e programas se acumulam. Reiniciar limpa a memória e conclui atualizações.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    if rapida {
        a.push(Achado {
            grav: Gravidade::Baixo,
            cat: Categoria::Sistema,
            titulo: "Inicialização Rápida ligada".into(),
            detalhe: "Painel de Controle > Opções de Energia > Escolher a função dos botões de energia".into(),
            porque: "Ao desligar, o Windows hiberna o sistema em vez de encerrar: problemas e vazamentos de memória persistem. Use \"Reiniciar\" ou desative.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    // reinicio pendente
    let pendente = chave(&hklm, r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired", false).is_some()
        || chave(&hklm, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending", false).is_some();
    if pendente {
        a.push(Achado {
            grav: Gravidade::Medio,
            cat: Categoria::Sistema,
            titulo: "Reinício pendente de atualização".into(),
            detalhe: String::new(),
            porque: "Atualizações aguardando reinício continuam trabalhando em segundo plano e podem causar lentidão.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    // disco
    let discos = sysinfo::Disks::new_with_refreshed_list();
    for dsk in discos.list() {
        let t = dsk.total_space() as f64;
        if t < 10.0 * 1073741824.0 {
            continue;
        }
        let livre = dsk.available_space() as f64;
        let pct = livre / t * 100.0;
        let mp = dsk.mount_point().to_string_lossy().into_owned();
        if pct < 15.0 {
            a.push(Achado {
                grav: if mp.to_uppercase().starts_with("C:") { Gravidade::Alto } else { Gravidade::Medio },
                cat: Categoria::Sistema,
                titulo: format!("Disco {mp} quase cheio ({pct:.0}% livre)"),
                detalhe: format!("{:.0} GB livres de {:.0} GB", livre / 1073741824.0, t / 1073741824.0),
                porque: "Com menos de 15% livre o Windows fica lento, o arquivo de paginação sofre e jogos travam ao carregar.".into(),
                caminho: String::new(),
                acao: Acao::Nenhuma,
            });
        }
    }
    // temporarios
    let mut temp: u64 = 0;
    for p in [std::env::temp_dir(), PathBuf::from(win_dir()).join("Temp")] {
        temp += tamanho_rapido(&p, 0);
    }
    if temp > 2 * 1073741824 {
        a.push(Achado {
            grav: Gravidade::Medio,
            cat: Categoria::Sistema,
            titulo: format!("{:.1} GB em arquivos temporários", temp as f64 / 1073741824.0),
            detalhe: "Use \"Limpar temporários\" nas Ações Rápidas.".into(),
            porque: "Ocupam espaço e deixam varreduras do antivírus mais lentas.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    // Game DVR
    if chave(&hkcu, r"System\GameConfigStore", false).and_then(|k| valor_u32(&k, "GameDVR_Enabled")) == Some(1) {
        a.push(Achado {
            grav: Gravidade::Medio,
            cat: Categoria::Sistema,
            titulo: "Gravação em segundo plano (Game DVR) ligada".into(),
            detalhe: "Configurações > Jogos > Capturas > \"Gravar o que aconteceu\"".into(),
            porque: "Grava a tela continuamente durante os jogos, custando FPS e disco.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    // HAGS
    let hags = chave(&hklm, r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers", false).and_then(|k| valor_u32(&k, "HwSchMode"));
    if hags != Some(2) {
        a.push(Achado {
            grav: Gravidade::Baixo,
            cat: Categoria::Sistema,
            titulo: "Agendamento de GPU acelerado por hardware desligado".into(),
            detalhe: "Configurações > Sistema > Tela > Gráficos > Configurações gráficas padrão".into(),
            porque: "Em placas RTX 20/30/40 ligar reduz a latência e é necessário para o Frame Generation.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    // Modo de jogo
    if chave(&hkcu, r"Software\Microsoft\GameBar", false).and_then(|k| valor_u32(&k, "AutoGameModeEnabled")) == Some(0) {
        a.push(Achado {
            grav: Gravidade::Baixo,
            cat: Categoria::Sistema,
            titulo: "Modo de Jogo desligado".into(),
            detalhe: "Configurações > Jogos > Modo de Jogo".into(),
            porque: "O Modo de Jogo dá prioridade ao jogo e adia atualizações durante a partida.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    // memoria virtual desativada
    if let Some(k) = chave(&hklm, r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management", false) {
        if let Ok(v) = k.get_value::<Vec<String>, _>("PagingFiles") {
            if v.iter().all(|s| s.trim().is_empty()) {
                a.push(Achado {
                    grav: Gravidade::Alto,
                    cat: Categoria::Sistema,
                    titulo: "Arquivo de paginação desativado".into(),
                    detalhe: String::new(),
                    porque: "Sem memória virtual, jogos e programas fecham sozinhos quando a RAM enche.".into(),
                    caminho: String::new(),
                    acao: Acao::Nenhuma,
                });
            }
        }
    }
}

fn tamanho_rapido(p: &Path, nivel: u32) -> u64 {
    if nivel > 6 {
        return 0;
    }
    let mut t = 0;
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            if let Ok(m) = e.metadata() {
                if m.is_dir() {
                    t += tamanho_rapido(&e.path(), nivel + 1);
                } else {
                    t += m.len();
                }
            }
        }
    }
    t
}

fn verificar_bateria(b: &Bateria, a: &mut Vec<Achado>) {
    if !b.tem {
        return;
    }
    if let Some(s) = b.saude() {
        let (g, txt) = if s < 40.0 {
            (Gravidade::Critico, "Bateria muito desgastada: considere a troca. Pode desligar de repente e limitar o desempenho.")
        } else if s < 60.0 {
            (Gravidade::Alto, "Bateria bem desgastada. Notebooks reduzem o desempenho para proteger baterias fracas.")
        } else if s < 80.0 {
            (Gravidade::Medio, "Desgaste moderado. Ative o modo de conservação de carga (limite de 80%) no app do fabricante.")
        } else {
            (Gravidade::Info, "Bateria em bom estado.")
        };
        a.push(Achado {
            grav: g,
            cat: Categoria::Bateria,
            titulo: format!("Saúde da bateria: {s:.0}%"),
            detalhe: format!(
                "Capacidade atual {:.1} Wh de {:.1} Wh originais{}",
                b.atual_mwh as f64 / 1000.0,
                b.projetada_mwh as f64 / 1000.0,
                b.ciclos.map(|c| format!(" · {c} ciclos")).unwrap_or_default()
            ),
            porque: txt.into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    if b.ciclos.unwrap_or(0) >= 500 {
        a.push(Achado {
            grav: Gravidade::Baixo,
            cat: Categoria::Bateria,
            titulo: format!("{} ciclos de carga", b.ciclos.unwrap_or(0)),
            detalhe: String::new(),
            porque: "Baterias de íon-lítio costumam perder capacidade a partir de 300-500 ciclos.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    let plano = plano_ativo();
    let copia = std::fs::read_to_string(pasta_dados().join("plano_alto.txt")).ok().and_then(|t| achar_guid(&t));
    let alto = plano.as_deref() == Some(GUID_ALTO_DESEMPENHO) || (plano.is_some() && plano == copia);
    if !b.na_tomada {
        a.push(Achado {
            grav: Gravidade::Medio,
            cat: Categoria::Bateria,
            titulo: format!("Notebook na bateria ({}%)", b.carga),
            detalhe: if alto { "Plano Alto desempenho ativo: drena a bateria rápido.".into() } else { String::new() },
            porque: "Na bateria, CPU e GPU são limitadas (a RTX pode perder metade do desempenho). Para jogar, ligue na tomada.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    } else if b.carga >= 95 && b.saude().map(|s| s < 90.0).unwrap_or(false) {
        a.push(Achado {
            grav: Gravidade::Baixo,
            cat: Categoria::Bateria,
            titulo: "Sempre na tomada com 100% de carga".into(),
            detalhe: "No app do fabricante: Dell Power Manager/Optimizer > \"Uso principal na tomada\" ou limite de 80%.".into(),
            porque: "Ficar em 100% o tempo todo e no calor acelera o desgaste da bateria.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
    if let (Some(at), Some(pr)) = (b.autonomia_min, b.autonomia_projetada_min) {
        a.push(Achado {
            grav: Gravidade::Info,
            cat: Categoria::Bateria,
            titulo: format!("Autonomia estimada: {}h{:02} (nova: {}h{:02})", at / 60, at % 60, pr / 60, pr % 60),
            detalhe: format!("{} {} {}", b.fabricante, b.modelo, b.quimica).trim().to_string(),
            porque: "Estimativa do próprio Windows com base no uso recente.".into(),
            caminho: String::new(),
            acao: Acao::Nenhuma,
        });
    }
}

// =================================================================== registro das acoes reversiveis
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Desativado {
    pub tipo: String, // "inicio" | "tarefa"
    pub nome: String,
    pub entrada: Option<EntradaInicio>,
    pub quando: u64,
}

fn arq_desativados() -> PathBuf {
    pasta_dados().join("desativados.json")
}

pub fn listar_desativados() -> Vec<Desativado> {
    std::fs::read_to_string(arq_desativados()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn salvar_desativados(v: &[Desativado]) {
    if let Ok(t) = serde_json::to_string_pretty(v) {
        let _ = std::fs::write(arq_desativados(), t);
    }
}

pub fn executar_acao(sh: &Shared, acao: &Acao) -> bool {
    match acao {
        Acao::Nenhuma => true,
        Acao::AbrirLocal(c) => {
            if Path::new(c).exists() {
                let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{c}")).spawn();
            }
            true
        }
        Acao::FecharProcesso(stem) => {
            let mut sys = System::new_with_specifics(RefreshKind::new());
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::new());
            let mut n = 0;
            for p in sys.processes().values() {
                let nm = p.name().to_string_lossy().to_lowercase();
                let s = nm.trim_end_matches(".exe");
                if s == stem && !PROTEGIDOS.contains(&s) && p.kill() {
                    n += 1;
                }
            }
            sh.log(format!("[OK] {n} processo(s) {stem} fechado(s) (volta ao reiniciar se iniciar com o Windows)"));
            n > 0
        }
        Acao::PararServico(nome) => {
            let ok = crate::sistema::parar_servico_registrado(nome);
            sh.log(if ok { format!("[OK] Serviço parado até reiniciar: {nome}") } else { format!("[--] Não foi possível parar: {nome}") });
            ok
        }
        Acao::DesativarInicio(e) => {
            let ok = definir_inicio(e, false);
            if ok {
                let mut v = listar_desativados();
                v.retain(|d| !(d.tipo == "inicio" && d.nome == e.nome));
                v.push(Desativado { tipo: "inicio".into(), nome: e.nome.clone(), entrada: Some(e.clone()), quando: crate::sistema::agora() });
                salvar_desativados(&v);
                sh.log(format!("[OK] Não inicia mais com o Windows: {} (reversível em Diagnóstico > Desativados)", e.nome));
            } else {
                sh.log(format!("[--] Não foi possível desativar: {}", e.nome));
            }
            ok
        }
        Acao::DesativarTarefa(t) => {
            let ok = definir_tarefa(t, false);
            if ok {
                let mut v = listar_desativados();
                v.retain(|d| !(d.tipo == "tarefa" && d.nome == *t));
                v.push(Desativado { tipo: "tarefa".into(), nome: t.clone(), entrada: None, quando: crate::sistema::agora() });
                salvar_desativados(&v);
                sh.log(format!("[OK] Tarefa desativada: {t} (reversível em Diagnóstico > Desativados)"));
            } else {
                sh.log(format!("[--] Não foi possível desativar a tarefa: {t}"));
            }
            ok
        }
    }
}

pub fn reativar(sh: &Shared, d: &Desativado) -> bool {
    let ok = match (d.tipo.as_str(), &d.entrada) {
        ("inicio", Some(e)) => definir_inicio(e, true),
        ("tarefa", _) => definir_tarefa(&d.nome, true),
        _ => false,
    };
    if ok {
        let mut v = listar_desativados();
        v.retain(|x| !(x.tipo == d.tipo && x.nome == d.nome));
        salvar_desativados(&v);
        sh.log(format!("[OK] Reativado: {}", d.nome));
    }
    ok
}

// =================================================================== relatorio
pub fn relatorio_txt(d: &Diagnostico) -> String {
    let mut s = String::new();
    s.push_str("WinOpti - Mapa de ameacas e desempenho\r\n");
    s.push_str(&format!(
        "Nota: {}/100 | {} criticos, {} altos, {} medios | {} processos, {} servicos, {} tarefas | {:.0}s\r\n",
        d.nota,
        d.contar(Gravidade::Critico),
        d.contar(Gravidade::Alto),
        d.contar(Gravidade::Medio),
        d.total_proc,
        d.total_servicos,
        d.total_tarefas,
        d.segundos
    ));
    s.push_str(&format!("RAM: {:.1} GB, {:.0}% em uso\r\n", d.ram_total_gb, d.ram_uso_pct));
    if d.bateria.tem {
        s.push_str(&format!(
            "Bateria: saude {} | carga {}% | {}\r\n",
            d.bateria.saude().map(|x| format!("{x:.0}%")).unwrap_or("?".into()),
            d.bateria.carga,
            if d.bateria.na_tomada { "na tomada" } else { "na bateria" }
        ));
    }
    for c in Categoria::TODAS {
        let itens = d.da_categoria(c);
        if itens.is_empty() {
            continue;
        }
        s.push_str(&format!("\r\n==== {} ({}) ====\r\n", c.nome(), itens.len()));
        for x in itens {
            s.push_str(&format!("[{}] {}\r\n      {}\r\n      {}\r\n", x.grav.nome(), x.titulo, x.detalhe, x.porque));
        }
    }
    s.push_str("\r\n==== Processos que mais consomem ====\r\n");
    for p in &d.top {
        s.push_str(&format!(
            "{:<32} x{:<3} RAM {:>7.0} MB  CPU {:>5.1}%  GPU {:>4.0}%  Disco {:>5.1} MB/s  {}\r\n",
            p.nome, p.qtd, p.ram_mb, p.cpu, p.gpu, p.disco_mbs, p.caminho
        ));
    }
    s.push_str("\r\n==== Inicializacao ====\r\n");
    for e in &d.inicio {
        s.push_str(&format!("{} {:<14} {} = {}\r\n", if e.ativo { "[ATIVO]     " } else { "[desativado]" }, e.origem, e.nome, e.comando));
    }
    s
}
